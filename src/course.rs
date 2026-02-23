use serde::Serialize;

/// A course / level definition
#[derive(Debug, Clone, Serialize)]
pub struct Course {
    pub name: String,
    pub level: u32,
    pub width: usize,
    pub height: usize,
    pub max_trail_length: usize,
    pub max_players: usize,
    pub obstructions: Vec<(usize, usize)>,
    pub walls: Vec<(usize, usize)>,
}

/// Get all available courses, ordered by difficulty
pub fn all_courses() -> Vec<Course> {
    vec![
        course_open_arena(),
        course_the_maze(),
        course_narrow_corridors(),
        course_the_gauntlet(),
        course_chaos(),
    ]
}

/// Get a course by level number (1-indexed)
pub fn get_course(level: u32) -> Course {
    let courses = all_courses();
    let idx = ((level as usize).saturating_sub(1)).min(courses.len() - 1);
    courses[idx].clone()
}

fn course_open_arena() -> Course {
    Course {
        name: "Open Arena".to_string(),
        level: 1,
        width: 30,
        height: 30,
        max_trail_length: 50,
        max_players: 4,
        obstructions: vec![],
        walls: vec![],
    }
}

fn course_the_maze() -> Course {
    let mut walls = Vec::new();
    // Horizontal walls
    for x in 8..22 {
        walls.push((x, 10));
        walls.push((x, 25));
    }
    // Vertical walls
    for y in 10..20 {
        walls.push((15, y));
    }
    for y in 5..15 {
        walls.push((25, y));
    }
    for y in 20..30 {
        walls.push((8, y));
    }

    Course {
        name: "The Maze".to_string(),
        level: 2,
        width: 40,
        height: 35,
        max_trail_length: 80,
        max_players: 4,
        obstructions: vec![],
        walls,
    }
}

fn course_narrow_corridors() -> Course {
    let mut walls = Vec::new();
    // Create horizontal corridor dividers
    for x in 0..50 {
        if x < 10 || x > 15 {
            walls.push((x, 7));
        }
        if x < 30 || x > 40 {
            walls.push((x, 14));
        }
    }

    Course {
        name: "Narrow Corridors".to_string(),
        level: 3,
        width: 50,
        height: 22,
        max_trail_length: 100,
        max_players: 4,
        obstructions: vec![],
        walls,
    }
}

fn course_the_gauntlet() -> Course {
    let mut obstructions = Vec::new();
    // Scatter obstructions in a pattern
    for x in (5..55).step_by(6) {
        for y in (5..35).step_by(6) {
            obstructions.push((x, y));
            obstructions.push((x + 1, y));
            obstructions.push((x, y + 1));
            obstructions.push((x + 1, y + 1));
        }
    }

    Course {
        name: "The Gauntlet".to_string(),
        level: 4,
        width: 60,
        height: 40,
        max_trail_length: 150,
        max_players: 6,
        obstructions,
        walls: vec![],
    }
}

fn course_chaos() -> Course {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let mut walls = Vec::new();

    // Random wall segments
    for _ in 0..30 {
        let sx = rng.gen_range(5..70);
        let sy = rng.gen_range(5..70);
        let horizontal = rng.gen_bool(0.5);
        let length = rng.gen_range(3..10);

        for i in 0..length {
            let (wx, wy) = if horizontal {
                (sx + i, sy)
            } else {
                (sx, sy + i)
            };
            if wx < 79 && wy < 79 {
                walls.push((wx, wy));
            }
        }
    }

    Course {
        name: "Chaos".to_string(),
        level: 5,
        width: 80,
        height: 80,
        max_trail_length: 300,
        max_players: 8,
        obstructions: vec![],
        walls,
    }
}
