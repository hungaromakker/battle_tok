# Project Summary for PRD Generation

**Project:** battle_tök

## What We're Building
A hexagon-based strategy game in Rust where players reunite a collapsed world by capturing regions, building castles, and defending against enemies. Turn-based conquest with physics-based building and AI-managed workers.

## Must Have
- Hexagonal tile world with chain bridges
- Castle building with Fortnite-style physics
- AI-controlled workers (no micromanagement)
- Turn-based regional capture mechanics
- 1v1 battle arena mode
- Main tower defense mechanic

## Must NOT Have
- Unit micromanagement
- Heavy external dependencies
- Complex economy systems
- Online multiplayer (phase 1)

## Technical Stack
- **Language**: Rust
- **Engine**: Custom (based on magic_engine)
- **Graphics**: wgpu + SDF ray marching
- **Math**: glam
- **Window**: winit
- **Deps**: Minimal (< 10 direct dependencies)

## Starting Assets
- battle_arena.rs - 1v1 arena demo scene
- hex_planet.rs - Full planet scene
- Full SDF rendering engine
- Existing shaders and game modules

## Success Looks Like
- Smooth 60fps gameplay
- Intuitive building system
- Satisfying castle sieges
- Fun 1v1 matches

---
**Ready for PRD generation. Run `/prd` to continue.**
