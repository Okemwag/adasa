use crate::config::{LimitAction, ProcessConfig};
use crate::error::{AdasaError, Result};
use crate::ipc::protocol::ProcessId;
use crate::process::monitor::ProcessMonitor;
use crate::process::spawner::spawn_process;
use crate::process::types::{ManagedProcess, ProcessState};
use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;
use std::collections::HashMap;
use std::time::Duration;

pub use crate::process::types::{ProcessState as ProcState, ProcessStats};

pub struct ProcessManager {
    processes: HashMap<ProcessId, ManagedProcess>,
    next_id: u64,
    monitor: ProcessMonitor,
}

impl ProcessManager {
    pub fn new() -> Self {
        Self {
            processes: HashMap::new(),
            next_id: 1,
            monitor: ProcessMonitor::new(),
        }
    }

    pub async fn spawn(&mut self, config: ProcessConfig) -> Result<ProcessId> {
        if self.processes.values().any(|p| p.name == config.name) {
            return Err(AdasaError::ProcessAlreadyExists(config.name.clone()));
        }

        config.validate()?;

        let spawned = spawn_process(&config).await?;
        let name = spawned.name.clone();
        let id = ProcessId::new(self.next_id);
        self.next_id += 1;

        let mut managed = ManagedProcess::new(id, name, config.clone(), spawned);

        if let Some(cpu_limit) = config.max_cpu {
            if let Some(ref cgroup_manager) = managed.cgroup_manager {
                if let Err(e) = cgroup_manager.apply_cpu_limit(managed.stats.pid, cpu_limit) {
                    tracing::warn!(
                        "Failed to apply CPU limit to process {}: {}",
                        managed.name,
                        e
                    );
                }
            }
        }

        managed.mark_running();
        self.processes.insert(id, managed);

        Ok(id)
    }

    pub async fn stop(&mut self, id: ProcessId, force: bool) -> Result<()> {
        let process = self
            .processes
            .get_mut(&id)
            .ok_or_else(|| AdasaError::ProcessNotFound(id.to_string()))?;

        process.mark_stopping();

        let pid = process.stats.pid;
        let nix_pid = Pid::from_raw(pid as i32);
        let process_name = process.name.clone();

        if force {
            tracing::info!(
                "Force stopping process {} (PID: {}) with SIGKILL",
                process_name,
                pid
            );
            signal::kill(nix_pid, Signal::SIGKILL).map_err(|e| {
                AdasaError::StopError(
                    process_name.clone(),
                    format!("Failed to send SIGKILL: {}", e),
                )
            })?;
        } else {
            let stop_signal = Self::parse_signal(&process.config.stop_signal)?;

            tracing::info!(
                "Gracefully stopping process {} (PID: {}) with {}",
                process_name,
                pid,
                process.config.stop_signal
            );

            signal::kill(nix_pid, stop_signal).map_err(|e| {
                AdasaError::StopError(
                    process_name.clone(),
                    format!("Failed to send {}: {}", process.config.stop_signal, e),
                )
            })?;

            let timeout = process.config.stop_timeout();
            tracing::debug!(
                "Waiting {:?} for process {} to exit gracefully",
                timeout,
                process_name
            );

            let wait_result = tokio::time::timeout(timeout, process.child.wait()).await;

            match wait_result {
                Ok(Ok(status)) => {
                    tracing::info!(
                        "Process {} exited gracefully with status: {:?}",
                        process_name,
                        status
                    );
                }
                Ok(Err(e)) => {
                    return Err(AdasaError::StopError(
                        process_name,
                        format!("Wait failed: {}", e),
                    ));
                }
                Err(_) => {
                    tracing::warn!(
                        "Process {} did not exit within {:?}, sending SIGKILL",
                        process_name,
                        timeout
                    );
                    signal::kill(nix_pid, Signal::SIGKILL).map_err(|e| {
                        AdasaError::StopError(
                            process_name.clone(),
                            format!("Failed to send SIGKILL after timeout: {}", e),
                        )
                    })?;
                }
            }
        }

        let _ = process.child.wait().await;
        process.mark_stopped();

        tracing::info!("Process {} stopped successfully", process_name);

        Ok(())
    }

