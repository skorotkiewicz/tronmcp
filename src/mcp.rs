use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    tool, tool_handler, tool_router,
    transport::stdio,
    ErrorData as McpError, ServerHandler, ServiceExt,
};
use rmcp::schemars;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::sync::Mutex;

use crate::game::SteerAction;
use crate::manager::SharedGameManager;

/// Parameters for join_game tool
#[derive(Debug, Deserialize, JsonSchema)]
pub struct JoinGameParams {
    /// Your display name for the game
    pub name: String,
}

/// Parameters for steer tool
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SteerParams {
    /// Direction to steer: "left", "right", or "straight"
    pub direction: String,
}

// ─── Shared MCP tool descriptions ───

const INSTRUCTIONS: &str = "Tron Light-Cycle MCP Game! You control a light-cycle on a grid. \
Your cycle does NOT move automatically — each 'steer' call moves you one step forward. \
You choose direction (left/right/straight) and it moves one cell that way. \
Crash into anything (walls, trails, obstructions) and you lose. \
Last cycle standing wins!\n\n\
Tools:\n\
1. join_game(name) - Join a game with your name\n\
2. look() - See the grid around you (call before every steer!)\n\
3. steer(direction) - Turn + move one step: 'left', 'right', or 'straight'\n\
4. game_status() - Check game outcome and scores\n\n\
Strategy: Always call 'look' first, then 'steer' to move. Repeat. \
Each steer = one grid step. Longer distance = more points.";

// ─── TCP-backed MCP Server (for `tronmcp play` stdio mode) ───

#[derive(Clone)]
pub struct TronMcpServer {
    tool_router: ToolRouter<Self>,
    conn: std::sync::Arc<Mutex<TcpStream>>,
    player_name: std::sync::Arc<Mutex<Option<String>>>,
}

impl TronMcpServer {
    pub fn new(server_addr: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let stream = TcpStream::connect(server_addr)?;
        stream.set_nodelay(true)?;
        Ok(Self {
            tool_router: Self::tool_router(),
            conn: std::sync::Arc::new(Mutex::new(stream)),
            player_name: std::sync::Arc::new(Mutex::new(None)),
        })
    }

    fn send_command(&self, cmd: &str) -> Result<String, McpError> {
        let mut conn = self.conn.lock().map_err(|e| {
            McpError::internal_error(format!("Lock error: {}", e), None)
        })?;
        writeln!(&mut *conn, "{}", cmd).map_err(|e| {
            McpError::internal_error(format!("Write error: {}", e), None)
        })?;
        conn.flush().map_err(|e| {
            McpError::internal_error(format!("Flush error: {}", e), None)
        })?;
        let mut reader = BufReader::new(&mut *conn);
        let mut response = String::new();
        let bytes_read = reader.read_line(&mut response).map_err(|e| {
            McpError::internal_error(format!("Read error: {}", e), None)
        })?;
        if bytes_read == 0 {
            return Err(McpError::internal_error("Connection closed by server.", None));
        }
        let trimmed = response.trim().to_string();
        if trimmed.is_empty() {
            return Err(McpError::internal_error("Empty response from server.", None));
        }
        Ok(trimmed)
    }
}

#[tool_router]
impl TronMcpServer {
    #[tool(description = "Join the next available Tron light-cycle game. You will be matched with other players. Once the game starts, use 'look' to see the grid and 'steer' to move. Your light-cycle does NOT move automatically — each 'steer' call moves you one step.")]
    fn join_game(&self, Parameters(params): Parameters<JoinGameParams>) -> Result<CallToolResult, McpError> {
        let name = params.name.trim().to_string();
        if name.is_empty() { return Ok(CallToolResult::error(vec![Content::text("Name cannot be empty.")])); }
        *self.player_name.lock().map_err(|e| McpError::internal_error(format!("{}", e), None))? = Some(name.clone());
        let response = self.send_command(&format!("JOIN {}", name))?;
        Ok(CallToolResult::success(vec![Content::text(response)]))
    }

    #[tool(description = "Look at the game grid around your light-cycle. Returns a text map showing your position (@), your trail (|), other players and their trails (1-9), walls (#), obstructions (X), and empty space (.). Use this to plan your moves and avoid collisions!")]
    fn look(&self) -> Result<CallToolResult, McpError> {
        let name = self.player_name.lock().map_err(|e| McpError::internal_error(format!("{}", e), None))?;
        let name = name.as_ref().ok_or_else(|| McpError::invalid_params("Use join_game first.", None))?;
        let response = self.send_command(&format!("LOOK {}", name))?;
        Ok(CallToolResult::success(vec![Content::text(response)]))
    }

    #[tool(description = "Steer your light-cycle and move ONE step forward. Direction must be 'left' (turn left then move), 'right' (turn right then move), or 'straight' (move forward). Each call moves exactly one cell. Call 'look' before each 'steer' to see what's ahead! Crashing into walls, obstructions, or any trail means you lose!")]
    fn steer(&self, Parameters(params): Parameters<SteerParams>) -> Result<CallToolResult, McpError> {
        let name = self.player_name.lock().map_err(|e| McpError::internal_error(format!("{}", e), None))?;
        let name = name.as_ref().ok_or_else(|| McpError::invalid_params("Use join_game first.", None))?;
        let dir = params.direction.to_lowercase();
        if !["left", "right", "straight"].contains(&dir.as_str()) {
            return Ok(CallToolResult::error(vec![Content::text("Direction must be 'left', 'right', or 'straight'.")]));
        }
        let response = self.send_command(&format!("STEER {} {}", name, dir))?;
        Ok(CallToolResult::success(vec![Content::text(response)]))
    }

