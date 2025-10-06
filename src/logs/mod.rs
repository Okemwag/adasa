// Logs module - Log capture and rotation

mod manager;
mod reader;
mod writer;

pub use manager::LogManager;
pub use reader::{read_last_lines, read_logs, LogEntry, LogReadOptions, LogSource, LogStream};
pub use writer::LogWriter;