    fn parse_signal(signal_name: &str) -> Result<Signal> {
        match signal_name {
            "SIGTERM" => Ok(Signal::SIGTERM),
            "SIGINT" => Ok(Signal::SIGINT),
            "SIGQUIT" => Ok(Signal::SIGQUIT),
            "SIGKILL" => Ok(Signal::SIGKILL),
            "SIGHUP" => Ok(Signal::SIGHUP),
            "SIGUSR1" => Ok(Signal::SIGUSR1),
            "SIGUSR2" => Ok(Signal::SIGUSR2),
            _ => Err(AdasaError::SignalError(format!(
                "Invalid signal name: {}",
                signal_name
            ))),
        }
    }

    pub fn get_status(&self, id: ProcessId) -> Option<&ManagedProcess> {
        self.processes.get(&id)
    }

    pub fn list(&self) -> Vec<&ManagedProcess> {
        self.processes.values().collect()
    }

    pub fn get_mut(&mut self, id: ProcessId) -> Option<&mut ManagedProcess> {
        self.processes.get_mut(&id)
    }

    pub fn remove(&mut self, id: ProcessId) -> Result<()> {
        let process = self
            .processes
            .remove(&id)
            .ok_or_else(|| AdasaError::ProcessNotFound(id.to_string()))?;

        self.monitor.clear_cache(process.stats.pid);

        Ok(())
    }

    pub fn find_by_name(&self, name: &str) -> Option<&ManagedProcess> {
        self.processes.values().find(|p| p.name == name)
    }

    pub fn find_all_by_name(&self, name: &str) -> Vec<&ManagedProcess> {
        self.processes
            .values()
            .filter(|p| p.name == name || p.name.starts_with(&format!("{}-", name)))
            .collect()
    }

    pub fn update_stats(&mut self) -> Result<()> {
        self.monitor.update_all_stats(self.processes.values_mut())
    }

    pub fn detect_crashes(&mut self) -> Vec<ProcessId> {
        let crashed_pids = self.monitor.detect_crashes(self.processes.values_mut());

        crashed_pids
            .into_iter()
            .filter_map(|pid| {
                self.processes
                    .iter()
                    .find(|(_, p)| p.stats.pid == pid)
                    .map(|(id, _)| *id)
            })
            .collect()
    }

    pub fn is_alive(&mut self, id: ProcessId) -> bool {
        if let Some(process) = self.processes.get(&id) {
            self.monitor.is_process_alive(process.stats.pid)
        } else {
            false
        }
    }

    pub async fn restart(&mut self, id: ProcessId) -> Result<()> {
        let process = self
            .processes
            .get(&id)
            .ok_or_else(|| AdasaError::ProcessNotFound(id.to_string()))?;

        let config = process.config.clone();

        self.stop(id, false).await?;

        let spawned = spawn_process(&config).await?;
        let new_pid = spawned.pid;

        let process = self
            .processes
            .get_mut(&id)
            .ok_or_else(|| AdasaError::ProcessNotFound(id.to_string()))?;

        process.child = spawned.child;
        process.stats.record_restart(new_pid);
        process.restart_tracker.record_restart();
        process.state = ProcessState::Running;

        Ok(())
    }

    pub async fn try_auto_restart(&mut self, id: ProcessId) -> Result<bool> {
        let process = self
            .processes
            .get(&id)
            .ok_or_else(|| AdasaError::ProcessNotFound(id.to_string()))?;

        if !process
            .restart_policy
            .should_restart(&process.restart_tracker)
        {
            return Ok(false);
        }

        let delay = process
            .restart_policy
            .calculate_delay(&process.restart_tracker);

        tokio::time::sleep(delay).await;

        let config = process.config.clone();

        let spawned = spawn_process(&config).await?;
        let new_pid = spawned.pid;

        let process = self
            .processes
            .get_mut(&id)
            .ok_or_else(|| AdasaError::ProcessNotFound(id.to_string()))?;

        process.child = spawned.child;
        process.stats.record_restart(new_pid);
        process.restart_tracker.record_restart();
        process.state = ProcessState::Running;

        Ok(true)
    }

    pub fn get_restart_info(&self, id: ProcessId) -> Option<(usize, bool)> {
        self.processes.get(&id).map(|p| {
            let count = p.restart_tracker.restart_count();
            let should_restart = p.restart_policy.should_restart(&p.restart_tracker);
            (count, should_restart)
        })
    }

