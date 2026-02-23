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
    pub pending_action: Option<SteerAction>,
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
            pending_action: None,
        });

        Some(idx)
    }

    /// Apply a steering action for a player
    pub fn apply_action(&mut self, player_idx: usize, action: SteerAction) {
        if let Some(player) = self.players.get_mut(player_idx) {
            if player.alive {
                player.pending_action = Some(action);
            }
        }
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

    /// Advance the game by one tick
    pub fn tick(&mut self) {
        if self.status != GameStatus::Running {
            return;
        }

        self.tick += 1;

        // Apply pending actions and calculate new positions
        let mut new_positions: Vec<(i32, i32)> = Vec::new();

        for player in self.players.iter_mut() {
            if !player.alive {
                new_positions.push((player.x, player.y));
                continue;
            }

            // Apply steering
            match player.pending_action.take() {
                Some(SteerAction::Left) => player.direction = player.direction.turn_left(),
                Some(SteerAction::Right) => player.direction = player.direction.turn_right(),
                Some(SteerAction::Straight) | None => {}
            }

            // Calculate new position
            let (dx, dy) = player.direction.delta();
            let nx = player.x + dx;
            let ny = player.y + dy;
            new_positions.push((nx, ny));
        }

        // Check collisions for each alive player
        let mut killed = vec![false; self.players.len()];

        for i in 0..self.players.len() {
            if !self.players[i].alive {
                continue;
            }

            let (nx, ny) = new_positions[i];

            // Out of bounds
            if nx < 0 || ny < 0 || nx >= self.width as i32 || ny >= self.height as i32 {
                killed[i] = true;
                continue;
            }

            let ux = nx as usize;
            let uy = ny as usize;

            // Check grid collision (wall, obstruction, trail)
            match self.grid[uy][ux] {
                Cell::Wall | Cell::Obstruction | Cell::Trail(_) => {
                    killed[i] = true;
                    continue;
                }
                Cell::Empty => {}
            }

            // Check head-on collision with other players
            for j in 0..self.players.len() {
                if i == j || !self.players[j].alive {
                    continue;
                }
                if new_positions[i] == new_positions[j] {
                    killed[i] = true;
                    killed[j] = true;
                }
            }
        }

        // Apply movements and kills
        for i in 0..self.players.len() {
            if !self.players[i].alive {
                continue;
            }

            if killed[i] {
                self.players[i].alive = false;
                continue;
            }

            let (nx, ny) = new_positions[i];

            // Add current position to trail (extract to avoid borrow conflict)
            let old_x = self.players[i].x;
            let old_y = self.players[i].y;
            self.players[i].trail.push_back((old_x, old_y));

            // Trim trail if too long
            let max_trail = self.max_trail_length;
            while self.players[i].trail.len() > max_trail {
                if let Some((tx, ty)) = self.players[i].trail.pop_front() {
                    let ux = tx as usize;
                    let uy = ty as usize;
                    if uy < self.height && ux < self.width {
                        if self.grid[uy][ux] == Cell::Trail(i) {
                            self.grid[uy][ux] = Cell::Empty;
                        }
                    }
                }
            }

            // Move player
            self.players[i].x = nx;
            self.players[i].y = ny;
            self.players[i].distance_traveled += 1;

            // Place trail on grid
            let ux = nx as usize;
            let uy = ny as usize;
            self.grid[uy][ux] = Cell::Trail(i);
        }

        // Check win condition
        let alive_players: Vec<usize> = self
            .players
            .iter()
            .enumerate()
            .filter(|(_, p)| p.alive)
            .map(|(i, _)| i)
            .collect();

        if alive_players.len() <= 1 {
            self.status = GameStatus::Finished;
            self.finished_at = Some(chrono::Utc::now());

            if alive_players.len() == 1 {
                let winner_idx = alive_players[0];
                self.winner = Some(winner_idx);

                // Calculate score: base 100 + distance bonus + speed bonus
                let speed_bonus = if self.tick > 0 {
                    (1000 / self.tick).min(200)
                } else {
                    0
                };
                self.players[winner_idx].score =
                    100 + self.players[winner_idx].distance_traveled + speed_bonus;
            }
        }

        // Also finish if no players alive
        if alive_players.is_empty() && self.status != GameStatus::Finished {
            self.status = GameStatus::Finished;
            self.finished_at = Some(chrono::Utc::now());
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
