//! Compares two sets of criterion results against the budgets in
//! `budgets.toml`.
//!
//! Criterion writes each run to `<criterion dir>/<benchmark>/<baseline>/` when
//! given `--save-baseline <baseline>`. That directory holds `benchmark.json`
//! (the benchmark's id) and `estimates.json` (its timing statistics). This
//! module reads the median from each, keeps the fastest median per side when a
//! side ran more than once, and flags every benchmark whose head time exceeds
//! its base time by more than its budget.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use serde_json::Value as Json;
use toml::{Table, Value as Toml};

/// Median time per benchmark, in nanoseconds, keyed by criterion's full id.
pub type Timings = BTreeMap<String, f64>;

/// Allowed slowdown per benchmark, in percent, as read from `budgets.toml`.
#[derive(Debug, Clone, PartialEq)]
pub struct Budgets {
    /// Applies to every benchmark without an override.
    pub default: f64,
    /// Per-benchmark budgets, keyed by criterion's full id.
    pub overrides: BTreeMap<String, f64>,
}

impl Budgets {
    /// Parses the contents of `budgets.toml`.
    ///
    /// Rejects unknown top-level keys, so a misspelled `default` fails loudly
    /// instead of leaving a benchmark without a budget.
    pub fn parse(text: &str) -> Result<Self, String> {
        let table: Table = text.parse().map_err(|err| format!("budgets.toml: {err}"))?;

        let mut default = None;
        let mut overrides = BTreeMap::new();
        for (key, value) in &table {
            match key.as_str() {
                "default" => default = Some(percent(key, value)?),
                "benchmarks" => {
                    let Toml::Table(entries) = value else {
                        return Err("budgets.toml: `benchmarks` must be a table".to_owned());
                    };
                    for (id, budget) in entries {
                        overrides.insert(id.clone(), percent(id, budget)?);
                    }
                }
                other => return Err(format!("budgets.toml: unknown key `{other}`")),
            }
        }

        let default = default.ok_or("budgets.toml: missing `default`")?;
        Ok(Self { default, overrides })
    }

    /// The allowed slowdown for `id`, in percent.
    pub fn for_benchmark(&self, id: &str) -> f64 {
        self.overrides.get(id).copied().unwrap_or(self.default)
    }
}

fn percent(key: &str, value: &Toml) -> Result<f64, String> {
    let number = match value {
        Toml::Float(f) => *f,
        Toml::Integer(i) => *i as f64,
        _ => return Err(format!("budgets.toml: `{key}` must be a number")),
    };
    if !number.is_finite() || number < 0.0 {
        return Err(format!(
            "budgets.toml: `{key}` must be a non-negative percentage"
        ));
    }
    Ok(number)
}

/// Reads the median time of every benchmark saved under `baseline` in
/// `criterion_dir`.
pub fn read_baseline(criterion_dir: &Path, baseline: &str) -> Result<Timings, String> {
    let mut timings = Timings::new();
    visit(criterion_dir, baseline, &mut timings)?;
    Ok(timings)
}

fn visit(dir: &Path, baseline: &str, timings: &mut Timings) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|err| format!("{}: {err}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|err| format!("{}: {err}", dir.display()))?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if entry.file_name() == baseline && path.join("benchmark.json").is_file() {
            let id = read_json(&path.join("benchmark.json"))?["full_id"]
                .as_str()
                .ok_or_else(|| format!("{}: no full_id", path.display()))?
                .to_owned();
            let median = read_json(&path.join("estimates.json"))?["median"]["point_estimate"]
                .as_f64()
                .ok_or_else(|| format!("{}: no median", path.display()))?;
            timings.insert(id, median);
        } else {
            visit(&path, baseline, timings)?;
        }
    }
    Ok(())
}

fn read_json(path: &Path) -> Result<Json, String> {
    let text = fs::read_to_string(path).map_err(|err| format!("{}: {err}", path.display()))?;
    serde_json::from_str(&text).map_err(|err| format!("{}: {err}", path.display()))
}

/// Merges several runs of one side into one set of timings, keeping the
/// fastest median for each benchmark. Noise on a shared machine only ever adds
/// time, so the fastest run is the closest to the code's real cost.
pub fn fastest(runs: &[Timings]) -> Timings {
    let mut merged = Timings::new();
    for run in runs {
        for (id, &time) in run {
            merged
                .entry(id.clone())
                .and_modify(|best| *best = best.min(time))
                .or_insert(time);
        }
    }
    merged
}

/// How one benchmark compares between base and head.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Head is within the budget.
    Ok,
    /// Head is slower than base by more than the budget.
    Regressed,
    /// The benchmark exists only on head, so there is nothing to compare.
    New,
    /// The benchmark exists only on base.
    Removed,
}

/// One line of the comparison.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub id: String,
    /// Base median, in nanoseconds.
    pub base: Option<f64>,
    /// Head median, in nanoseconds.
    pub head: Option<f64>,
    /// Allowed slowdown, in percent.
    pub budget: f64,
    pub status: Status,
}

impl Row {
    /// Head's change over base, in percent. Positive means slower.
    pub fn change(&self) -> Option<f64> {
        match (self.base, self.head) {
            (Some(base), Some(head)) => Some((head - base) / base * 100.0),
            _ => None,
        }
    }
}

/// Compares every benchmark that appears on either side.
pub fn compare(base: &Timings, head: &Timings, budgets: &Budgets) -> Vec<Row> {
    let ids: BTreeSet<&String> = base.keys().chain(head.keys()).collect();
    ids.into_iter()
        .map(|id| {
            let mut row = Row {
                id: id.clone(),
                base: base.get(id).copied(),
                head: head.get(id).copied(),
                budget: budgets.for_benchmark(id),
                status: Status::Ok,
            };
            row.status = match (row.base, row.head) {
                (None, _) => Status::New,
                (_, None) => Status::Removed,
                _ if row.change().is_some_and(|change| change > row.budget) => Status::Regressed,
                _ => Status::Ok,
            };
            row
        })
        .collect()
}

