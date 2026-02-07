use glam::Vec3;

use super::types::{
    BuildAudioEvent, BuildAudioEventKind, DamageSource, VoxelCell, VoxelDamageResult, VoxelHit,
};
use super::world::VoxelWorld;

pub fn material_max_hp(material: u8) -> u16 {
    match material {
        0 => 180,
        1 => 95,
        2 => 220,
        3 => 150,
        4 => 170,
        5 => 140,
        6 => 80,
        7 => 300,
        8 => 190,
        9 => 160,
        _ => 160,
    }
}

pub fn material_color(material: u8) -> [u8; 3] {
    match material {
        0 => [153, 153, 153],
        1 => [176, 124, 70],
        2 => [102, 102, 115],
        3 => [204, 179, 128],
        4 => [77, 77, 89],
        5 => [153, 77, 51],
        6 => [51, 102, 51],
        7 => [128, 128, 153],
        8 => [230, 230, 217],
        9 => [51, 51, 77],
        _ => [128, 128, 128],
    }
}

pub fn default_voxel_cell(material: u8, normal_oct: [u8; 2]) -> VoxelCell {
    let max_hp = material_max_hp(material);
    VoxelCell {
        material,
        hp: max_hp,
        max_hp,
        color_rgb: material_color(material),
        normal_oct,
    }
}

pub fn oct_encode_from_normal(normal: Vec3) -> [u8; 2] {
    let n = if normal.length_squared() > 1e-6 {
        normal.normalize()
    } else {
        Vec3::Y
    };
    let inv_l1 = 1.0 / (n.x.abs() + n.y.abs() + n.z.abs()).max(1e-6);
    let mut p = Vec3::new(n.x * inv_l1, n.y * inv_l1, n.z * inv_l1);
    if p.z < 0.0 {
        let x = (1.0 - p.y.abs()) * p.x.signum();
        let y = (1.0 - p.x.abs()) * p.y.signum();
        p.x = x;
        p.y = y;
    }
    let ex = ((p.x * 0.5 + 0.5) * 255.0).round().clamp(0.0, 255.0) as u8;
    let ey = ((p.y * 0.5 + 0.5) * 255.0).round().clamp(0.0, 255.0) as u8;
    [ex, ey]
}

pub fn apply_damage_at_hit(
    world: &mut VoxelWorld,
    hit: VoxelHit,
    damage: f32,
    _impulse: Vec3,
    source: DamageSource,
    audio_events: &mut Vec<BuildAudioEvent>,
) -> VoxelDamageResult {
    let Some(cell) = world.get_mut(hit.coord) else {
        return VoxelDamageResult {
            destroyed: false,
            remaining_hp: 0,
        };
    };

    let source_scale = match source {
        DamageSource::Cannonball => 1.0,
        DamageSource::Rocket => 1.25,
        DamageSource::HitscanGun => 0.22,
    };
    let applied = (damage.max(0.1) * source_scale).ceil() as u16;

    let prev_hp = cell.hp;
    cell.hp = cell.hp.saturating_sub(applied.max(1));
    let destroyed = cell.hp == 0;

    audio_events.push(BuildAudioEvent {
        kind: BuildAudioEventKind::Hit,
        world_pos: hit.world_pos,
        material: cell.material,
    });

    if !destroyed && cell.hp <= (cell.max_hp / 2) && prev_hp > (cell.max_hp / 2) {
        audio_events.push(BuildAudioEvent {
            kind: BuildAudioEventKind::Crack,
            world_pos: hit.world_pos,
            material: cell.material,
        });
    }

    if destroyed {
        let material = cell.material;
        let _ = cell;
        let _removed = world.remove(hit.coord);
        audio_events.push(BuildAudioEvent {
            kind: BuildAudioEventKind::Break,
            world_pos: hit.world_pos,
            material,
        });
    }

    VoxelDamageResult {
        destroyed,
        remaining_hp: world.get(hit.coord).map_or(0, |c| c.hp),
    }
}