    pub async fn rolling_restart(
        &mut self,
        name_or_id: &str,
        health_check_delay: Duration,
    ) -> Result<usize> {
        let instances: Vec<ProcessId> = if let Ok(id_num) = name_or_id.parse::<u64>() {
            let id = ProcessId::new(id_num);
            if let Some(process) = self.processes.get(&id) {
                let base_name = &process.name;
                self.find_all_by_name(base_name)
                    .iter()
                    .map(|p| p.id)
                    .collect()
            } else {
                return Err(AdasaError::ProcessNotFound(name_or_id.to_string()));
            }
        } else {
            self.find_all_by_name(name_or_id)
                .iter()
                .map(|p| p.id)
                .collect()
        };

        if instances.is_empty() {
            return Err(AdasaError::ProcessNotFound(name_or_id.to_string()));
        }

        if instances.len() == 1 {
            self.restart(instances[0]).await?;
            return Ok(1);
        }

        let mut restarted_count = 0;

        for (idx, instance_id) in instances.iter().enumerate() {
            println!(
                "Rolling restart: restarting instance {} of {} (ID: {})",
                idx + 1,
                instances.len(),
                instance_id
            );

            self.restart(*instance_id).await?;

            if idx < instances.len() - 1 {
                println!(
                    "Waiting {:?} for health check before restarting next instance...",
                    health_check_delay
                );
                tokio::time::sleep(health_check_delay).await;

                if !self.is_alive(*instance_id) {
                    return Err(AdasaError::RestartError(
                        instance_id.to_string(),
                        "Instance failed health check after restart".to_string(),
                    ));
                }

                println!("Health check passed for instance {}", instance_id);
            }

            restarted_count += 1;
        }

        println!(
            "Rolling restart completed: {} instances restarted successfully",
            restarted_count
        );

        Ok(restarted_count)
    }

    pub async fn stop_all(&mut self) -> Result<()> {
        let process_ids: Vec<ProcessId> = self.processes.keys().copied().collect();

        tracing::info!("Stopping {} processes gracefully", process_ids.len());

        for id in process_ids {
            if let Err(e) = self.stop(id, false).await {
                tracing::error!("Failed to stop process {}: {}", id, e);
            }
        }

        Ok(())
    }

    pub async fn check_resource_limits(&mut self) -> Vec<(ProcessId, String)> {
        let mut violations = Vec::new();
        let mut actions_needed: Vec<(ProcessId, LimitAction, String)> = Vec::new();

        for (id, process) in self.processes.iter_mut() {
            if let Some(max_memory) = process.config.max_memory {
                if process.stats.memory_usage > max_memory {
                    process.stats.record_memory_violation();
                    let msg = format!(
                        "Process {} exceeded memory limit: {} bytes (limit: {} bytes)",
                        process.name, process.stats.memory_usage, max_memory
                    );
                    tracing::warn!("{}", msg);
                    actions_needed.push((*id, process.config.limit_action, msg));
                    continue;
                }
            }

            if let Some(max_cpu) = process.config.max_cpu {
                if process.stats.cpu_usage > max_cpu as f32 {
                    process.stats.record_cpu_violation();
                    let msg = format!(
                        "Process {} exceeded CPU limit: {:.1}% (limit: {}%)",
                        process.name, process.stats.cpu_usage, max_cpu
                    );
                    tracing::warn!("{}", msg);
                    actions_needed.push((*id, process.config.limit_action, msg));
                }
            }
        }

        for (id, action, msg) in actions_needed {
            match action {
                LimitAction::Log => {
                    violations.push((id, format!("{} (logged)", msg)));
                }
                LimitAction::Restart => {
                    tracing::info!("Restarting process {} due to resource limit violation", id);
                    if let Err(e) = self.restart(id).await {
                        tracing::error!("Failed to restart process {}: {}", id, e);
                        violations.push((id, format!("{} (restart failed: {})", msg, e)));
                    } else {
                        violations.push((id, format!("{} (restarted)", msg)));
                    }
                }
                LimitAction::Stop => {
                    tracing::info!("Stopping process {} due to resource limit violation", id);
                    if let Err(e) = self.stop(id, false).await {
                        tracing::error!("Failed to stop process {}: {}", id, e);
                        violations.push((id, format!("{} (stop failed: {})", msg, e)));
                    } else {
                        violations.push((id, format!("{} (stopped)", msg)));
                    }
                }
            }
        }

        violations
    }
}

impl Default for ProcessManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
