# Original Prompt

A strategy game built on a custom Rust engine with minimal dependencies and high performance.

The game is about reuniting a collapsed world. You start on one hexagon with minimal resources and must build a castle to defend against enemies. Hexagons are connected by chain bridges. Your goal is to capture other regions by planting your flag at their center.

## Core Concepts

- **Hexagon-based world**: The world consists of hexagonal tiles
- **Castle building**: Build castles with main towers that are hard to capture
- **Fortnite-style building**: Physical building system with boundaries and physics checks for realistic building limits
- **Actors and Workers**: AI automatically distributes workers, no micromanagement needed - just build, upgrade, and attack
- **Resource production**: Produce projectiles and resources
- **Turn-based conquest**: Capture regions turn by turn

## Game Modes (Initial Focus)

1. **1v1 Battle Arena** - Core gameplay mode with one hexagon each
2. **Hex Planet Scene** - Full planet view with multiple hexagons

Both demo scenes (battle_arena and hex_planet) from magic_engine have been copied as starting points.

## Technical Requirements

- Rust-based custom engine
- Minimal dependencies
- High performance
- No micromanagement - AI handles worker distribution
