mod app;
mod autostart;
mod cli;
mod output;
mod process;
mod profile;
mod service;

use app::execute;
use clap::Parser;
use cli::Cli;
use output::color_enabled;
use std::io::{self, IsTerminal};

fn main() {
    let code = match execute(Cli::parse()) {
        Ok(code) => code,
        Err(error) => {
            let label = if color_enabled(io::stderr().is_terminal()) {
                "\x1b[31mError:\x1b[0m"
            } else {
                "Error:"
            };
            eprintln!("{label} {error:#}");
            1
        }
    };
    std::process::exit(code);
}
