// Process module - Core process lifecycle management

mod manager;
pub mod monitor;
pub mod restart;
pub mod spawner;
pub mod supervisor;

pub use manager::{ManagedProcess, ProcessManager, ProcessState, ProcessStats};
pub use monitor::ProcessMonitor;
pub use restart::{BackoffStrategy, RestartPolicy, RestartTracker};
pub use spawner::{spawn_process, SpawnedProcess};
pub use supervisor::{ProcessSupervisor, SupervisorConfig};
