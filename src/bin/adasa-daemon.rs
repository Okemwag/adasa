// Module imports for daemon_core
// These are used within the daemon_core module below

// Daemon core module
mod daemon_core {
    use adasa::config::ProcessConfig;
    use adasa::error::{AdasaError, Result};
    use adasa::ipc::protocol::{Command, ProcessId, ProcessInfo, Response, ResponseData};
    use adasa::ipc::server::IpcServer;
    use adasa::logs::LogManager;
    use adasa::process::{ProcessManager, ProcessState as ProcState};
    use adasa::state::{DaemonState, PersistedProcess, StateStore};
    use std::path::Path;
    use std::sync::Arc;
    use std::time::SystemTime;
    use tokio::signal;
    use tokio::sync::RwLock;

    /// Default paths for daemon state and logs
    const DEFAULT_STATE_PATH: &str = "/tmp/adasa_state.json";
    const DEFAULT_LOG_DIR: &str = "/tmp/adasa_logs";
    const DEFAULT_SOCKET_PATH: &str = "/tmp/adasa.sock";

    /// Main daemon struct that coordinates all components
    pub struct Daemon {
        /// Process manager for lifecycle management
        process_manager: Arc<RwLock<ProcessManager>>,
        /// State store for persistence
        state_store: StateStore,
        /// Log manager for capturing process output
        log_manager: Arc<RwLock<LogManager>>,
        /// IPC server for client communication
        ipc_server: IpcServer,
        /// Time when daemon was started
        start_time: SystemTime,
    }

    impl Daemon {
        /// Create a new daemon with default paths
        pub async fn new() -> Result<Self> {
            Self::with_paths(DEFAULT_STATE_PATH, DEFAULT_LOG_DIR, DEFAULT_SOCKET_PATH).await
        }

        /// Create a new daemon with custom paths
        pub async fn with_paths<P1, P2, P3>(
            state_path: P1,
            log_dir: P2,
            socket_path: P3,
        ) -> Result<Self>
        where
            P1: AsRef<Path>,
            P2: AsRef<Path>,
            P3: AsRef<Path>,
        {
            let process_manager = Arc::new(RwLock::new(ProcessManager::new()));
            let state_store = StateStore::new(state_path);
            let log_manager = Arc::new(RwLock::new(LogManager::new(log_dir).await?));
            let ipc_server = IpcServer::with_socket_path(socket_path);

            Ok(Self {
                process_manager,
                state_store,
                log_manager,
                ipc_server,
                start_time: SystemTime::now(),
            })
        }

        /// Initialize the daemon and restore previous state
        pub async fn initialize(&mut self) -> Result<()> {
            // Load previous state
            let state = self.state_store.load()?;

            // Restore processes from state
            if !state.processes.is_empty() {
                println!(
                    "Restoring {} processes from previous state...",
                    state.processes.len()
                );

                for persisted in state.processes {
                    // Convert persisted process to config
                    let config = ProcessConfig {
                        name: persisted.name.clone(),
                        script: persisted.script,
                        args: persisted.args,
                        cwd: persisted.cwd,
                        env: persisted.env,
                        instances: persisted.instances,
                        autorestart: persisted.autorestart,
                        max_restarts: persisted.max_restarts,
                        restart_delay_secs: 1,
                        max_memory: None,
                        max_cpu: None,
                        limit_action: adasa::config::LimitAction::Log,
                        stop_signal: "SIGTERM".to_string(),
                        stop_timeout_secs: 10,
                    };

                    // Spawn the process
                    match self.spawn_process(config).await {
                        Ok(id) => {
                            println!("Restored process: {} (ID: {})", persisted.name, id);
                        }
                        Err(e) => {
                            eprintln!("Failed to restore process {}: {}", persisted.name, e);
                        }
                    }
                }
            }

            // Start IPC server
            self.ipc_server.start()?;
            println!(
                "IPC server listening on: {}",
                self.ipc_server.socket_path().display()
            );

            Ok(())
        }

