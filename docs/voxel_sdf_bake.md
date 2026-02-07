# Voxel SDF Baking and Block Look

This doc describes how to improve collision/block visuals by baking voxels into SDF bricks and rendering them in the raymarcher with a sharp block look.

## Current State

- **Building blocks** are rendered as **mesh** (chunked vertex buffers in `regenerate_block_mesh`). Collision uses AABB/block manager.
- **SDF baking** (`sdf_bake.wgsl`, `BrickCache`) bakes **one primitive per brick** (sphere, box, capsule, etc.). Entities with `baked_sdf_id` sample that brick in the raymarcher.
- **Block look**: The raymarcher now uses an **analytical box normal** for `SDF_BOX` entities so boxes render with sharp edges and clear faces (see `sdf_box_normal_local` and `get_entity_normal_world` in `raymarcher.wgsl`).

## Improvements Implemented

### 1. Block look in raymarcher (shader)

- **`sdf_box_normal_local(p, half_extents)`**  
  Returns an axis-aligned outward normal for the box SDF so faces are flat and edges sharp (no gradient smoothing).

- **`get_entity_normal_world(p, entity)`**  
  For `SDF_BOX` entities, returns the analytical world-space normal; otherwise returns zero so the pipeline falls back to `calculate_normal(p)`.

- **Lighting branches**  
  When shading an entity hit (`result.entity_index >= 0`), the shader uses `get_entity_normal_world` for box entities so blocks get a crisp block look; other types keep gradient normals.

So any entity rendered as a box (e.g. building blocks pushed as SDF entities with `sdf_type == SDF_BOX`) now gets sharp block shading automatically.

## Baking voxels into SDF (design)

To render **many blocks** as a single SDF volume (one brick per chunk) and sample it in the raymarcher:

### Option A: Chunk SDF bake (many boxes → one brick)

1. **Chunk layout**  
   - World is divided into chunks (e.g. 8×8×8 m).  
   - Each chunk has a list of axis-aligned boxes (center + half-extents or min/max) in chunk-local space.

2. **Bake compute**  
   - New mode in `sdf_bake.wgsl` (or a separate `sdf_bake_voxel.wgsl`): for each voxel in the 64³ grid, compute the **minimum** of all box SDFs in that chunk (union).  
   - Formula: `d = min over boxes of sdf_box(local_pos - box_center, box_half_extents)`.  
   - Write `d` into the brick at the voxel index (same layout as current bricks: `brick_offset + z*4096 + y*64 + x`).

3. **Data**  
   - Per chunk: one `baked_sdf_id` (slot in `BrickCache`).  
   - A buffer of box data (e.g. `vec3 center`, `vec3 half_extents`) per chunk, or a single global buffer with chunk → box count + offset.  
   - Bounds for the chunk in world space (e.g. `chunk_origin`, `chunk_size`) so the raymarcher can map world `p` to chunk-local normalized `[0,1]` and sample the correct brick.

4. **Raymarcher**  
   - In `scene_sdf`, after (or instead of) entity list: for the ray position `p`, determine which chunk it’s in. If that chunk has a baked voxel brick, transform `p` to chunk-local normalized coords, sample that brick (same trilinear as `sample_baked_sdf`), scale distance by chunk size, and merge with the rest of the scene (e.g. min with entity/terrain distance).  
   - Normal: either gradient of the chunk SDF (several samples) or, if you store per-voxel material and decode which box was hit, an analytical box normal in chunk space then transformed to world.

5. **Collision**  
   - Same SDF can be sampled for collision (sphere cast or ray march) so collision and visuals stay in sync.

### Option B: Blocks as entities with baked box

- Keep current “one entity = one primitive” bake.  
- For each building block (or only cube blocks), create a `GpuEntity` with `sdf_type = SDF_BOX`, position/scale/rotation, and run the existing bake so it gets a `baked_sdf_id`.  
- Pushing blocks into the entity buffer and baking them gives raymarched blocks with the existing block look (analytical box normal).  
- Collision can still use the same entity SDF or keep AABB for blocks.

### Option C: Hybrid

- **Voxel chunks** for static, dense regions (one brick per chunk, union of boxes).  
- **Entities** for moving or few blocks (current path).  
- Raymarcher merges chunk SDF and entity SDF (min distance), and uses analytical box normal when the hit is classified as “chunk voxel” and the dominant box is known (e.g. from a material/ID buffer or gradient).

## Summary

- **Block look**: Done in the raymarcher with analytical box normals for `SDF_BOX` entities.  
- **Bake voxels into SDF**: Implement chunk-level union-of-boxes baking (Option A) and wire chunk bricks into the raymarcher and optionally collision; or push blocks as entities and use current baking (Option B).
