use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use uuid::Uuid;

use crate::course::Course;

/// Cell types on the game grid
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Cell {
    Empty,
    Wall,
    Obstruction,
    Trail(usize), // player index
}

/// Movement direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl Direction {
    pub fn turn_left(self) -> Self {
        match self {
            Direction::Up => Direction::Left,
            Direction::Left => Direction::Down,
            Direction::Down => Direction::Right,
            Direction::Right => Direction::Up,
        }
    }

    pub fn turn_right(self) -> Self {
        match self {
            Direction::Up => Direction::Right,
            Direction::Right => Direction::Down,
            Direction::Down => Direction::Left,
            Direction::Left => Direction::Up,
        }
    }

    pub fn delta(self) -> (i32, i32) {
        match self {
            Direction::Up => (0, -1),
            Direction::Down => (0, 1),
            Direction::Left => (-1, 0),
            Direction::Right => (1, 0),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Direction::Up => "NORTH",
            Direction::Down => "SOUTH",
            Direction::Left => "WEST",
            Direction::Right => "EAST",
        }
    }
}

/// Steering action from an LLM
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SteerAction {
    Left,
    Right,
    Straight,
}

/// A player in the game
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub direction: Direction,
    pub alive: bool,
    pub trail: VecDeque<(i32, i32)>,
    pub distance_traveled: u32,
    pub score: u32,
}

/// Game status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameStatus {
    WaitingForPlayers,
    Running,
    Finished,
}

