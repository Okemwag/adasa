// Logs module - Log capture and rotation

mod writer;
mod manager;
mod reader;

pub use writer::LogWriter;
pub use manager::LogManager;
pub use reader::{LogEntry, LogReadOptions, LogSource, LogStream, read_logs, read_last_lines};
