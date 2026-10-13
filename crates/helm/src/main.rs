//! The Helm binary: startup, settings file and wiring the crates together.

fn main() {
    println!("helm {}", env!("CARGO_PKG_VERSION"));
}
