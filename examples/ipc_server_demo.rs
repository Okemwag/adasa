// Example demonstrating IPC server usage

use adasa::error::Result;
use adasa::ipc::{
    Command, IpcServer, ProcessId, ProcessInfo, ProcessState, ProcessStats, Response, ResponseData,
};

#[tokio::main]
async fn main() -> Result<()> {
    println!("Starting IPC server demo...");

    // Create and start the server
    let mut server = IpcServer::with_socket_path("/tmp/adasa_demo.sock");
    server.start()?;

    println!("Server listening on: {:?}", server.socket_path());
    println!("Press Ctrl+C to stop the server");
    println!("\nYou can test the server with the client by running:");
    println!("  cargo run --example ipc_client_demo");

    // Simple command handler
    let handler = |command: Command| async move {
        println!("Received command: {:?}", command);

        match command {
            Command::List => {
                // Return a mock process list
                Ok(Response::success(
                    1,
                    ResponseData::ProcessList(vec![
                        ProcessInfo {
                            id: ProcessId::new(1),
                            name: "demo-process-1".to_string(),
                            state: ProcessState::Running,
                            stats: ProcessStats::default(),
                        },
                        ProcessInfo {
                            id: ProcessId::new(2),
                            name: "demo-process-2".to_string(),
                            state: ProcessState::Running,
                            stats: ProcessStats::default(),
                        },
                    ]),
                ))
            }
            Command::Start(opts) => {
                println!("Starting process: {:?}", opts.script);
                Ok(Response::success(
                    1,
                    ResponseData::Started {
                        id: ProcessId::new(3),
                        name: opts.name.unwrap_or_else(|| "new-process".to_string()),
                    },
                ))
            }
            Command::Stop(opts) => {
                println!("Stopping process: {}", opts.id);
                Ok(Response::success(
                    1,
                    ResponseData::Stopped { id: opts.id },
                ))
            }
            Command::Restart(opts) => {
                println!("Restarting process: {}", opts.target);
                Ok(Response::success(
                    1,
                    ResponseData::Restarted { id: ProcessId::new(1) },
                ))
            }
            _ => Ok(Response::success(
                1,
                ResponseData::Success("Command received".to_string()),
            )),
        }
    };

    // Run the server
    println!("\nServer is ready to accept connections...\n");
    server.run(handler).await?;

    Ok(())
}
