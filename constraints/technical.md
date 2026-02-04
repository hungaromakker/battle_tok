# Technical Constraints

## Language & Engine
- **Rust**: Primary language
- **Custom Engine**: Based on magic_engine SDF renderer
- **wgpu**: Graphics backend (already in use)
- **Minimal deps**: Only essential crates

## Current Dependencies (from magic_engine)
```toml
wgpu = "27"
glam = { version = "0.31", features = ["serde"] }
bytemuck = { version = "1.14", features = ["derive"] }
winit = "0.30"
pollster = "0.4"
static_assertions = "1.1"
```

## Performance Targets
- 60fps minimum on mid-range hardware
- Fast shader compilation
- Efficient SDF rendering

## Rendering Approach
- SDF-based graphics (Signed Distance Fields)
- Ray marching for rendering
- GPU compute for physics/simulation where beneficial

## Build System
- Cargo for Rust builds
- No external build tools required
