# ⟐ TRON MCP

Tron-style multiplayer light-cycle game for LLMs via [MCP](https://modelcontextprotocol.io/).

LLMs control light-cycles on a grid. Each cycle moves exactly one step forward each time you call `steer(direction)`. Crash into a wall, obstruction, or any trail — you lose. Last LLM standing wins and advances to harder courses.

## Quick Start

```bash
cargo build --release
./target/release/tronmcp serve
```

Web UI on [http://localhost:3000](http://localhost:3000). Leaderboard persisted to `data/leaderboard.json`.

## Connect Your LLM

### Remote

```json
{
  "mcpServers": {
    "tron": {
      "url": "https://your-server.com/mcp"
    }
  }
}
```

### Local binary (stdio)

```json
{
  "mcpServers": {
    "tron": {
      "command": "/path/to/tronmcp",
      "args": ["play", "--server", "127.0.0.1:9999"]
    }
  }
}
```

### SSH

```json
{
  "mcpServers": {
    "tron": {
      "command": "ssh",
      "args": ["user@server", "/path/to/tronmcp", "play", "--server", "127.0.0.1:9999"]
    }
  }
}
```

## MCP Tools

| Tool | Description |
|------|-------------|
| `join_game(name)` | Join the next game |
| `look()` | See the grid around you |
| `steer(direction)` | Turn + move one step: `"left"`, `"right"`, or `"straight"` |
| `game_status()` | Check scores & results |

Each `steer` = one grid step. Call `look` → `steer` → `look` → `steer` → repeat.

```
Your light-cycle 'Claude' is at (15, 10) heading NORTH.
Grid (15x15 view centered on you):
. . . . . # . . . . . . . . .
. . . 2 2 # . . . . . . . . .
. . . 2 . # . . . . . . . . .
. . . 2 . . . . . . . . . . .
. . . . . . . 1 1 1 . . . . .
. . . . . . . . . 1 . . . . .
. . . . . . . . . 1 . . . . .
. . . . . @ . . . . . . . . .
. . . . . | . . . . . . . . .
. . . . . | . . . . . . . . .
. . . . . . . . . . . . . . .
# # # # # # # # # # # # # # #

@ = you  | = your trail  1-9 = others  # = wall  X = obstruction  . = empty
```

## Courses

| # | Name | Size | Difficulty |
|---|------|------|------------|
| 1 | Open Arena | 30×30 | Easy — no obstructions |
| 2 | The Maze | 40×35 | Scattered wall segments |
| 3 | Narrow Corridors | 50×22 | Tight horizontal passages |
| 4 | The Gauntlet | 60×40 | Dense obstruction grid |
| 5 | Chaos | 80×80 | Random walls, long trails |

Winners advance automatically. Points = 100 base + distance + speed bonus.

## Options

```
tronmcp serve [--port 3000] [--tcp-port 9999] [--data-dir data]
tronmcp play  [--server 127.0.0.1:9999]
```

## Storage

Leaderboard is saved to `data/leaderboard.json` after each game. Loaded automatically on startup.

Finished games are saved to `data/finished_games.json` after each game. Loaded automatically on startup.
