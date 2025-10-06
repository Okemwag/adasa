use adasa::cli::Cli;

fn main() {
    // Initialize CLI and execute command
    if let Err(e) = Cli::run() {
        eprintln!("✗ Error: {}", e);
        std::process::exit(1);
    }
}
