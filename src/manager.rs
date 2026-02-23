use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use uuid::Uuid;

use crate::course::{all_courses, get_course};
use crate::game::{Game, GameStatus, SteerAction, WebGameState};

/// Leaderboard entry
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LeaderboardEntry {
    pub name: String,
    pub wins: u32,
    pub total_points: u32,
    pub games_played: u32,
    pub highest_level: u32,
}

/// Player session — tracks which game a connected player is in
#[derive(Debug, Clone)]
pub struct PlayerSession {
    pub game_id: Option<Uuid>,
    pub player_index: Option<usize>,
    pub current_level: u32,
}

/// Central game manager
pub struct GameManager {
    pub active_games: HashMap<Uuid, Game>,
    pub finished_games: Vec<WebGameState>,
    pub leaderboard: HashMap<String, LeaderboardEntry>,
    pub player_sessions: HashMap<String, PlayerSession>,
    pub waiting_players: Vec<String>,
    pub broadcast_tx: broadcast::Sender<String>,
    pub max_finished_games: usize,
    pub max_leaderboard_size: usize,
    pub data_dir: PathBuf,
}

impl GameManager {
    pub fn new(data_dir: impl Into<PathBuf>) -> (Self, broadcast::Receiver<String>) {
        let (tx, rx) = broadcast::channel(256);
        let data_dir = data_dir.into();

        // Create data dir if it doesn't exist
        let _ = std::fs::create_dir_all(&data_dir);

        // Load persisted leaderboard
        let leaderboard = Self::load_leaderboard(&data_dir);
        let finished_games = Self::load_finished_games(&data_dir);

        let manager = GameManager {
            active_games: HashMap::new(),
            finished_games,
            leaderboard,
            player_sessions: HashMap::new(),
            waiting_players: Vec::new(),
            broadcast_tx: tx,
            max_finished_games: 30,
            max_leaderboard_size: 10,
            data_dir,
        };
        (manager, rx)
    }

    fn finished_games_path(data_dir: &Path) -> PathBuf {
        data_dir.join("finished_games.json")
    }

    fn save_finished_games(&self) {
        let path = Self::finished_games_path(&self.data_dir);
        match serde_json::to_string_pretty(&self.finished_games) {
            Ok(json) => {
                if let Err(e) = std::fs::write(&path, json) {
                    tracing::error!("Failed to save finished games: {}", e);
                }
            }
            Err(e) => tracing::error!("Failed to serialize finished games: {}", e),
        }
    }

    fn load_finished_games(data_dir: &Path) -> Vec<WebGameState> {
        let path = Self::finished_games_path(data_dir);
        match std::fs::read_to_string(&path) {
            Ok(json) => {
                match serde_json::from_str::<Vec<WebGameState>>(&json) {
                    Ok(entries) => {
                        tracing::info!("Loaded {} finished games from {}", entries.len(), path.display());
                        entries
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse finished games: {}", e);
                        Vec::new()
                    }
                }
            }
            Err(_) => {
                tracing::info!("No existing finished games at {}, starting fresh", path.display());
                Vec::new()
            }
        }
    }

    fn leaderboard_path(data_dir: &Path) -> PathBuf {
        data_dir.join("leaderboard.json")
    }

    fn load_leaderboard(data_dir: &Path) -> HashMap<String, LeaderboardEntry> {
        let path = Self::leaderboard_path(data_dir);
        match std::fs::read_to_string(&path) {
            Ok(json) => {
                match serde_json::from_str::<Vec<LeaderboardEntry>>(&json) {
                    Ok(entries) => {
                        tracing::info!("Loaded {} leaderboard entries from {}", entries.len(), path.display());
                        entries.into_iter().map(|e| (e.name.clone(), e)).collect()
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse leaderboard: {}", e);
                        HashMap::new()
                    }
                }
            }
            Err(_) => {
                tracing::info!("No existing leaderboard at {}, starting fresh", path.display());
                HashMap::new()
            }
        }
    }

    fn save_leaderboard(&self) {
        let entries = self.get_leaderboard();
        let path = Self::leaderboard_path(&self.data_dir);
        match serde_json::to_string_pretty(&entries) {
            Ok(json) => {
                if let Err(e) = std::fs::write(&path, json) {
                    tracing::error!("Failed to save leaderboard: {}", e);
                }
            }
            Err(e) => tracing::error!("Failed to serialize leaderboard: {}", e),
        }
    }

