// State module - Persistent storage for process state

use crate::error::{AdasaError, Result};
use crate::ipc::protocol::{ProcessId, ProcessState, ProcessStats};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Version of the state file format
const STATE_VERSION: &str = "1.0.0";

/// Persistent state for a single process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedProcess {
    pub id: ProcessId,
    pub name: String,
    pub script: PathBuf,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub env: HashMap<String, String>,
    pub state: ProcessState,
    pub stats: ProcessStats,
    pub autorestart: bool,
    pub max_restarts: usize,
    pub instances: usize,
}

/// Complete daemon state that gets persisted to disk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonState {
    pub version: String,
    pub processes: Vec<PersistedProcess>,
    pub last_updated: SystemTime,
}

impl DaemonState {
    /// Create a new empty daemon state
    pub fn new() -> Self {
        Self {
            version: STATE_VERSION.to_string(),
            processes: Vec::new(),
            last_updated: SystemTime::now(),
        }
    }

    /// Validate the state structure
    pub fn validate(&self) -> Result<()> {
        // Check version compatibility
        if self.version != STATE_VERSION {
            return Err(AdasaError::StateCorruption(format!(
                "Incompatible state version: expected {}, found {}",
                STATE_VERSION, self.version
            )));
        }

        // Check for duplicate process IDs
        let mut seen_ids = std::collections::HashSet::new();
        for process in &self.processes {
            if !seen_ids.insert(process.id) {
                return Err(AdasaError::StateCorruption(format!(
                    "Duplicate process ID found: {}",
                    process.id
                )));
            }
        }

        // Check for duplicate process names
        let mut seen_names = std::collections::HashSet::new();
        for process in &self.processes {
            if !seen_names.insert(&process.name) {
                return Err(AdasaError::StateCorruption(format!(
                    "Duplicate process name found: {}",
                    process.name
                )));
            }
        }

        Ok(())
    }
}

impl Default for DaemonState {
    fn default() -> Self {
        Self::new()
    }
}

/// State store handles persistence of daemon state to disk
pub struct StateStore {
    path: PathBuf,
}

