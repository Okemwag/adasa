mod cli;
mod config;
mod error;
mod ipc;
mod logs;
mod process;
mod state;

use cli::Cli;

fn main() {
    // Initialize CLI and execute command
    if let Err(e) = Cli::run() {
        eprintln!("✗ Error: {}", e);
        std::process::exit(1);
    }
}
