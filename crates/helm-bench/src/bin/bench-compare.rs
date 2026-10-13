//! Compares criterion baselines against `budgets.toml`.
//!
//! ```text
//! bench-compare --criterion-dir DIR --budgets FILE \
//!     --base NAME [--base NAME ...] --head NAME [--head NAME ...]
//! ```
//!
//! Each `--base` and `--head` names a baseline saved with criterion's
//! `--save-baseline`. Prints a Markdown table of every benchmark and exits 1
//! if any benchmark regressed past its budget, 2 on bad input.

use std::path::PathBuf;
use std::process::ExitCode;

use helm_bench::compare::{self, Budgets, Status, Timings};

struct Args {
    criterion_dir: PathBuf,
    budgets: PathBuf,
    base: Vec<String>,
    head: Vec<String>,
}

fn parse_args() -> Result<Args, String> {
    let mut criterion_dir = None;
    let mut budgets = None;
    let mut base = Vec::new();
    let mut head = Vec::new();

    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let value = args.next().ok_or_else(|| format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--criterion-dir" => criterion_dir = Some(PathBuf::from(value)),
            "--budgets" => budgets = Some(PathBuf::from(value)),
            "--base" => base.push(value),
            "--head" => head.push(value),
            _ => return Err(format!("unknown flag {flag}")),
        }
    }

    if head.is_empty() {
        return Err("give at least one --head baseline".to_owned());
    }
    Ok(Args {
        criterion_dir: criterion_dir.ok_or("--criterion-dir is required")?,
        budgets: budgets.ok_or("--budgets is required")?,
        base,
        head,
    })
}

fn read_side(args: &Args, baselines: &[String]) -> Result<Timings, String> {
    let runs = baselines
        .iter()
        .map(|name| compare::read_baseline(&args.criterion_dir, name))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(compare::fastest(&runs))
}

fn run() -> Result<bool, String> {
    let args = parse_args()?;
    let text = std::fs::read_to_string(&args.budgets)
        .map_err(|err| format!("{}: {err}", args.budgets.display()))?;
    let budgets = Budgets::parse(&text)?;

    let base = read_side(&args, &args.base)?;
    let head = read_side(&args, &args.head)?;
    if head.is_empty() {
        return Err("no benchmark results found for the head baselines".to_owned());
    }

    for id in budgets.overrides.keys() {
        if !head.contains_key(id) {
            eprintln!("warning: budgets.toml has an override for `{id}`, which did not run");
        }
    }

    let rows = compare::compare(&base, &head, &budgets);
    println!("{}", compare::render_table(&rows));

    let regressed: Vec<&str> = rows
        .iter()
        .filter(|row| row.status == Status::Regressed)
        .map(|row| row.id.as_str())
        .collect();
    if regressed.is_empty() {
        println!("No benchmark regressed past its budget.");
        Ok(true)
    } else {
        println!("Regressed past budget: {}", regressed.join(", "));
        Ok(false)
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(err) => {
            eprintln!("bench-compare: {err}");
            ExitCode::from(2)
        }
    }
}
