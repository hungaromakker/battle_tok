# US-P4-002: Orbit Camera for Asset Preview

## Description
Create an orbit camera system for the Asset Editor that allows the user to rotate around the center point of the 3D preview. This camera is used in stages 2-5 (Extrude, Sculpt, Color, Save) for inspecting the 3D mesh from all angles. The existing `Camera` in `src/game/types.rs` is an FPS camera — the orbit camera is a new, separate struct purpose-built for editor workflows.

## The Core Concept / Why This Matters
When working with 3D assets, the user needs to view the mesh from every angle to check proportions, surface quality, and coloring. A standard FPS camera is wrong for this — the user needs an orbit camera that always looks at the center of the asset. This is the standard camera model used in 3D modeling tools (Blender, Maya, etc.). Without this, stages 2-5 are unusable because the user cannot inspect their work. The orbit camera uses spherical coordinates (azimuth, elevation, distance) centered on a target point, making rotation intuitive and predictable.

## Goal
Create `src/game/asset_editor/orbit_camera.rs` with an `OrbitCamera` struct that provides view and projection matrices, responds to mouse input for orbit/zoom/pan, and integrates into the `battle_editor.rs` render loop for stages 2-5.

## Files to Create/Modify
- **Create** `src/game/asset_editor/orbit_camera.rs` — `OrbitCamera` struct with spherical coordinate orbit, scroll zoom, right-mouse pan, view/projection matrix generation
- **Modify** `src/game/asset_editor/mod.rs` — Add `pub mod orbit_camera;`, add `camera: OrbitCamera` field to `AssetEditor`, use camera matrices in render for stages 2-5
- **Modify** `src/bin/battle_editor.rs` — Forward mouse events (middle drag, right drag, scroll) to AssetEditor/OrbitCamera

## Implementation Steps
1. Create `OrbitCamera` struct with fields:
   - `azimuth: f32` — horizontal angle in degrees (0-360, wrapping)
   - `elevation: f32` — vertical angle in degrees (clamped -89 to 89)
   - `distance: f32` — zoom distance from target (clamped 0.5 to 50.0)
   - `target: [f32; 3]` — point camera orbits around (default `[0, 0, 0]`)
   - `aspect: f32` — viewport aspect ratio
   - `fov: f32` — field of view in degrees (default 45.0)
   - `near: f32`, `far: f32` — clip planes (0.01, 100.0)
   - `is_orbiting: bool`, `is_panning: bool` — mouse state tracking
   - `last_mouse: [f32; 2]` — previous mouse position for delta calculation
2. Implement `OrbitCamera::new(aspect: f32) -> Self` with defaults: azimuth 30.0, elevation 25.0, distance 5.0
3. Implement `view_matrix(&self) -> [[f32; 4]; 4]`:
   - Convert azimuth/elevation to radians
   - Compute eye position on sphere: `eye = target + [distance * cos(elev) * sin(azim), distance * sin(elev), distance * cos(elev) * cos(azim)]`
   - Build look-at matrix from eye, target, up=`[0, 1, 0]`
4. Implement `projection_matrix(&self) -> [[f32; 4]; 4]`:
   - Standard perspective projection from `fov`, `aspect`, `near`, `far`
5. Implement `handle_mouse_drag(&mut self, button: MouseButton, pressed: bool)`:
   - Middle button → set `is_orbiting`
   - Right button → set `is_panning`
   - Record `last_mouse` on press
6. Implement `handle_mouse_move(&mut self, x: f32, y: f32)`:
   - If orbiting: dx changes azimuth, dy changes elevation (sensitivity ~0.3 deg/px)
   - If panning: delta moves target in camera-local right/up directions
   - Always update `last_mouse`
7. Implement `handle_scroll(&mut self, delta: f32)`:
   - Multiplicative zoom: `distance *= 1.0 - delta * 0.1`
   - Clamp distance to `[0.5, 50.0]`
8. Implement `handle_pan(&mut self, dx: f32, dy: f32)`:
   - Compute camera right and up vectors from current azimuth/elevation
   - Offset `target` by `right * dx * pan_speed + up * dy * pan_speed`
