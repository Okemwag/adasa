// Process module - Core process lifecycle management

pub mod spawner;
mod manager;

pub use spawner::{spawn_process, SpawnedProcess};
pub use manager::{ProcessManager, ProcessId, ProcessState, ManagedProcess, ProcessStats};