impl StateStore {
    /// Create a new state store with the given file path
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }

    /// Load state from disk
    pub fn load(&self) -> Result<DaemonState> {
        // If file doesn't exist, return empty state
        if !self.path.exists() {
            return Ok(DaemonState::new());
        }

        // Open and read the file
        let file = File::open(&self.path).map_err(|e| {
            AdasaError::StateLoadError(format!("Failed to open state file: {}", e))
        })?;

        let reader = BufReader::new(file);

        // Deserialize JSON
        let state: DaemonState = serde_json::from_reader(reader).map_err(|e| {
            AdasaError::StateLoadError(format!("Failed to parse state file: {}", e))
        })?;

        // Validate the loaded state
        state.validate()?;

        Ok(state)
    }

    /// Save state to disk with atomic writes
    pub fn save(&self, state: &DaemonState) -> Result<()> {
        // Validate state before saving
        state.validate()?;

        // Create parent directory if it doesn't exist
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                AdasaError::StateSaveError(format!("Failed to create state directory: {}", e))
            })?;
        }

        // Write to a temporary file first (atomic write pattern)
        let temp_path = self.path.with_extension("tmp");

        {
            let file = File::create(&temp_path).map_err(|e| {
                AdasaError::StateSaveError(format!("Failed to create temp state file: {}", e))
            })?;

            let mut writer = BufWriter::new(file);

            // Serialize to JSON with pretty printing for human readability
            serde_json::to_writer_pretty(&mut writer, state).map_err(|e| {
                AdasaError::StateSaveError(format!("Failed to serialize state: {}", e))
            })?;

            // Ensure all data is written to disk
            writer.flush().map_err(|e| {
                AdasaError::StateSaveError(format!("Failed to flush state file: {}", e))
            })?;
        }

        // Atomically rename temp file to actual file
        fs::rename(&temp_path, &self.path).map_err(|e| {
            AdasaError::StateSaveError(format!("Failed to rename temp state file: {}", e))
        })?;

        Ok(())
    }

    /// Clear the state file
    pub fn clear(&self) -> Result<()> {
        if self.path.exists() {
            fs::remove_file(&self.path).map_err(|e| {
                AdasaError::StateError(format!("Failed to clear state file: {}", e))
            })?;
        }
        Ok(())
    }

    /// Get the path to the state file
    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tempfile::TempDir;

    fn create_test_process(id: u64, name: &str) -> PersistedProcess {
        PersistedProcess {
            id: ProcessId::new(id),
            name: name.to_string(),
            script: PathBuf::from("/usr/bin/test"),
            args: vec!["arg1".to_string()],
            cwd: Some(PathBuf::from("/tmp")),
            env: HashMap::new(),
            state: ProcessState::Running,
            stats: ProcessStats {
                pid: Some(1234),
                uptime: Duration::from_secs(100),
                restarts: 0,
                cpu_usage: 1.5,
                memory_usage: 1024 * 1024,
                last_restart: None,
            },
            autorestart: true,
            max_restarts: 10,
            instances: 1,
        }
    }

    #[test]
    fn test_daemon_state_new() {
        let state = DaemonState::new();
        assert_eq!(state.version, STATE_VERSION);
        assert!(state.processes.is_empty());
    }

    #[test]
    fn test_daemon_state_validate_success() {
        let mut state = DaemonState::new();
        state.processes.push(create_test_process(1, "test1"));
        state.processes.push(create_test_process(2, "test2"));

        assert!(state.validate().is_ok());
    }

    #[test]
    fn test_daemon_state_validate_duplicate_id() {
        let mut state = DaemonState::new();
        state.processes.push(create_test_process(1, "test1"));
        state.processes.push(create_test_process(1, "test2"));

        let result = state.validate();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AdasaError::StateCorruption(_)));
    }

    #[test]
    fn test_daemon_state_validate_duplicate_name() {
        let mut state = DaemonState::new();
        state.processes.push(create_test_process(1, "test"));
        state.processes.push(create_test_process(2, "test"));

        let result = state.validate();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AdasaError::StateCorruption(_)));
    }

    #[test]
    fn test_daemon_state_validate_wrong_version() {
        let mut state = DaemonState::new();
        state.version = "0.0.0".to_string();

        let result = state.validate();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AdasaError::StateCorruption(_)));
    }

    #[test]
    fn test_state_store_load_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let state_path = temp_dir.path().join("state.json");
        let store = StateStore::new(&state_path);

        let state = store.load().unwrap();
        assert!(state.processes.is_empty());
    }

    #[test]
    fn test_state_store_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let state_path = temp_dir.path().join("state.json");
        let store = StateStore::new(&state_path);

        // Create and save state
        let mut state = DaemonState::new();
        state.processes.push(create_test_process(1, "test1"));
        state.processes.push(create_test_process(2, "test2"));

        store.save(&state).unwrap();

        // Load and verify
        let loaded_state = store.load().unwrap();
        assert_eq!(loaded_state.processes.len(), 2);
        assert_eq!(loaded_state.processes[0].name, "test1");
        assert_eq!(loaded_state.processes[1].name, "test2");
    }

    #[test]
    fn test_state_store_atomic_write() {
        let temp_dir = TempDir::new().unwrap();
        let state_path = temp_dir.path().join("state.json");
        let store = StateStore::new(&state_path);

        // Save initial state
        let mut state1 = DaemonState::new();
        state1.processes.push(create_test_process(1, "test1"));
        store.save(&state1).unwrap();

        // Save updated state
        let mut state2 = DaemonState::new();
        state2.processes.push(create_test_process(2, "test2"));
        store.save(&state2).unwrap();

        // Verify only the latest state is present
        let loaded_state = store.load().unwrap();
        assert_eq!(loaded_state.processes.len(), 1);
        assert_eq!(loaded_state.processes[0].name, "test2");
    }

    #[test]
    fn test_state_store_clear() {
        let temp_dir = TempDir::new().unwrap();
        let state_path = temp_dir.path().join("state.json");
        let store = StateStore::new(&state_path);

        // Save state
        let mut state = DaemonState::new();
        state.processes.push(create_test_process(1, "test1"));
        store.save(&state).unwrap();

        // Clear state
        store.clear().unwrap();

        // Verify file is gone
        assert!(!state_path.exists());

        // Loading should return empty state
        let loaded_state = store.load().unwrap();
        assert!(loaded_state.processes.is_empty());
    }

    #[test]
    fn test_state_store_creates_parent_directory() {
        let temp_dir = TempDir::new().unwrap();
        let state_path = temp_dir.path().join("subdir").join("state.json");
        let store = StateStore::new(&state_path);

        let state = DaemonState::new();
        store.save(&state).unwrap();

        assert!(state_path.exists());
        assert!(state_path.parent().unwrap().exists());
    }

    #[test]
    fn test_state_store_path() {
        let state_path = PathBuf::from("/tmp/test_state.json");
        let store = StateStore::new(&state_path);

        assert_eq!(store.path(), state_path.as_path());
    }
}