9. Implement `resize(&mut self, width: u32, height: u32)` to update aspect ratio
10. Implement `view_projection_matrix(&self) -> [[f32; 4]; 4]` — multiply projection * view
11. Integrate into `AssetEditor`:
    - Add `camera: OrbitCamera` field
    - Stages 2-5: pass camera view-projection to the render uniform buffer
    - Stage 1 (Draw2D): use orthographic projection, orbit camera is not active
12. In `battle_editor.rs`, forward winit events to the camera:
    - `MouseInput` for middle and right buttons
    - `CursorMoved` for orbit/pan deltas
    - `MouseWheel` for zoom

## Code Patterns
Matrix math — use the same hand-rolled style as the existing codebase (no glam/nalgebra):
```rust
fn look_at(eye: [f32; 3], target: [f32; 3], up: [f32; 3]) -> [[f32; 4]; 4] {
    let f = normalize(sub(target, eye));
    let r = normalize(cross(f, up));
    let u = cross(r, f);
    [
        [r[0], u[0], -f[0], 0.0],
        [r[1], u[1], -f[1], 0.0],
        [r[2], u[2], -f[2], 0.0],
        [-dot(r, eye), -dot(u, eye), dot(f, eye), 1.0],
    ]
}
```

Mouse orbit pattern:
```rust
pub fn handle_mouse_move(&mut self, x: f32, y: f32) {
    let dx = x - self.last_mouse[0];
    let dy = y - self.last_mouse[1];
    if self.is_orbiting {
        self.azimuth += dx * 0.3;
        self.elevation = (self.elevation - dy * 0.3).clamp(-89.0, 89.0);
    }
    if self.is_panning {
        self.handle_pan(-dx * 0.005 * self.distance, dy * 0.005 * self.distance);
    }
    self.last_mouse = [x, y];
}
```

## Acceptance Criteria
- [ ] `orbit_camera.rs` exists at `src/game/asset_editor/orbit_camera.rs` with `OrbitCamera` struct
- [ ] `view_matrix()` returns a correct look-at matrix from spherical coordinates
- [ ] `projection_matrix(aspect)` returns a correct perspective projection matrix
- [ ] `handle_mouse_drag` sets orbiting/panning state based on middle/right mouse
- [ ] `handle_mouse_move` updates azimuth/elevation when orbiting, target when panning
- [ ] `handle_scroll` zooms distance multiplicatively with clamping to `[0.5, 50.0]`
- [ ] Camera is used in stages 2-5 but NOT in stage 1 (Draw2D uses orthographic)
- [ ] `battle_arena.rs` is NOT modified
- [ ] `cargo check --bin battle_editor` compiles with 0 errors

## Verification Commands
- `cmd`: `cargo check --bin battle_editor 2>&1; echo EXIT:$?`
  `expect_contains`: `EXIT:0`
  `description`: `battle_editor binary compiles with orbit camera`
- `cmd`: `cargo check --bin battle_arena 2>&1; echo EXIT:$?`
  `expect_contains`: `EXIT:0`
  `description`: `battle_arena still compiles unmodified`
- `cmd`: `test -f src/game/asset_editor/orbit_camera.rs && echo EXISTS`
  `expect_contains`: `EXISTS`
  `description`: `orbit_camera.rs file exists`
- `cmd`: `grep -c 'pub struct OrbitCamera' src/game/asset_editor/orbit_camera.rs`
  `expect_gt`: 0
  `description`: `OrbitCamera struct is public`
- `cmd`: `grep -c 'view_matrix\|projection_matrix\|handle_mouse_drag\|handle_scroll\|handle_pan' src/game/asset_editor/orbit_camera.rs`
  `expect_gt`: 4
  `description`: `All required methods are implemented`
- `cmd`: `grep -c 'pub mod orbit_camera' src/game/asset_editor/mod.rs`
  `expect_gt`: 0
  `description`: `orbit_camera module registered in mod.rs`

## Success Looks Like
Running `cargo run --bin battle_editor` and switching to stages 2-5 shows a 3D perspective view. Holding middle-mouse and dragging rotates the view around the asset center. Scrolling zooms in and out smoothly. Right-mouse dragging pans the view laterally. The camera starts at a sensible default angle (slightly above and to the side). In stage 1, the view remains a flat 2D orthographic canvas unaffected by the orbit camera.

## Dependencies
- Depends on: US-P4-001

## Complexity
- Complexity: normal
- Min iterations: 1
