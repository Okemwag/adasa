use thiserror::Error;

/// Main error type for Adasa process manager
#[derive(Debug, Error)]
#[allow(dead_code)]
pub enum AdasaError {
    // Process-related errors
    #[error("Process not found: {0}")]
    ProcessNotFound(String),
    
    #[error("Failed to spawn process: {0}")]
    SpawnError(String),
    
    #[error("Process already exists: {0}")]
    ProcessAlreadyExists(String),
    
    #[error("Failed to stop process {0}: {1}")]
    StopError(String, String),
    
    #[error("Process {0} is in invalid state for this operation: {1}")]
    InvalidProcessState(String, String),
    
    #[error("Process restart limit exceeded for {0}")]
    RestartLimitExceeded(String),
    
    // IPC-related errors
    #[error("IPC error: {0}")]
    IpcError(String),
    
    #[error("Failed to connect to daemon: {0}")]
    ConnectionError(String),
    
    #[error("IPC protocol error: {0}")]
    ProtocolError(String),
    
    #[error("Daemon not running")]
    DaemonNotRunning,
    
    #[error("Daemon already running")]
    DaemonAlreadyRunning,
    
    // State store errors
    #[error("State store error: {0}")]
    StateError(String),
    
    #[error("Failed to load state: {0}")]
    StateLoadError(String),
    
    #[error("Failed to save state: {0}")]
    StateSaveError(String),
    
    #[error("State corruption detected: {0}")]
    StateCorruption(String),
    
    // Configuration errors
    #[error("Configuration error: {0}")]
    ConfigError(String),
    
    #[error("Invalid configuration file: {0}")]
    InvalidConfig(String),
    
    #[error("Missing required configuration field: {0}")]
    MissingConfigField(String),
    
    #[error("Configuration validation failed: {0}")]
    ConfigValidationError(String),
    
    // Log-related errors
    #[error("Log error: {0}")]
    LogError(String),
    
    #[error("Failed to open log file: {0}")]
    LogFileError(String),
    
    #[error("Log rotation failed: {0}")]
    LogRotationError(String),
    
    // Resource-related errors
    #[error("Resource limit error: {0}")]
    ResourceLimitError(String),
    
    #[error("Memory limit exceeded for process {0}")]
    MemoryLimitExceeded(String),
    
    #[error("CPU limit exceeded for process {0}")]
    CpuLimitExceeded(String),
    
    // Permission and security errors
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    
    #[error("Invalid process ID: {0}")]
    InvalidProcessId(String),
    
    // System errors
    #[error("System error: {0}")]
    SystemError(String),
    
    #[error("Signal error: {0}")]
    SignalError(String),
    
    #[error("Timeout error: {0}")]
    TimeoutError(String),
    
    // IO errors (automatically converted from std::io::Error)
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    // Serialization errors
    #[error("Serialization error: {0}")]
    SerializationError(String),
    
    #[error("Deserialization error: {0}")]
    DeserializationError(String),
    
    // Generic errors
    #[error("Internal error: {0}")]
    Internal(String),
    
    #[error("{0}")]
    Other(String),
}

/// Result type alias for Adasa operations
pub type Result<T> = std::result::Result<T, AdasaError>;
