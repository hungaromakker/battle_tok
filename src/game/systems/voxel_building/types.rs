use glam::{IVec3, Vec3};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct VoxelCoord {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl VoxelCoord {
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    pub fn as_ivec3(self) -> IVec3 {
        IVec3::new(self.x, self.y, self.z)
    }
}

impl From<IVec3> for VoxelCoord {
    fn from(value: IVec3) -> Self {
        Self::new(value.x, value.y, value.z)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VoxelMaterialId(pub u8);

pub const VOXEL_FLAG_TERRAIN_ANCHORED: u8 = 1 << 0;
pub const VOXEL_FLAG_RIGID_JOINT: u8 = 1 << 1;
pub const VOXEL_FLAG_RIB_MEMBER: u8 = 1 << 2;

#[derive(Debug, Clone, Copy)]
pub struct VoxelCell {
    pub material: u8,
    pub hp: u16,
    pub max_hp: u16,
    pub color_rgb: [u8; 3],
    pub normal_oct: [u8; 2],
    pub flags: u8,
}

#[derive(Debug, Clone, Copy)]
pub struct VoxelHit {
    pub coord: VoxelCoord,
    pub world_pos: Vec3,
    pub normal: IVec3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamageSource {
    Cannonball,
    Rocket,
    HitscanGun,
}

#[derive(Debug, Clone, Copy)]
pub struct VoxelDamageResult {
    pub destroyed: bool,
    pub remaining_hp: u16,
}

#[derive(Debug, Clone, Copy)]
pub struct CastleToolParams {
    pub wall_height_vox: u8,
    pub wall_thickness_vox: u8,
    pub plate_thickness_vox: u8,
    pub joint_spacing_vox: u8,
    pub joint_radius_vox: u8,
    pub rib_spacing_vox: u8,
    pub rib_levels: [f32; 2],
}

impl Default for CastleToolParams {
    fn default() -> Self {
        Self {
            wall_height_vox: 24,
            wall_thickness_vox: 4,
            plate_thickness_vox: 3,
            joint_spacing_vox: 4,
            joint_radius_vox: 2,
            rib_spacing_vox: 4,
            rib_levels: [0.33, 0.66],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoxelEditOp {
    Place,
    Remove,
}

#[derive(Debug, Clone, Copy)]
pub struct VoxelEdit {
    pub op: VoxelEditOp,
    pub coord: VoxelCoord,
    pub material: VoxelMaterialId,
    pub normal_oct: [u8; 2],
    pub flags: u8,
}

impl VoxelEdit {
    pub fn place(coord: VoxelCoord, material: VoxelMaterialId, normal_oct: [u8; 2], flags: u8) -> Self {
        Self {
            op: VoxelEditOp::Place,
            coord,
            material,
            normal_oct,
            flags,
        }
    }

    pub fn remove(coord: VoxelCoord) -> Self {
        Self {
            op: VoxelEditOp::Remove,
            coord,
            material: VoxelMaterialId(0),
            normal_oct: [128, 128],
            flags: 0,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct VoxelEditBatch {
    pub edits: Vec<VoxelEdit>,
    pub request_support_check: bool,
    pub support_reason: Option<SupportReason>,
}

#[derive(Debug, Clone, Default)]
pub struct VoxelBatchResult {
    pub applied: usize,
    pub placed: usize,
    pub removed: usize,
    pub changed_coords: Vec<VoxelCoord>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupportReason {
    Damage,
    Remove,
    BatchDestructive,
    ExplicitValidation,
}

#[derive(Debug, Clone)]
pub struct SupportSolveJob {
    pub revision: u64,
    pub reason: SupportReason,
    pub changed_coords: Vec<VoxelCoord>,
    pub region_min: IVec3,
    pub region_max: IVec3,
    pub occupied_region: Vec<(VoxelCoord, u8)>,
    pub boundary_supported: Vec<VoxelCoord>,
    pub full_world_fallback: Option<Vec<(VoxelCoord, u8)>>,
}

#[derive(Debug, Clone, Default)]
pub struct SupportSolveResult {
    pub revision: u64,
    pub reason: Option<SupportReason>,
    pub unsupported: Vec<VoxelCoord>,
    pub used_full_world: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum BuildAudioEventKind {
    Hit,
    Crack,
    Break,
    CollapseStart,
    CollapseSettle,
}

#[derive(Debug, Clone, Copy)]
pub struct BuildAudioEvent {
    pub kind: BuildAudioEventKind,
    pub world_pos: Vec3,
    pub material: u8,
}

#[derive(Debug, Clone, Copy)]
pub struct VoxelAabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl VoxelAabb {
    pub fn include_point(&mut self, p: Vec3) {
        self.min = self.min.min(p);
        self.max = self.max.max(p);
    }
}

#[derive(Debug, Clone)]
pub struct ShellBakeJob {
    pub dirty_aabb: VoxelAabb,
    pub priority: u8,
    pub reason: &'static str,
}

#[derive(Debug, Clone)]
pub struct ShellBakeResult {
    pub sdf_bricks_updated: u32,
    pub timestamp_s: f32,
    pub bounds: VoxelAabb,
}

#[derive(Debug, Clone, Default)]
pub struct RenderDeltaBatch {
    pub dirty_chunks: Vec<IVec3>,
    pub bake_jobs: Vec<ShellBakeJob>,
    pub bake_results: Vec<ShellBakeResult>,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BrickNode {
    pub child_mask: u64,
    pub child_base_index: u32,
    pub leaf_payload_index: u32,
    pub lod_meta: u32,
    pub _pad0: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BrickLeaf64 {
    pub occupancy_mask: u64,
    pub material: [u8; 64],
    pub color_rgb: [[u8; 3]; 64],
    pub normal_oct: [[u8; 2]; 64],
    pub hp: [u16; 64],
}

impl Default for BrickLeaf64 {
    fn default() -> Self {
        Self {
            occupancy_mask: 0,
            material: [0; 64],
            color_rgb: [[0, 0, 0]; 64],
            normal_oct: [[128, 128]; 64],
            hp: [0; 64],
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RaymarchQualityState {
    pub dynamic_resolution_scale: f32,
    pub step_multiplier: f32,
    pub frame_budget_ms: f32,
}

impl Default for RaymarchQualityState {
    fn default() -> Self {
        Self {
            dynamic_resolution_scale: 1.0,
            step_multiplier: 1.0,
            frame_budget_ms: 16.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ShellBlendState {
    pub blend_t: f32,
    pub blend_duration_s: f32,
    pub preview_active: bool,
}

impl Default for ShellBlendState {
    fn default() -> Self {
        Self {
            blend_t: 1.0,
            blend_duration_s: 0.20,
            preview_active: false,
        }
    }
}
