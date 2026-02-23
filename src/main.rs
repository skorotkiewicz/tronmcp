mod course;
mod game;
mod manager;
mod mcp;
mod web;

use clap::{Parser, Subcommand};
use manager::{GameManager, SharedGameManager};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::Mutex;

use crate::game::SteerAction;

#[derive(Parser)]
#[command(name = "tronmcp", about = "Tron Light-Cycle MCP Game for LLMs")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the game server with web UI
    Serve {
        /// HTTP port for the web UI
        #[arg(long, default_value = "3000")]
        port: u16,
        /// TCP port for MCP player connections
        #[arg(long, default_value = "9999")]
        tcp_port: u16,
        /// Game tick interval in milliseconds
        #[arg(long, default_value = "500")]
        tick_ms: u64,
    },
    /// Connect as an MCP player (stdio mode for LLM agents)
    Play {
        /// Game server address
        #[arg(long, default_value = "127.0.0.1:9999")]
        server: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Serve {
            port,
            tcp_port,
            tick_ms,
        } => {
            run_server(port, tcp_port, tick_ms).await?;
        }
        Commands::Play { server } => {
            mcp::run_mcp_server(server).await?;
        }
    }

    Ok(())
}

async fn run_server(
    http_port: u16,
    tcp_port: u16,
    tick_ms: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let (manager, _rx) = GameManager::new();
    let shared: SharedGameManager = Arc::new(Mutex::new(manager));

    // Spawn game tick loop
    let tick_manager = shared.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(tick_ms));
        loop {
            interval.tick().await;
            let mut mgr = tick_manager.lock().await;
            mgr.tick_all();
        }
    });

    // Spawn TCP command server for MCP players
    let tcp_manager = shared.clone();
    tokio::spawn(async move {
        if let Err(e) = run_tcp_server(tcp_port, tcp_manager).await {
            tracing::error!("TCP server error: {}", e);
        }
    });

    // Start HTTP web UI
    let app = web::create_router(shared.clone());
    let addr = format!("0.0.0.0:{}", http_port);
    tracing::info!("Tron MCP server starting!");
    tracing::info!("Web UI: http://localhost:{}", http_port);
    tracing::info!("TCP command server: 0.0.0.0:{}", tcp_port);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// TCP command server — handles commands from MCP player instances
async fn run_tcp_server(
    port: u16,
    manager: SharedGameManager,
) -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    tracing::info!("TCP command server listening on port {}", port);

    loop {
        let (stream, addr) = listener.accept().await?;
        tracing::info!("MCP player connected from {}", addr);
        let mgr = manager.clone();

        tokio::spawn(async move {
            let (reader, mut writer) = stream.into_split();
            let mut buf_reader = BufReader::new(reader);
            let mut line = String::new();

            loop {
                line.clear();
                match buf_reader.read_line(&mut line).await {
                    Ok(0) => break, // Connection closed
                    Ok(_) => {
                        let response = handle_command(line.trim(), &mgr).await;
                        let response_line = response.replace('\n', "\\n");
                        if let Err(e) =
                            writer.write_all(format!("{}\n", response_line).as_bytes()).await
                        {
                            tracing::error!("Write error: {}", e);
                            break;
                        }
                        let _ = writer.flush().await;
                    }
                    Err(e) => {
                        tracing::error!("Read error: {}", e);
                        break;
                    }
                }
            }

            tracing::info!("MCP player disconnected from {}", addr);
        });
    }
}

/// Handle a single TCP command from an MCP player
async fn handle_command(cmd: &str, manager: &SharedGameManager) -> String {
    let parts: Vec<&str> = cmd.splitn(3, ' ').collect();

    if parts.is_empty() {
        return "ERROR: Empty command".to_string();
    }

    match parts[0].to_uppercase().as_str() {
        "JOIN" => {
            if parts.len() < 2 {
                return "ERROR: JOIN requires a name".to_string();
            }
            let name = parts[1..].join(" ");
            let mut mgr = manager.lock().await;
            match mgr.join(name) {
                Ok(msg) => msg,
                Err(e) => format!("ERROR: {}", e),
            }
        }
        "LOOK" => {
            if parts.len() < 2 {
                return "ERROR: LOOK requires player name".to_string();
            }
            let mgr = manager.lock().await;
            match mgr.look(parts[1]) {
                Ok(msg) => msg,
                Err(e) => format!("ERROR: {}", e),
            }
        }
        "STEER" => {
            if parts.len() < 3 {
                return "ERROR: STEER requires player name and direction".to_string();
            }
            let action = match parts[2].to_lowercase().as_str() {
                "left" => SteerAction::Left,
                "right" => SteerAction::Right,
                "straight" => SteerAction::Straight,
                _ => return "ERROR: Direction must be left, right, or straight".to_string(),
            };
            let mut mgr = manager.lock().await;
            match mgr.steer(parts[1], action) {
                Ok(msg) => msg,
                Err(e) => format!("ERROR: {}", e),
            }
        }
        "STATUS" => {
            if parts.len() < 2 {
                return "ERROR: STATUS requires player name".to_string();
            }
            let mgr = manager.lock().await;
            match mgr.game_status(parts[1]) {
                Ok(msg) => msg,
                Err(e) => format!("ERROR: {}", e),
            }
        }
        _ => format!("ERROR: Unknown command '{}'", parts[0]),
    }
}