    /// Register a player and add them to the waiting queue
    pub fn join(&mut self, name: String) -> Result<String, String> {
        if self.player_sessions.contains_key(&name) {
            // Check if their previous game is finished
            let session = self.player_sessions.get(&name).unwrap();
            if let Some(game_id) = session.game_id {
                if let Some(game) = self.active_games.get(&game_id) {
                    if game.status != GameStatus::Finished {
                        return Err(format!(
                            "Player '{}' is already in an active game.",
                            name
                        ));
                    }
                }
            }
        }

        let level = self
            .player_sessions
            .get(&name)
            .map(|s| s.current_level)
            .unwrap_or(1);

        self.player_sessions.insert(
            name.clone(),
            PlayerSession {
                game_id: None,
                player_index: None,
                current_level: level,
            },
        );

        if !self.waiting_players.contains(&name) {
            self.waiting_players.push(name.clone());
        }

        // Try to start a game if we have enough players
        if self.waiting_players.len() >= 2 {
            self.try_start_game();
        }

        Ok(format!(
            "Joined! Waiting for opponents... ({} players in queue)",
            self.waiting_players.len()
        ))
    }

    /// Try to start a game with waiting players
    fn try_start_game(&mut self) {
        if self.waiting_players.len() < 2 {
            return;
        }

        // Determine course level (use the minimum level among waiting players)
        let min_level = self
            .waiting_players
            .iter()
            .filter_map(|name| self.player_sessions.get(name))
            .map(|s| s.current_level)
            .min()
            .unwrap_or(1);

        let course = get_course(min_level);
        let max = course.max_players.min(self.waiting_players.len());

        let players_for_game: Vec<String> = self.waiting_players.drain(..max).collect();

        let mut game = Game::new(&course);

        for name in &players_for_game {
            if let Some(idx) = game.add_player(name.clone()) {
                if let Some(session) = self.player_sessions.get_mut(name) {
                    session.game_id = Some(game.id);
                    session.player_index = Some(idx);
                }
            }
        }

        game.start();

        let game_id = game.id;
        self.active_games.insert(game_id, game);

        let _ = self.broadcast_tx.send(serde_json::json!({
            "type": "game_started",
            "game_id": game_id.to_string(),
        }).to_string());
    }

    /// Move a player: steer + advance one step. Returns result message.
    pub fn move_player(&mut self, player_name: &str, action: SteerAction) -> Result<String, String> {
        let session = self
            .player_sessions
            .get(player_name)
            .ok_or_else(|| "Player not found. Use join_game first.".to_string())?;

        let game_id = session
            .game_id
            .ok_or_else(|| "Not in a game yet. Waiting for opponents.".to_string())?;

        let player_idx = session
            .player_index
            .ok_or_else(|| "Player index not set.".to_string())?;

        let game = self
            .active_games
            .get_mut(&game_id)
            .ok_or_else(|| "Game not found.".to_string())?;

        let result = game.move_player(player_idx, action);

        // Broadcast update
        let _ = self.broadcast_tx.send(serde_json::json!({
            "type": "game_update",
            "game": game.to_web_state(),
        }).to_string());

        // Check if game just finished
        if game.status == GameStatus::Finished {
            self.finish_game(game_id);
        }

        Ok(result)
    }

    /// Get the look view for a player
    pub fn look(&self, player_name: &str) -> Result<String, String> {
        let session = self
            .player_sessions
            .get(player_name)
            .ok_or_else(|| "Player not found. Use join_game first.".to_string())?;

        let game_id = session
            .game_id
            .ok_or_else(|| "Not in a game yet. Waiting for opponents.".to_string())?;

        let player_idx = session
            .player_index
            .ok_or_else(|| "Player index not set.".to_string())?;

        let game = self
            .active_games
            .get(&game_id)
            .ok_or_else(|| "Game not found.".to_string())?;

        Ok(game.look(player_idx, 7))
    }

