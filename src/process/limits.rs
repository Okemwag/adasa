use crate::error::{AdasaError, Result};
use tracing::{info, warn};

#[cfg(unix)]
use nix::sys::resource::{setrlimit, Resource};

/// Resource limits configuration for a process
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    /// Maximum memory in bytes
    pub max_memory: Option<u64>,
    /// Maximum CPU percentage (0-100)
    pub max_cpu: Option<u32>,
}

impl ResourceLimits {
    /// Create new resource limits from configuration
    pub fn new(max_memory: Option<u64>, max_cpu: Option<u32>) -> Self {
        Self {
            max_memory,
            max_cpu,
        }
    }

    /// Apply memory limits to the current process (must be called before exec)
    #[cfg(unix)]
    pub fn apply_memory_limit(&self) -> Result<()> {
        if let Some(max_memory) = self.max_memory {
            info!("Setting memory limit to {} bytes", max_memory);

            // Set both soft and hard limits for virtual memory (RLIMIT_AS)
            setrlimit(Resource::RLIMIT_AS, max_memory, max_memory).map_err(|e| {
                AdasaError::ResourceLimitError(format!("Failed to set memory limit: {}", e))
            })?;

            info!("Memory limit applied successfully");
        }
        Ok(())
    }

    #[cfg(not(unix))]
    pub fn apply_memory_limit(&self) -> Result<()> {
        if self.max_memory.is_some() {
            warn!("Memory limits are not supported on this platform");
        }
        Ok(())
    }

    /// Check if CPU limit is configured
    pub fn has_cpu_limit(&self) -> bool {
        self.max_cpu.is_some()
    }

    /// Get the CPU limit percentage
    pub fn cpu_limit(&self) -> Option<u32> {
        self.max_cpu
    }
}

/// CPU throttling using cgroups (Linux only)
#[cfg(target_os = "linux")]
pub mod cgroup {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};

    const CGROUP_BASE: &str = "/sys/fs/cgroup";

    /// CGroup manager for CPU throttling
    #[derive(Debug)]
    pub struct CGroupManager {
        cgroup_path: PathBuf,
        process_name: String,
    }

    impl CGroupManager {
        /// Create a new cgroup manager for a process
        pub fn new(process_name: String) -> Self {
            let cgroup_path = PathBuf::from(CGROUP_BASE)
                .join("adasa")
                .join(&process_name);

            Self {
                cgroup_path,
                process_name,
            }
        }

        /// Check if cgroups v2 is available
        pub fn is_cgroups_v2_available() -> bool {
            Path::new(CGROUP_BASE).join("cgroup.controllers").exists()
        }

        /// Setup cgroup for the process
        pub fn setup(&self) -> Result<()> {
            if !Self::is_cgroups_v2_available() {
                return Err(AdasaError::ResourceLimitError(
                    "cgroups v2 not available on this system".to_string(),
                ));
            }

            // Create the adasa parent cgroup if it doesn't exist
            let adasa_cgroup = PathBuf::from(CGROUP_BASE).join("adasa");
            if !adasa_cgroup.exists() {
                fs::create_dir(&adasa_cgroup).map_err(|e| {
                    AdasaError::ResourceLimitError(format!(
                        "Failed to create adasa cgroup: {}. You may need root privileges.",
                        e
                    ))
                })?;

                // Enable cpu controller in parent
                let subtree_control = adasa_cgroup.join("cgroup.subtree_control");
                fs::write(&subtree_control, "+cpu").map_err(|e| {
                    AdasaError::ResourceLimitError(format!(
                        "Failed to enable cpu controller: {}",
                        e
                    ))
                })?;
            }

            // Create process-specific cgroup
            if !self.cgroup_path.exists() {
                fs::create_dir(&self.cgroup_path).map_err(|e| {
                    AdasaError::ResourceLimitError(format!("Failed to create cgroup: {}", e))
                })?;
            }

            Ok(())
        }

        /// Apply CPU limit to a process
        pub fn apply_cpu_limit(&self, pid: u32, cpu_percent: u32) -> Result<()> {
            if !self.cgroup_path.exists() {
                self.setup()?;
            }

            // Add process to cgroup
            let procs_file = self.cgroup_path.join("cgroup.procs");
            fs::write(&procs_file, pid.to_string()).map_err(|e| {
                AdasaError::ResourceLimitError(format!("Failed to add process to cgroup: {}", e))
            })?;

            // Set CPU quota
            // cpu.max format: "$MAX $PERIOD"
            // For example, "50000 100000" means 50% CPU (50ms out of every 100ms)
            let period = 100_000; // 100ms in microseconds
            let quota = (period * cpu_percent as u64) / 100;

            let cpu_max = format!("{} {}", quota, period);
            let cpu_max_file = self.cgroup_path.join("cpu.max");

            fs::write(&cpu_max_file, cpu_max).map_err(|e| {
                AdasaError::ResourceLimitError(format!("Failed to set CPU limit: {}", e))
            })?;

            info!(
                "Applied {}% CPU limit to process {} (PID: {})",
                cpu_percent, self.process_name, pid
            );

            Ok(())
        }

        /// Remove CPU limit from a process
        pub fn remove_cpu_limit(&self, pid: u32) -> Result<()> {
            if !self.cgroup_path.exists() {
                return Ok(());
            }

            // Move process back to root cgroup
            let root_procs = PathBuf::from(CGROUP_BASE).join("cgroup.procs");
            if let Err(e) = fs::write(&root_procs, pid.to_string()) {
                warn!(
                    "Failed to remove process {} from cgroup: {}",
                    pid, e
                );
            }

            Ok(())
        }

        /// Cleanup cgroup (call when process is stopped)
        pub fn cleanup(&self) -> Result<()> {
            if self.cgroup_path.exists() {
                if let Err(e) = fs::remove_dir(&self.cgroup_path) {
                    warn!(
                        "Failed to remove cgroup for {}: {}",
                        self.process_name, e
                    );
                }
            }
            Ok(())
        }
    }

    impl Drop for CGroupManager {
        fn drop(&mut self) {
            let _ = self.cleanup();
        }
    }
}

