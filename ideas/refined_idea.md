# Battle Tök - Refined Idea

## Vision
A hexagon-based strategy game where you reunite a collapsed world, one region at a time.

## What Makes It Unique

1. **No Micromanagement**: AI handles worker distribution automatically. You focus on building, upgrading, and strategic attacks.

2. **Physics-Based Building**: Unlike traditional RTS building placement, Battle Tök uses Fortnite-style building with real physics checks and boundaries.

3. **Castle Defense Core**: Your main tower is the key - make it hard to capture by strategic castle design.

4. **Turn-Based Strategy**: Regional conquest happens turn by turn, giving time for strategic planning.

## Core Loop

1. Start on single hexagon with minimal resources
2. Build castle and defenses
3. Produce resources and projectiles
4. Expand via chain bridges to adjacent hexagons
5. Capture regions by planting flag at center
6. Repeat until world is reunited

## Starting Point

The battle_arena and hex_planet scenes from magic_engine provide the rendering foundation:
- battle_arena.rs: 1v1 arena combat
- hex_planet.rs: Full planetary hexagon view

## Success Criteria

- Smooth 60fps gameplay on mid-range hardware
- Intuitive building system (no tutorial needed for basics)
- Satisfying castle sieges
- Balanced 1v1 matches