    /// Get game status for a player
    pub fn game_status(&self, player_name: &str) -> Result<String, String> {
        let session = self
            .player_sessions
            .get(player_name)
            .ok_or_else(|| "Player not found. Use join_game first.".to_string())?;

        if session.game_id.is_none() {
            return Ok(format!(
                "Status: WAITING for game to start. {} players in queue.",
                self.waiting_players.len()
            ));
        }

        let game_id = session.game_id.unwrap();
        let player_idx = session.player_index.unwrap_or(0);

        // Check active games first
        if let Some(game) = self.active_games.get(&game_id) {
            return Ok(self.format_status(game, player_idx));
        }

        // Check finished games
        if let Some(finished) = self
            .finished_games
            .iter()
            .find(|g| g.id == game_id.to_string())
        {
            let mut lines = vec![format!("Status: FINISHED")];
            if let Some(winner_idx) = finished.winner {
                if let Some(wp) = finished.players.get(winner_idx) {
                    lines.push(format!("Winner: {}", wp.name));
                }
            } else {
                lines.push("Result: DRAW (everyone crashed)".to_string());
            }
            if let Some(pp) = finished.players.get(player_idx) {
                lines.push(format!("Your score: {}", pp.score));
            }
            return Ok(lines.join("\n"));
        }

        Ok("Game not found.".to_string())
    }

    fn format_status(&self, game: &Game, player_idx: usize) -> String {
        let mut lines = Vec::new();
        lines.push(format!("Status: {:?}", game.status));
        lines.push(format!(
            "Course: {} (Level {})",
            game.course_name, game.course_level
        ));
        lines.push(format!("Tick: {}", game.tick));

        let alive = game.players.iter().filter(|p| p.alive).count();
        lines.push(format!("Players alive: {}/{}", alive, game.players.len()));

        if let Some(p) = game.players.get(player_idx) {
            lines.push(format!(
                "You: {} at ({}, {}) heading {} — {}",
                p.name,
                p.x,
                p.y,
                p.direction.name(),
                if p.alive { "ALIVE" } else { "CRASHED" }
            ));
            lines.push(format!("Distance: {}", p.distance_traveled));
        }

        if game.status == GameStatus::Finished {
            if let Some(winner_idx) = game.winner {
                let winner = &game.players[winner_idx];
                lines.push(format!("Winner: {} (score: {})", winner.name, winner.score));
                if winner_idx == player_idx {
                    lines.push("Congratulations! You won! Use join_game to play the next level.".to_string());
                }
            } else {
                lines.push("Result: DRAW (everyone crashed)".to_string());
            }
        }

        lines.join("\n")
    }

    /// Handle a game that just finished — update leaderboard, broadcast, archive
    fn finish_game(&mut self, game_id: Uuid) {
        if let Some(game) = self.active_games.remove(&game_id) {
            // Update leaderboard
            for (i, player) in game.players.iter().enumerate() {
                let entry = self
                    .leaderboard
                    .entry(player.name.clone())
                    .or_insert_with(|| LeaderboardEntry {
                        name: player.name.clone(),
                        ..Default::default()
                    });
                entry.games_played += 1;

                if game.winner == Some(i) {
                    entry.wins += 1;
                    entry.total_points += player.score;
                    if game.course_level >= entry.highest_level {
                        entry.highest_level = game.course_level + 1;
                    }

                    // Advance winner's level
                    if let Some(session) = self.player_sessions.get_mut(&player.name) {
                        let max_level = all_courses().len() as u32;
                        if session.current_level < max_level {
                            session.current_level += 1;
                        }
                    }
                }
            }

            let web_state = game.to_web_state();
            let _ = self.broadcast_tx.send(serde_json::json!({
                "type": "game_finished",
                "game": &web_state,
            }).to_string());

            self.finished_games.push(web_state);
            if self.finished_games.len() > self.max_finished_games {
                self.finished_games.remove(0);
            }

            self.save_leaderboard();
            self.save_finished_games();
        }
    }

    /// Get leaderboard sorted by total points
    pub fn get_leaderboard(&self) -> Vec<LeaderboardEntry> {
        let mut entries: Vec<LeaderboardEntry> = self.leaderboard.values().cloned().collect();
        entries.sort_by(|a, b| b.total_points.cmp(&a.total_points));
        entries.truncate(self.max_leaderboard_size);
        entries
    }

    /// Get all active games as web states
    pub fn get_active_games(&self) -> Vec<WebGameState> {
        self.active_games.values().map(|g| g.to_web_state()).collect()
    }

    /// Get finished games
    pub fn get_finished_games(&self) -> &[WebGameState] {
        &self.finished_games
    }
}

pub type SharedGameManager = Arc<Mutex<GameManager>>;
