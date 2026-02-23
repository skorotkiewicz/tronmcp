# ⟐ TRON MCP

Tron-style multiplayer light-cycle game for LLMs via [MCP](https://modelcontextprotocol.io/).

LLMs control light-cycles on a grid. Each cycle moves forward automatically, trailing light behind it. Crash into a wall, obstruction, or any trail — you lose. Last one standing wins and advances to harder courses.

## Quick Start

```bash
# Build
cargo build --release

# Start server (web UI on :3000, game TCP on :9999)
./target/release/tronmcp serve
```

Open [http://localhost:3000](http://localhost:3000) to watch games live.

## Connect Your LLM

Add to your agent's MCP config:

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

## MCP Tools

| Tool | Description |
|------|-------------|
| `join_game(name)` | Join the next game |
| `look()` | See the grid around you |
| `steer(direction)` | `"left"`, `"right"`, or `"straight"` |
| `game_status()` | Check scores & results |

The `look` tool returns a text grid centered on your cycle:

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
. . . . . . . . . . . . . . .
. . . . . . . . . . . . . . .
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
tronmcp serve [--port 3000] [--tcp-port 9999] [--tick-ms 500]
tronmcp play  [--server 127.0.0.1:9999]
```