#[cfg(not(target_os = "linux"))]
pub mod cgroup {
    use super::*;

    #[derive(Debug)]
    pub struct CGroupManager {
        process_name: String,
    }

    impl CGroupManager {
        pub fn new(process_name: String) -> Self {
            Self { process_name }
        }

        pub fn is_cgroups_v2_available() -> bool {
            false
        }

        pub fn setup(&self) -> Result<()> {
            warn!("CPU throttling via cgroups is only supported on Linux");
            Ok(())
        }

        pub fn apply_cpu_limit(&self, _pid: u32, _cpu_percent: u32) -> Result<()> {
            warn!(
                "CPU throttling is not supported on this platform for process {}",
                self.process_name
            );
            Ok(())
        }

        pub fn remove_cpu_limit(&self, _pid: u32) -> Result<()> {
            Ok(())
        }

        pub fn cleanup(&self) -> Result<()> {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_limits_new() {
        let limits = ResourceLimits::new(Some(1024 * 1024 * 100), Some(50));
        assert_eq!(limits.max_memory, Some(1024 * 1024 * 100));
        assert_eq!(limits.max_cpu, Some(50));
    }

    #[test]
    fn test_has_cpu_limit() {
        let limits_with = ResourceLimits::new(None, Some(50));
        assert!(limits_with.has_cpu_limit());

        let limits_without = ResourceLimits::new(None, None);
        assert!(!limits_without.has_cpu_limit());
    }

    #[test]
    fn test_cpu_limit() {
        let limits = ResourceLimits::new(None, Some(75));
        assert_eq!(limits.cpu_limit(), Some(75));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_cgroup_manager_creation() {
        let _manager = cgroup::CGroupManager::new("test-process".to_string());
        // Just verify it can be created
        assert!(true);
    }
}
