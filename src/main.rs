// Entry point — routes CLI subcommands to their implementations
mod cli;
mod commands;
mod config;
mod diff;
mod ignore;
mod index;
mod object;
mod repo;

use std::process;

fn main() {
    if let Err(e) = cli::run() {
        eprintln!("error: {e}");
        process::exit(1);
    }
}