        /// Start the daemon and run the main event loop
        pub async fn start(mut self) -> Result<()> {
            println!("Starting Adasa daemon...");

            // Initialize daemon
            self.initialize().await?;

            println!("Daemon started successfully");

            // Extract fields we need for the event loop
            let process_manager = self.process_manager;
            let log_manager = self.log_manager;
            let state_store = self.state_store;
            let ipc_server = self.ipc_server;
            let start_time = self.start_time;

            // Spawn supervisor task for monitoring and auto-restart
            let pm = Arc::clone(&process_manager);
            tokio::spawn(async move {
                Self::supervisor_loop(pm).await;
            });

            // Spawn stats update task
            let pm = Arc::clone(&process_manager);
            tokio::spawn(async move {
                Self::stats_update_loop(pm).await;
            });

            // Setup signal handlers
            let shutdown_signal = Self::setup_signal_handlers();

            // Run IPC server loop
            let pm = Arc::clone(&process_manager);
            let lm = Arc::clone(&log_manager);

            let server_handle = tokio::spawn(async move {
                let result = ipc_server
                    .run(move |cmd| {
                        let pm = Arc::clone(&pm);
                        let lm = Arc::clone(&lm);
                        async move { Self::handle_command(cmd, pm, lm, start_time).await }
                    })
                    .await;

                if let Err(e) = result {
                    eprintln!("IPC server error: {}", e);
                }
            });

            // Wait for shutdown signal
            shutdown_signal.await;

            println!("Received shutdown signal, stopping daemon...");

            // Abort server task
            server_handle.abort();

            // Perform graceful shutdown
            Self::shutdown_components(process_manager, log_manager, state_store).await?;

            println!("Daemon stopped");

            Ok(())
        }

