mod cli;
mod config;
mod error;
mod ipc;
mod logs;
mod process;
mod state;

use cli::Cli;
use error::Result;

fn main() -> Result<()> {
    // Initialize CLI and execute command
    Cli::run()
}