/// A game instance
#[derive(Debug, Clone, Serialize)]
pub struct Game {
    pub id: Uuid,
    pub grid: Vec<Vec<Cell>>,
    pub width: usize,
    pub height: usize,
    pub players: Vec<Player>,
    pub status: GameStatus,
    pub tick: u32,
    pub max_trail_length: usize,
    pub course_name: String,
    pub course_level: u32,
    pub winner: Option<usize>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub finished_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl Game {
    /// Create a new game from a course definition
    pub fn new(course: &Course) -> Self {
        let mut grid = vec![vec![Cell::Empty; course.width]; course.height];

        // Place walls around the border
        for x in 0..course.width {
            grid[0][x] = Cell::Wall;
            grid[course.height - 1][x] = Cell::Wall;
        }
        for y in 0..course.height {
            grid[y][0] = Cell::Wall;
            grid[y][course.width - 1] = Cell::Wall;
        }

        // Place course obstructions
        for &(x, y) in &course.obstructions {
            if x < course.width && y < course.height {
                grid[y][x] = Cell::Obstruction;
            }
        }

        // Place course walls
        for &(x, y) in &course.walls {
            if x < course.width && y < course.height {
                grid[y][x] = Cell::Wall;
            }
        }

        Game {
            id: Uuid::new_v4(),
            width: course.width,
            height: course.height,
            grid,
            players: Vec::new(),
            status: GameStatus::WaitingForPlayers,
            tick: 0,
            max_trail_length: course.max_trail_length,
            course_name: course.name.clone(),
            course_level: course.level,
            winner: None,
            created_at: chrono::Utc::now(),
            finished_at: None,
        }
    }

    /// Spawn positions for players (corners and midpoints)
    fn spawn_positions(&self) -> Vec<(i32, i32, Direction)> {
        let w = self.width as i32;
        let h = self.height as i32;
        vec![
            (3, 3, Direction::Right),
            (w - 4, h - 4, Direction::Left),
            (w - 4, 3, Direction::Down),
            (3, h - 4, Direction::Up),
            (w / 2, 3, Direction::Down),
            (3, h / 2, Direction::Right),
            (w - 4, h / 2, Direction::Left),
            (w / 2, h - 4, Direction::Up),
        ]
    }

    /// Add a player to the game. Returns player index or None if full.
    pub fn add_player(&mut self, name: String) -> Option<usize> {
        let spawns = self.spawn_positions();
        let idx = self.players.len();
        if idx >= spawns.len() {
            return None;
        }

        let (x, y, dir) = spawns[idx];
        self.players.push(Player {
            name,
            x,
            y,
            direction: dir,
            alive: true,
            trail: VecDeque::new(),
            distance_traveled: 0,
            score: 0,
        });

        Some(idx)
    }

    /// Start the game
    pub fn start(&mut self) {
        self.status = GameStatus::Running;
        // Place initial player positions on the grid
        for (idx, player) in self.players.iter().enumerate() {
            let x = player.x as usize;
            let y = player.y as usize;
            if y < self.height && x < self.width {
                self.grid[y][x] = Cell::Trail(idx);
            }
        }
    }

    /// Move a single player one step: apply steering then advance forward.
    /// Returns a description of what happened.
    pub fn move_player(&mut self, player_idx: usize, action: SteerAction) -> String {
        if self.status != GameStatus::Running {
            return "Game is not running.".to_string();
        }

        let player = &mut self.players[player_idx];
        if !player.alive {
            return "You have crashed! Game over.".to_string();
        }

        // Apply steering
        match action {
            SteerAction::Left => player.direction = player.direction.turn_left(),
            SteerAction::Right => player.direction = player.direction.turn_right(),
            SteerAction::Straight => {}
        }

        // Calculate new position
        let (dx, dy) = player.direction.delta();
        let nx = player.x + dx;
        let ny = player.y + dy;

        // Check out of bounds
        if nx < 0 || ny < 0 || nx >= self.width as i32 || ny >= self.height as i32 {
            self.players[player_idx].alive = false;
            self.check_win_condition();
            return "CRASHED into the boundary wall!".to_string();
        }

        let ux = nx as usize;
        let uy = ny as usize;

        // Check grid collision
        match self.grid[uy][ux] {
            Cell::Wall => {
                self.players[player_idx].alive = false;
                self.check_win_condition();
                return "CRASHED into a wall!".to_string();
            }
            Cell::Obstruction => {
                self.players[player_idx].alive = false;
                self.check_win_condition();
                return "CRASHED into an obstruction!".to_string();
            }
            Cell::Trail(other_idx) => {
                self.players[player_idx].alive = false;
                let whose = if other_idx == player_idx {
                    "your own".to_string()
                } else {
                    format!("{}'s", self.players[other_idx].name)
                };
                self.check_win_condition();
                return format!("CRASHED into {} trail!", whose);
            }
            Cell::Empty => {}
        }

        // Move is safe — update position
        let old_x = self.players[player_idx].x;
        let old_y = self.players[player_idx].y;
        self.players[player_idx].trail.push_back((old_x, old_y));

        // Trim trail if too long
        let max_trail = self.max_trail_length;
        while self.players[player_idx].trail.len() > max_trail {
            if let Some((tx, ty)) = self.players[player_idx].trail.pop_front() {
                let tux = tx as usize;
                let tuy = ty as usize;
                if tuy < self.height && tux < self.width {
                    if self.grid[tuy][tux] == Cell::Trail(player_idx) {
                        self.grid[tuy][tux] = Cell::Empty;
                    }
                }
            }
        }

        // Update player position
        self.players[player_idx].x = nx;
        self.players[player_idx].y = ny;
        self.players[player_idx].distance_traveled += 1;
        self.tick += 1;

        // Place trail on grid
        self.grid[uy][ux] = Cell::Trail(player_idx);

        self.check_win_condition();

        format!(
            "Moved {} to ({}, {}). Distance: {}.",
            self.players[player_idx].direction.name(),
            nx,
            ny,
            self.players[player_idx].distance_traveled
        )
    }

    /// Check if only one (or zero) players are alive and finish the game
    fn check_win_condition(&mut self) {
        let alive_players: Vec<usize> = self
            .players
            .iter()
            .enumerate()
            .filter(|(_, p)| p.alive)
            .map(|(i, _)| i)
            .collect();

        if alive_players.len() <= 1 && self.players.len() > 1 {
            self.status = GameStatus::Finished;
            self.finished_at = Some(chrono::Utc::now());

            if alive_players.len() == 1 {
                let winner_idx = alive_players[0];
                self.winner = Some(winner_idx);

                let speed_bonus = if self.tick > 0 {
                    (1000 / self.tick).min(200)
                } else {
                    0
                };
                self.players[winner_idx].score =
                    100 + self.players[winner_idx].distance_traveled + speed_bonus;
            }
        }
    }

    /// Get the visible area around a player for the `look` tool
    pub fn look(&self, player_idx: usize, view_radius: usize) -> String {
        let player = &self.players[player_idx];
        let mut lines = Vec::new();

        lines.push(format!(
            "Your light-cycle '{}' is at ({}, {}) heading {}.",
            player.name, player.x, player.y, player.direction.name()
        ));

        if !player.alive {
            lines.push("YOU HAVE CRASHED! Game over for you.".to_string());
            return lines.join("\n");
        }

        lines.push(format!(
            "Distance traveled: {}. Tick: {}.",
            player.distance_traveled, self.tick
        ));

        let alive_count = self.players.iter().filter(|p| p.alive).count();
        let total_count = self.players.len();
        lines.push(format!(
            "Players alive: {}/{}",
            alive_count, total_count
        ));

        // Render grid view
        let r = view_radius as i32;
        lines.push(format!(
            "Grid ({}x{} view centered on you):",
            view_radius * 2 + 1,
            view_radius * 2 + 1
        ));

        for dy in -r..=r {
            let mut row = String::new();
            for dx in -r..=r {
                let gx = player.x + dx;
                let gy = player.y + dy;

                if !row.is_empty() {
                    row.push(' ');
                }

                if gx == player.x && gy == player.y {
                    row.push('@');
                } else if gx < 0
                    || gy < 0
                    || gx >= self.width as i32
                    || gy >= self.height as i32
                {
                    row.push('#');
                } else {
                    let cell = self.grid[gy as usize][gx as usize];
                    match cell {
                        Cell::Empty => row.push('.'),
                        Cell::Wall => row.push('#'),
                        Cell::Obstruction => row.push('X'),
                        Cell::Trail(idx) => {
                            if idx == player_idx {
                                row.push('|');
                            } else {
                                // Use digits 1-9 for other players
                                let digit = ((idx % 9) + 1).to_string();
                                row.push_str(&digit);
                            }
                        }
                    }
                }
            }
            lines.push(row);
        }

        lines.push(String::new());
        lines.push(
            "Legend: @ = you, | = your trail, 1-9 = other players/trails, # = wall, X = obstruction, . = empty"
                .to_string(),
        );

        // Show other players info
        for (i, p) in self.players.iter().enumerate() {
            if i == player_idx {
                continue;
            }
            let status = if p.alive { "ALIVE" } else { "CRASHED" };
            let distance = ((p.x - player.x).abs() + (p.y - player.y).abs()) as u32;
            lines.push(format!(
                "Player '{}': {} (manhattan distance: {})",
                p.name, status, distance
            ));
        }

        lines.join("\n")
    }

    /// Serialize game state for the web UI
    pub fn to_web_state(&self) -> WebGameState {
        let grid_data: Vec<Vec<u8>> = self
            .grid
            .iter()
            .map(|row| {
                row.iter()
                    .map(|cell| match cell {
                        Cell::Empty => 0,
                        Cell::Wall => 1,
                        Cell::Obstruction => 2,
                        Cell::Trail(idx) => (3 + *idx) as u8,
                    })
                    .collect()
            })
            .collect();

        let players: Vec<WebPlayer> = self
            .players
            .iter()
            .enumerate()
            .map(|(i, p)| WebPlayer {
                index: i,
                name: p.name.clone(),
                x: p.x,
                y: p.y,
                alive: p.alive,
                direction: p.direction,
                distance: p.distance_traveled,
                score: p.score,
            })
            .collect();

        WebGameState {
            id: self.id.to_string(),
            width: self.width,
            height: self.height,
            grid: grid_data,
            players,
            status: self.status,
            tick: self.tick,
            course_name: self.course_name.clone(),
            course_level: self.course_level,
            winner: self.winner,
            created_at: self.created_at.to_rfc3339(),
            finished_at: self.finished_at.map(|t| t.to_rfc3339()),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct WebGameState {
    pub id: String,
    pub width: usize,
    pub height: usize,
    pub grid: Vec<Vec<u8>>,
    pub players: Vec<WebPlayer>,
    pub status: GameStatus,
    pub tick: u32,
    pub course_name: String,
    pub course_level: u32,
    pub winner: Option<usize>,
    pub created_at: String,
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WebPlayer {
    pub index: usize,
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub alive: bool,
    pub direction: Direction,
    pub distance: u32,
    pub score: u32,
}