        /// Handle a command from a client
        async fn handle_command(
            command: Command,
            process_manager: Arc<RwLock<ProcessManager>>,
            log_manager: Arc<RwLock<LogManager>>,
            start_time: SystemTime,
        ) -> Result<Response> {
            match command {
                Command::StartFromConfig { config_path } => {
                    // Load and validate configuration file
                    let configs = ProcessConfig::from_file(&config_path)?;

                    let mut pm = process_manager.write().await;
                    let mut lm = log_manager.write().await;
                    let mut spawned_count = 0;
                    let mut failed_count = 0;

                    // Spawn all processes from config
                    for config in configs {
                        let instances = config.instances;
                        let base_name = config.name.clone();

                        for instance_num in 0..instances {
                            // Generate unique name for each instance
                            let instance_name = if instances > 1 {
                                format!("{}-{}", base_name, instance_num)
                            } else {
                                base_name.clone()
                            };

                            // Create instance-specific config
                            let mut instance_config = config.clone();
                            instance_config.name = instance_name.clone();
                            instance_config.instances = 1;

                            // Spawn the process
                            match pm.spawn(instance_config).await {
                                Ok(id) => {
                                    // Create logger for the process
                                    if let Err(e) = lm.create_logger(id.as_u64(), &instance_name).await {
                                        eprintln!("Failed to create logger for {}: {}", instance_name, e);
                                        continue;
                                    }

                                    // Capture logs from the process
                                    if let Some(process) = pm.get_mut(id) {
                                        if let Err(e) = lm
                                            .capture_logs(id.as_u64(), &instance_name, &mut process.child)
                                            .await
                                        {
                                            eprintln!("Failed to capture logs for {}: {}", instance_name, e);
                                        }
                                    }

                                    spawned_count += 1;
                                }
                                Err(e) => {
                                    eprintln!("Failed to spawn {}: {}", instance_name, e);
                                    failed_count += 1;
                                }
                            }
                        }
                    }

                    if spawned_count == 0 {
                        return Err(AdasaError::SpawnError(
                            "Failed to spawn any processes from config file".to_string(),
                        ));
                    }

                    let message = if failed_count > 0 {
                        format!(
                            "Started {} processes from config ({} failed)",
                            spawned_count, failed_count
                        )
                    } else {
                        format!("Started {} processes from config", spawned_count)
                    };

                    Ok(Response::success(0, ResponseData::Success(message)))
                }

                Command::ReloadConfig { config_path } => {
                    // Load and validate configuration file
                    let configs = ProcessConfig::from_file(&config_path)?;

                    let mut pm = process_manager.write().await;
                    let mut lm = log_manager.write().await;
                    let mut updated_count = 0;
                    let mut added_count = 0;

                    // Process each config
                    for config in configs {
                        let base_name = config.name.clone();
                        
                        // Check if process with this name already exists
                        if pm.find_by_name(&base_name).is_some() {
                            // Update existing process configuration without restarting
                            // For now, we'll just track that we found it
                            // In a full implementation, we'd update the config and optionally restart
                            updated_count += 1;
                            println!("Process {} already exists, configuration noted", base_name);
                        } else {
                            // Spawn new process
                            let instances = config.instances;
                            
                            for instance_num in 0..instances {
                                let instance_name = if instances > 1 {
                                    format!("{}-{}", base_name, instance_num)
                                } else {
                                    base_name.clone()
                                };

                                let mut instance_config = config.clone();
                                instance_config.name = instance_name.clone();
                                instance_config.instances = 1;

                                match pm.spawn(instance_config).await {
                                    Ok(id) => {
                                        if let Err(e) = lm.create_logger(id.as_u64(), &instance_name).await {
                                            eprintln!("Failed to create logger for {}: {}", instance_name, e);
                                            continue;
                                        }

                                        if let Some(process) = pm.get_mut(id) {
                                            if let Err(e) = lm
                                                .capture_logs(id.as_u64(), &instance_name, &mut process.child)
                                                .await
                                            {
                                                eprintln!("Failed to capture logs for {}: {}", instance_name, e);
                                            }
                                        }

                                        added_count += 1;
                                    }
                                    Err(e) => {
                                        eprintln!("Failed to spawn {}: {}", instance_name, e);
                                    }
                                }
                            }
                        }
                    }

                    let message = format!(
                        "Config reloaded: {} processes added, {} existing processes found",
                        added_count, updated_count
                    );

                    Ok(Response::success(0, ResponseData::Success(message)))
                }

                Command::Start(options) => {
                    let base_name = options.name.unwrap_or_else(|| {
                        options
                            .script
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("process")
                            .to_string()
                    });

                    let instances = options.instances;

                    // Spawn multiple instances if requested
                    let mut pm = process_manager.write().await;
                    let mut lm = log_manager.write().await;
                    let mut spawned_ids = Vec::new();

                    for instance_num in 0..instances {
                        // Generate unique name for each instance
                        let instance_name = if instances > 1 {
                            format!("{}-{}", base_name, instance_num)
                        } else {
                            base_name.clone()
                        };

                        // Create process config for this instance
                        let config = ProcessConfig {
                            name: instance_name.clone(),
                            script: options.script.clone(),
                            args: options.args.clone(),
                            cwd: options.cwd.clone(),
                            env: options.env.clone(),
                            instances: 1, // Each spawned process is a single instance
                            autorestart: true,
                            max_restarts: 10,
                            restart_delay_secs: 1,
                            max_memory: None,
                            max_cpu: None,
                            limit_action: adasa::config::LimitAction::Log,
                            stop_signal: "SIGTERM".to_string(),
                            stop_timeout_secs: 10,
                        };

                        // Spawn the process
                        match pm.spawn(config).await {
                            Ok(id) => {
                                // Create logger for the process
                                if let Err(e) = lm.create_logger(id.as_u64(), &instance_name).await
                                {
                                    eprintln!(
                                        "Failed to create logger for {}: {}",
                                        instance_name, e
                                    );
                                    continue;
                                }

                                // Capture logs from the process
                                if let Some(process) = pm.get_mut(id) {
                                    if let Err(e) = lm
                                        .capture_logs(
                                            id.as_u64(),
                                            &instance_name,
                                            &mut process.child,
                                        )
                                        .await
                                    {
                                        eprintln!(
                                            "Failed to capture logs for {}: {}",
                                            instance_name, e
                                        );
                                    }
                                }

                                spawned_ids.push(id);
                            }
                            Err(e) => {
                                eprintln!("Failed to spawn instance {}: {}", instance_name, e);
                                // Continue spawning other instances
                            }
                        }
                    }

                    // Return success if at least one instance was spawned
                    if spawned_ids.is_empty() {
                        return Err(AdasaError::SpawnError(format!(
                            "Failed to spawn any instances of {}",
                            base_name
                        )));
                    }

                    // Return the first ID and base name
                    let id = spawned_ids[0];
                    let response_name = if instances > 1 {
                        format!("{} ({} instances)", base_name, spawned_ids.len())
                    } else {
                        base_name
                    };

                    Ok(Response::success(
                        0,
                        ResponseData::Started {
                            id,
                            name: response_name,
                        },
                    ))
                }

                Command::Stop(options) => {
                    let mut pm = process_manager.write().await;
                    pm.stop(options.id, options.force).await?;

                    Ok(Response::success(
                        0,
                        ResponseData::Stopped { id: options.id },
                    ))
                }

                Command::Restart(options) => {
                    let mut pm = process_manager.write().await;

                    if options.rolling {
                        // Perform rolling restart
                        let health_check_delay = std::time::Duration::from_secs(3);
                        let count = pm
                            .rolling_restart(&options.target, health_check_delay)
                            .await?;

                        Ok(Response::success(
                            0,
                            ResponseData::Success(format!(
                                "Rolling restart completed: {} instances restarted",
                                count
                            )),
                        ))
                    } else {
                        // Try to parse as ProcessId first
                        let id = if let Ok(id_num) = options.target.parse::<u64>() {
                            ProcessId::new(id_num)
                        } else {
                            // Find by name
                            let process = pm.find_by_name(&options.target).ok_or_else(|| {
                                AdasaError::ProcessNotFound(options.target.clone())
                            })?;
                            process.id
                        };

                        pm.restart(id).await?;

                        Ok(Response::success(0, ResponseData::Restarted { id }))
                    }
                }

                Command::List => {
                    let pm = process_manager.read().await;
                    let processes = pm.list();

                    let process_list: Vec<ProcessInfo> = processes
                        .iter()
                        .map(|p| {
                            let stats = adasa::ipc::protocol::ProcessStats {
                                pid: Some(p.stats.pid),
                                uptime: p.stats.uptime(),
                                restarts: p.stats.restarts,
                                cpu_usage: p.stats.cpu_usage,
                                memory_usage: p.stats.memory_usage,
                                last_restart: p.stats.last_restart,
                            };

                            let state = match p.state {
                                ProcState::Starting => adasa::ipc::protocol::ProcessState::Starting,
                                ProcState::Running => adasa::ipc::protocol::ProcessState::Running,
                                ProcState::Stopping => adasa::ipc::protocol::ProcessState::Stopping,
                                ProcState::Stopped => adasa::ipc::protocol::ProcessState::Stopped,
                                ProcState::Errored => adasa::ipc::protocol::ProcessState::Errored,
                            };

                            ProcessInfo {
                                id: p.id,
                                name: p.name.clone(),
                                state,
                                stats,
                            }
                        })
                        .collect();

                    Ok(Response::success(
                        0,
                        ResponseData::ProcessList(process_list),
                    ))
                }

                Command::Logs(options) => {
                    let pm = process_manager.read().await;
                    let process = pm
                        .get_status(options.id)
                        .ok_or_else(|| AdasaError::ProcessNotFound(options.id.to_string()))?;

                    let lm = log_manager.read().await;

                    if options.follow {
                        // For streaming logs, we'll return a message indicating streaming is not yet implemented
                        Ok(Response::success(
                            0,
                            ResponseData::Success("Log streaming not yet implemented".to_string()),
                        ))
                    } else {
                        // Read last N lines
                        let log_options = adasa::logs::LogReadOptions {
                            lines: options.lines.unwrap_or(100),
                            include_stderr: true,
                            include_stdout: true,
                            filter: None,
                        };

                        let entries = lm
                            .read_logs(options.id.as_u64(), &process.name, &log_options)
                            .await?;

                        let log_lines: Vec<String> =
                            entries.iter().map(|entry| entry.format()).collect();

                        Ok(Response::success(0, ResponseData::Logs(log_lines)))
                    }
                }

                Command::Delete(options) => {
                    let mut pm = process_manager.write().await;
                    let mut lm = log_manager.write().await;

                    // Try to parse as ProcessId first, otherwise treat as name
                    let processes_to_delete: Vec<(ProcessId, String)> = if let Ok(id_num) = options.target.parse::<u64>() {
                        let id = ProcessId::new(id_num);
                        // Single process by ID
                        let process_name = pm
                            .get_status(id)
                            .map(|p| p.name.clone())
                            .ok_or_else(|| AdasaError::ProcessNotFound(options.target.clone()))?;
                        vec![(id, process_name)]
                    } else {
                        // All instances by name
                        let instances = pm.find_all_by_name(&options.target);
                        if instances.is_empty() {
                            return Err(AdasaError::ProcessNotFound(options.target.clone()));
                        }
                        instances.iter().map(|p| (p.id, p.name.clone())).collect()
                    };

                    let mut deleted_count = 0;
                    let mut first_id = None;

                    for (process_id, process_name) in processes_to_delete {
                        if first_id.is_none() {
                            first_id = Some(process_id);
                        }

                        // Stop the process first if it's running
                        if let Some(process) = pm.get_status(process_id) {
                            if process.state != ProcState::Stopped {
                                if let Err(e) = pm.stop(process_id, true).await {
                                    tracing::error!("Failed to stop process {} before deletion: {}", process_name, e);
                                    continue;
                                }
                            }
                        }

                        // Remove from process manager
                        if let Err(e) = pm.remove(process_id) {
                            tracing::error!("Failed to remove process {}: {}", process_name, e);
                            continue;
                        }

                        // Delete log files
                        if let Err(e) = lm.delete_logs(process_id.as_u64(), &process_name).await {
                            tracing::warn!("Failed to delete log files for process {}: {}", process_name, e);
                        }

                        deleted_count += 1;
                        tracing::info!("Deleted process: {} (ID: {})", process_name, process_id);
                    }

                    if deleted_count == 0 {
                        return Err(AdasaError::Other(format!(
                            "Failed to delete any processes matching '{}'",
                            options.target
                        )));
                    }

                    let message = if deleted_count == 1 {
                        ResponseData::Deleted { id: first_id.unwrap() }
                    } else {
                        ResponseData::Success(format!("Deleted {} processes", deleted_count))
                    };

                    Ok(Response::success(0, message))
                }

                Command::Daemon(daemon_cmd) => {
                    use adasa::ipc::protocol::DaemonCommand;

                    match daemon_cmd {
                        DaemonCommand::Status => {
                            let uptime = SystemTime::now()
                                .duration_since(start_time)
                                .unwrap_or_default();

                            Ok(Response::success(
                                0,
                                ResponseData::DaemonStatus {
                                    running: true,
                                    uptime,
                                },
                            ))
                        }
                        DaemonCommand::Stop => {
                            // Signal shutdown (this would need to be coordinated with main loop)
                            Ok(Response::success(
                                0,
                                ResponseData::Success("Shutdown initiated".to_string()),
                            ))
                        }
                        DaemonCommand::Start => Ok(Response::success(
                            0,
                            ResponseData::Success("Daemon already running".to_string()),
                        )),
                    }
                }
            }
        }

