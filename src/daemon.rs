mod config;
mod error;
mod ipc;
mod logs;
mod process;
mod state;

use error::Result;

fn main() -> Result<()> {
    // Daemon entry point - will be implemented in task 17
    println!("Adasa Daemon");
    Ok(())
}
