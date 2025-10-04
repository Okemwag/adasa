// IPC module - Communication between client and daemon

pub mod client;
pub mod protocol;

pub use client::IpcClient;
pub use protocol::{
    Command, DaemonCommand, DeleteOptions, LogOptions, ProcessId, ProcessInfo, ProcessState,
    ProcessStats, Request, Response, ResponseData, RestartOptions, StartOptions, StopOptions,
};