        /// Supervisor loop that monitors processes and handles auto-restart (optimized)
        async fn supervisor_loop(process_manager: Arc<RwLock<ProcessManager>>) {
            // Use 500ms interval for faster crash detection while still being efficient
            let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(500));
            // Skip first tick to avoid immediate execution
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                interval.tick().await;

                // Use a shorter-lived write lock to reduce contention
                let crashed = {
                    let mut pm = process_manager.write().await;
                    pm.detect_crashes()
                };

                // If no crashes, continue immediately without holding lock
                if crashed.is_empty() {
                    continue;
                }

                // Only acquire write lock again if we need to restart processes
                let mut pm = process_manager.write().await;
                
                // Attempt to restart crashed processes
                for process_id in crashed {
                    match pm.try_auto_restart(process_id).await {
                        Ok(true) => {
                            tracing::info!("Auto-restarted process: {}", process_id);
                        }
                        Ok(false) => {
                            tracing::debug!("Process {} not restarted (policy prevented it)", process_id);
                        }
                        Err(e) => {
                            tracing::error!("Failed to auto-restart process {}: {}", process_id, e);
                        }
                    }
                }
            }
        }

        /// Stats update loop that periodically updates process statistics (optimized)
        async fn stats_update_loop(process_manager: Arc<RwLock<ProcessManager>>) {
            // Use 2-second interval for more responsive stats while being efficient
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(2));
            // Skip missed ticks to avoid backlog if system is busy
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                interval.tick().await;

                // Use shorter-lived lock for stats update
                let result = {
                    let mut pm = process_manager.write().await;
                    pm.update_stats()
                };
                
                if let Err(e) = result {
                    tracing::warn!("Failed to update stats: {}", e);
                }
            }
        }

        /// Setup signal handlers for graceful shutdown
        async fn setup_signal_handlers() -> tokio::sync::oneshot::Receiver<()> {
            let (tx, rx) = tokio::sync::oneshot::channel();

            tokio::spawn(async move {
                let mut sigterm = signal::unix::signal(signal::unix::SignalKind::terminate())
                    .expect("Failed to setup SIGTERM handler");
                let mut sigint = signal::unix::signal(signal::unix::SignalKind::interrupt())
                    .expect("Failed to setup SIGINT handler");

                tokio::select! {
                    _ = sigterm.recv() => {
                        println!("Received SIGTERM");
                    }
                    _ = sigint.recv() => {
                        println!("Received SIGINT");
                    }
                }

                let _ = tx.send(());
            });

            rx
        }

        /// Perform graceful shutdown of daemon components
        async fn shutdown_components(
            process_manager: Arc<RwLock<ProcessManager>>,
            log_manager: Arc<RwLock<LogManager>>,
            state_store: StateStore,
        ) -> Result<()> {
            tracing::info!("Shutting down daemon gracefully...");

            // Stop all processes gracefully
            let mut pm = process_manager.write().await;
            if let Err(e) = pm.stop_all().await {
                tracing::error!("Error during graceful shutdown of processes: {}", e);
            }

            // Save state
            let state = Self::build_state_from_manager(&pm).await;
            if let Err(e) = state_store.save(&state) {
                tracing::error!("Failed to save state: {}", e);
            } else {
                tracing::info!("Daemon state saved successfully");
            }

            // Flush all logs
            let mut lm = log_manager.write().await;
            if let Err(e) = lm.flush_all().await {
                tracing::error!("Failed to flush logs: {}", e);
            } else {
                tracing::info!("All logs flushed successfully");
            }

            tracing::info!("Daemon shutdown complete");

            Ok(())
        }

        /// Build daemon state from current process manager state
        async fn build_state_from_manager(pm: &ProcessManager) -> DaemonState {
            let processes: Vec<PersistedProcess> = pm
                .list()
                .iter()
                .map(|p| PersistedProcess {
                    id: p.id,
                    name: p.name.clone(),
                    script: p.config.script.clone(),
                    args: p.config.args.clone(),
                    cwd: p.config.cwd.clone(),
                    env: p.config.env.clone(),
                    state: match p.state {
                        ProcState::Starting => adasa::ipc::protocol::ProcessState::Starting,
                        ProcState::Running => adasa::ipc::protocol::ProcessState::Running,
                        ProcState::Stopping => adasa::ipc::protocol::ProcessState::Stopping,
                        ProcState::Stopped => adasa::ipc::protocol::ProcessState::Stopped,
                        ProcState::Errored => adasa::ipc::protocol::ProcessState::Errored,
                    },
                    stats: adasa::ipc::protocol::ProcessStats {
                        pid: Some(p.stats.pid),
                        uptime: p.stats.uptime(),
                        restarts: p.stats.restarts,
                        cpu_usage: p.stats.cpu_usage,
                        memory_usage: p.stats.memory_usage,
                        last_restart: p.stats.last_restart,
                    },
                    autorestart: p.config.autorestart,
                    max_restarts: p.config.max_restarts,
                    instances: p.config.instances,
                })
                .collect();

            DaemonState {
                version: "1.0.0".to_string(),
                processes,
                last_updated: SystemTime::now(),
            }
        }

        /// Helper method to spawn a process (used during initialization)
        async fn spawn_process(&self, config: ProcessConfig) -> Result<ProcessId> {
            let name = config.name.clone();

            let mut pm = self.process_manager.write().await;
            let id = pm.spawn(config).await?;

            // Create logger
            let mut lm = self.log_manager.write().await;
            lm.create_logger(id.as_u64(), &name).await?;

            // Capture logs
            if let Some(process) = pm.get_mut(id) {
                lm.capture_logs(id.as_u64(), &name, &mut process.child)
                    .await?;
            }

            Ok(id)
        }
    }
}

use daemon_core::Daemon;
use std::env;

#[tokio::main]
async fn main() -> adasa::error::Result<()> {
    use adasa::daemon::{daemonize, DaemonManager};

    // Check if we should daemonize
    let should_daemonize = env::args().any(|arg| arg == "--daemonize");

    if should_daemonize {
        // Daemonize the process
        daemonize()?;
    }

    // Create daemon manager
    let daemon_manager = DaemonManager::new();

    // Register daemon (write PID file)
    daemon_manager.register_daemon()?;

    // Setup cleanup on exit
    let daemon_manager_clone = DaemonManager::new();
    ctrlc::set_handler(move || {
        let _ = daemon_manager_clone.unregister_daemon();
        std::process::exit(0);
    })
    .ok();

    // Create and start the daemon
    let daemon = Daemon::new().await?;
    let result = daemon.start().await;

    // Unregister daemon on exit
    daemon_manager.unregister_daemon()?;

    result
}
