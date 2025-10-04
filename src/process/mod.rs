// Process module - Core process lifecycle management

mod manager;
pub mod spawner;

pub use manager::{ManagedProcess, ProcessId, ProcessManager, ProcessState, ProcessStats};
pub use spawner::{spawn_process, SpawnedProcess};