/// Renders the comparison as a Markdown table, for the job log and the
/// workflow summary.
pub fn render_table(rows: &[Row]) -> String {
    let mut out = String::from(
        "| benchmark | base | head | change | budget | result |\n\
         | --- | ---: | ---: | ---: | ---: | --- |\n",
    );
    for row in rows {
        let change = row
            .change()
            .map_or_else(|| "n/a".to_owned(), |change| format!("{change:+.2}%"));
        let result = match row.status {
            Status::Ok => "ok",
            Status::Regressed => "REGRESSED",
            Status::New => "new",
            Status::Removed => "removed",
        };
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {:.1}% | {} |",
            row.id,
            row.base.map_or_else(|| "n/a".to_owned(), format_time),
            row.head.map_or_else(|| "n/a".to_owned(), format_time),
            change,
            row.budget,
            result,
        );
    }
    out
}

fn format_time(ns: f64) -> String {
    if ns < 1e3 {
        format!("{ns:.2} ns")
    } else if ns < 1e6 {
        format!("{:.2} µs", ns / 1e3)
    } else if ns < 1e9 {
        format!("{:.3} ms", ns / 1e6)
    } else {
        format!("{:.3} s", ns / 1e9)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn timings(entries: &[(&str, f64)]) -> Timings {
        entries.iter().map(|&(id, t)| (id.to_owned(), t)).collect()
    }

    #[test]
    fn parses_default_and_overrides() {
        let budgets =
            Budgets::parse("default = 5.0\n[benchmarks]\n\"rope/insert\" = 8\nslice = 2.5\n")
                .unwrap();
        assert_eq!(budgets.for_benchmark("rope/insert"), 8.0);
        assert_eq!(budgets.for_benchmark("slice"), 2.5);
        assert_eq!(budgets.for_benchmark("other"), 5.0);
    }

    #[test]
    fn parses_the_committed_budgets_file() {
        let text = include_str!("../budgets.toml");
        assert_eq!(Budgets::parse(text).unwrap().default, 5.0);
    }

    #[test]
    fn rejects_bad_budgets() {
        assert!(Budgets::parse("[benchmarks]\n").is_err());
        assert!(Budgets::parse("defualt = 5.0\n").is_err());
        assert!(Budgets::parse("default = -1.0\n").is_err());
        assert!(Budgets::parse("default = \"5%\"\n").is_err());
        assert!(Budgets::parse("default = 5.0\n[benchmarks]\na = nan\n").is_err());
    }

    #[test]
    fn fastest_keeps_the_minimum_of_each_benchmark() {
        let merged = fastest(&[
            timings(&[("a", 10.0), ("b", 5.0)]),
            timings(&[("a", 8.0), ("b", 6.0), ("c", 1.0)]),
        ]);
        assert_eq!(merged, timings(&[("a", 8.0), ("b", 5.0), ("c", 1.0)]));
    }

    #[test]
    fn flags_only_changes_over_budget() {
        let budgets = Budgets::parse("default = 5.0\n[benchmarks]\nloose = 30.0\n").unwrap();
        let base = timings(&[
            ("same", 100.0),
            ("edge", 100.0),
            ("slow", 100.0),
            ("loose", 100.0),
            ("gone", 1.0),
        ]);
        let head = timings(&[
            ("same", 90.0),
            ("edge", 105.0),
            ("slow", 105.1),
            ("loose", 120.0),
            ("fresh", 1.0),
        ]);
        let status: BTreeMap<String, Status> = compare(&base, &head, &budgets)
            .into_iter()
            .map(|row| (row.id, row.status))
            .collect();
        assert_eq!(status["same"], Status::Ok);
        assert_eq!(status["edge"], Status::Ok);
        assert_eq!(status["slow"], Status::Regressed);
        assert_eq!(status["loose"], Status::Ok);
        assert_eq!(status["gone"], Status::Removed);
        assert_eq!(status["fresh"], Status::New);
    }

    #[test]
    fn reads_criterion_output() {
        let dir = std::env::temp_dir().join(format!("helm-bench-compare-{}", std::process::id()));
        let run = dir.join("group").join("insert").join("base-1");
        fs::create_dir_all(&run).unwrap();
        fs::write(run.join("benchmark.json"), r#"{"full_id": "group/insert"}"#).unwrap();
        fs::write(
            run.join("estimates.json"),
            r#"{"median": {"point_estimate": 1234.5}}"#,
        )
        .unwrap();
        // Another baseline next to it must not be read.
        let other = dir.join("group").join("insert").join("head-1");
        fs::create_dir_all(&other).unwrap();
        fs::write(
            other.join("benchmark.json"),
            r#"{"full_id": "group/insert"}"#,
        )
        .unwrap();

        let read = read_baseline(&dir, "base-1");
        fs::remove_dir_all(&dir).unwrap();
        assert_eq!(read.unwrap(), timings(&[("group/insert", 1234.5)]));
    }

    #[test]
    fn renders_a_row_per_benchmark() {
        let budgets = Budgets::parse("default = 5.0\n").unwrap();
        let rows = compare(&timings(&[("a", 2e6)]), &timings(&[("a", 2.5e6)]), &budgets);
        let table = render_table(&rows);
        assert!(
            table.contains("| a | 2.000 ms | 2.500 ms | +25.00% | 5.0% | REGRESSED |"),
            "{table}"
        );
    }
}
