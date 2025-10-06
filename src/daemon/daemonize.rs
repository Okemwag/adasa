// Daemonization support for Unix systems

use crate::error::{AdasaError, Result};

#[cfg(unix)]
pub fn daemonize() -> Result<()> {
    use nix::unistd::{fork, setsid, ForkResult};
    use std::fs::OpenOptions;
    use std::os::unix::io::AsRawFd;

    // First fork
    match unsafe { fork() } {
        Ok(ForkResult::Parent { .. }) => {
            // Parent process exits
            std::process::exit(0);
        }
        Ok(ForkResult::Child) => {
            // Child continues
        }
        Err(e) => {
            return Err(AdasaError::Other(format!("First fork failed: {}", e)));
        }
    }

    // Create new session and become session leader
    setsid().map_err(|e| AdasaError::Other(format!("setsid failed: {}", e)))?;

    // Second fork to ensure we're not a session leader
    // This prevents the daemon from acquiring a controlling terminal
    match unsafe { fork() } {
        Ok(ForkResult::Parent { .. }) => {
            // Parent process exits
            std::process::exit(0);
        }
        Ok(ForkResult::Child) => {
            // Child continues as daemon
        }
        Err(e) => {
            return Err(AdasaError::Other(format!("Second fork failed: {}", e)));
        }
    }

    // Change working directory to root to avoid keeping any directory in use
    std::env::set_current_dir("/")
        .map_err(|e| AdasaError::Other(format!("Failed to change directory to /: {}", e)))?;

    // Redirect stdin, stdout, stderr to /dev/null
    let devnull = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/null")
        .map_err(|e| AdasaError::Other(format!("Failed to open /dev/null: {}", e)))?;

    let devnull_fd = devnull.as_raw_fd();

    // Redirect standard file descriptors
    use nix::libc;
    unsafe {
        libc::dup2(devnull_fd, libc::STDIN_FILENO);
        libc::dup2(devnull_fd, libc::STDOUT_FILENO);
        libc::dup2(devnull_fd, libc::STDERR_FILENO);
    }

    Ok(())
}

#[cfg(not(unix))]
pub fn daemonize() -> Result<()> {
    Err(AdasaError::Other(
        "Daemonization is only supported on Unix systems".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(unix)]
    fn test_daemonize_compiles() {
        // This test just ensures the daemonize function compiles
        // We can't actually test daemonization in a unit test
        // as it would fork the test process
    }
}