    #[tool(description = "Get the current game status: whether the game is waiting, running, or finished, your score, the winner, and the leaderboard standings. Use this after the game ends to see results. If you won, use join_game again to play the next level!")]
    fn game_status(&self) -> Result<CallToolResult, McpError> {
        let name = self.player_name.lock().map_err(|e| McpError::internal_error(format!("{}", e), None))?;
        let name = name.as_ref().ok_or_else(|| McpError::invalid_params("Use join_game first.", None))?;
        let response = self.send_command(&format!("STATUS {}", name))?;
        Ok(CallToolResult::success(vec![Content::text(response)]))
    }
}

#[tool_handler]
impl ServerHandler for TronMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(INSTRUCTIONS.into()),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

/// Run the MCP stdio server (for `tronmcp play`)
pub async fn run_mcp_server(server_addr: String) -> Result<(), Box<dyn std::error::Error>> {
    let server = TronMcpServer::new(&server_addr)?;
    tracing::info!("MCP server connected to game server at {}", server_addr);
    let service = server.serve(stdio()).await.inspect_err(|e| {
        tracing::error!("Error starting MCP server: {}", e);
    })?;
    service.waiting().await?;
    Ok(())
}

// ─── HTTP-backed MCP Server (for streamable HTTP transport) ───

/// MCP handler that talks directly to the shared GameManager (no TCP relay)
#[derive(Clone)]
pub struct TronMcpHttpHandler {
    tool_router: ToolRouter<Self>,
    manager: SharedGameManager,
    player_name: std::sync::Arc<tokio::sync::Mutex<Option<String>>>,
}

impl TronMcpHttpHandler {
    pub fn new(manager: SharedGameManager) -> Self {
        Self {
            tool_router: Self::tool_router(),
            manager,
            player_name: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
        }
    }
}

#[tool_router]
impl TronMcpHttpHandler {
    #[tool(description = "Join the next available Tron light-cycle game. You will be matched with other players. Once the game starts, use 'look' to see the grid and 'steer' to move. Your light-cycle does NOT move automatically — each 'steer' call moves you one step.")]
    async fn join_game(&self, Parameters(params): Parameters<JoinGameParams>) -> Result<CallToolResult, McpError> {
        let name = params.name.trim().to_string();
        if name.is_empty() { return Ok(CallToolResult::error(vec![Content::text("Name cannot be empty.")])); }
        *self.player_name.lock().await = Some(name.clone());
        let mut mgr = self.manager.lock().await;
        match mgr.join(name) {
            Ok(msg) => Ok(CallToolResult::success(vec![Content::text(msg)])),
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
        }
    }

    #[tool(description = "Look at the game grid around your light-cycle. Returns a text map showing your position (@), your trail (|), other players and their trails (1-9), walls (#), obstructions (X), and empty space (.). Use this to plan your moves and avoid collisions!")]
    async fn look(&self) -> Result<CallToolResult, McpError> {
        let name = self.player_name.lock().await;
        let name = name.as_ref().ok_or_else(|| McpError::invalid_params("Use join_game first.", None))?;
        let mgr = self.manager.lock().await;
        match mgr.look(name) {
            Ok(msg) => Ok(CallToolResult::success(vec![Content::text(msg)])),
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
        }
    }

    #[tool(description = "Steer your light-cycle and move ONE step forward. Direction must be 'left' (turn left then move), 'right' (turn right then move), or 'straight' (move forward). Each call moves exactly one cell. Call 'look' before each 'steer' to see what's ahead! Crashing into walls, obstructions, or any trail means you lose!")]
    async fn steer(&self, Parameters(params): Parameters<SteerParams>) -> Result<CallToolResult, McpError> {
        let name_guard = self.player_name.lock().await;
        let name = name_guard.as_ref().ok_or_else(|| McpError::invalid_params("Use join_game first.", None))?;
        let dir = params.direction.to_lowercase();
        let action = match dir.as_str() {
            "left" => SteerAction::Left,
            "right" => SteerAction::Right,
            "straight" => SteerAction::Straight,
            _ => return Ok(CallToolResult::error(vec![Content::text("Direction must be 'left', 'right', or 'straight'.")])),
        };
        let mut mgr = self.manager.lock().await;
        match mgr.move_player(name, action) {
            Ok(msg) => Ok(CallToolResult::success(vec![Content::text(msg)])),
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
        }
    }

    #[tool(description = "Get the current game status: whether the game is waiting, running, or finished, your score, the winner, and the leaderboard standings. Use this after the game ends to see results. If you won, use join_game again to play the next level!")]
    async fn game_status(&self) -> Result<CallToolResult, McpError> {
        let name = self.player_name.lock().await;
        let name = name.as_ref().ok_or_else(|| McpError::invalid_params("Use join_game first.", None))?;
        let mgr = self.manager.lock().await;
        match mgr.game_status(name) {
            Ok(msg) => Ok(CallToolResult::success(vec![Content::text(msg)])),
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
        }
    }
}

#[tool_handler]
impl ServerHandler for TronMcpHttpHandler {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(INSTRUCTIONS.into()),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}
