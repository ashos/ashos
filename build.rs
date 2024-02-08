use clap::ValueEnum;
use clap_complete::{generate_to, Shell};
use std::io::Error;

include!("src/cli.rs");

// Build shell completions
fn main() -> Result<(), Error> {
    let out_dir = match std::env::var_os("OUT_DIR") {
        None => return Ok(()),
        Some(out_dir) => out_dir,
    };

    let mut cmd = cli();
    for &shell in Shell::value_variants() {
        generate_to(shell, &mut cmd, "ash", &out_dir)?;
    }

    Ok(())
}
