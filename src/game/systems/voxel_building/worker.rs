use std::collections::HashSet;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

use super::connectivity::unsupported_from_region;
use super::damage::{apply_damage_at_hit, default_voxel_cell};
use super::shell_bake::ShellBakeScheduler;
use super::types::{
    BuildAudioEvent, RenderDeltaBatch, ShellBakeResult, SupportSolveJob, SupportSolveResult,
    VoxelCoord, VoxelDamageResult, VoxelHit, VOXEL_FLAG_TERRAIN_ANCHORED,
};
use super::world::VoxelWorld;

pub enum WorkerCommand {
    Place { coord: VoxelCoord, material: u8 },
    Remove { coord: VoxelCoord },
    Damage {
        hit: VoxelHit,
        damage: f32,
        impulse: glam::Vec3,
        source: super::types::DamageSource,
    },
    SupportSolve(SupportSolveJob),
    Tick { dt: f32 },
    Shutdown,
}

pub enum WorkerEvent {
    Render(RenderDeltaBatch),
    Audio(Vec<BuildAudioEvent>),
    DamageResult(VoxelDamageResult),
    BakeResult(ShellBakeResult),
    SupportSolved(SupportSolveResult),
}

pub struct VoxelWorker {
    tx_cmd: Sender<WorkerCommand>,
    rx_evt: Receiver<WorkerEvent>,
    thread: Option<JoinHandle<()>>,
}

impl VoxelWorker {
    pub fn spawn() -> Self {
        let (tx_cmd, rx_cmd) = mpsc::channel::<WorkerCommand>();
        let (tx_evt, rx_evt) = mpsc::channel::<WorkerEvent>();

        let thread = thread::Builder::new()
            .name("voxel-build-worker".to_string())
            .spawn(move || worker_loop(rx_cmd, tx_evt))
            .expect("failed to spawn voxel worker");

        Self {
            tx_cmd,
            rx_evt,
            thread: Some(thread),
        }
    }

    pub fn send(&self, cmd: WorkerCommand) -> bool {
        self.tx_cmd.send(cmd).is_ok()
    }

    pub fn try_recv(&self) -> Option<WorkerEvent> {
        self.rx_evt.try_recv().ok()
    }
}

impl Drop for VoxelWorker {
    fn drop(&mut self) {
        let _ = self.tx_cmd.send(WorkerCommand::Shutdown);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn worker_loop(rx_cmd: Receiver<WorkerCommand>, tx_evt: Sender<WorkerEvent>) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Some(cores) = core_affinity::get_core_ids()
            && cores.len() > 1
        {
            let _ = core_affinity::set_for_current(cores[1]);
        }
    }

    let mut world = VoxelWorld::new();
    let mut audio = Vec::new();
    let mut bakes = ShellBakeScheduler::new();

    while let Ok(cmd) = rx_cmd.recv() {
        match cmd {
            WorkerCommand::Place { coord, material } => {
                let normal = [128, 128];
                let cell = default_voxel_cell(material, normal);
                world.place(coord, cell);
                bakes.mark_voxel_dirty(coord);
            }
            WorkerCommand::Remove { coord } => {
                world.remove(coord);
                bakes.mark_voxel_dirty(coord);
            }
            WorkerCommand::Damage {
                hit,
                damage,
                impulse,
                source,
            } => {
                let result = apply_damage_at_hit(&mut world, hit, damage, impulse, source, &mut audio);
                bakes.mark_voxel_dirty(hit.coord);
                let _ = tx_evt.send(WorkerEvent::DamageResult(result));
            }
            WorkerCommand::Tick { dt } => {
                let mut batch = RenderDeltaBatch::default();
                batch.dirty_chunks = world.drain_dirty_chunks();
                batch.bake_jobs = bakes.tick(dt);
                let _ = tx_evt.send(WorkerEvent::Render(batch));
                if !audio.is_empty() {
                    let drained = std::mem::take(&mut audio);
                    let _ = tx_evt.send(WorkerEvent::Audio(drained));
                }
            }
            WorkerCommand::SupportSolve(job) => {
                let result = solve_support_job(job);
                let _ = tx_evt.send(WorkerEvent::SupportSolved(result));
            }
            WorkerCommand::Shutdown => break,
        }
    }
}

fn solve_support_job(job: SupportSolveJob) -> SupportSolveResult {
    let (occupied_cells, used_full_world) = if let Some(full) = job.full_world_fallback.clone() {
        (full, true)
    } else {
        (job.occupied_region.clone(), false)
    };

    let occupied_region: HashSet<VoxelCoord> = occupied_cells.iter().map(|(coord, _)| *coord).collect();
    let anchored_region: HashSet<VoxelCoord> = occupied_cells
        .iter()
        .filter_map(|(coord, flags)| {
            if (*flags & VOXEL_FLAG_TERRAIN_ANCHORED) != 0 || coord.y <= 0 {
                Some(*coord)
            } else {
                None
            }
        })
        .collect();
    let boundary_supported: HashSet<VoxelCoord> = job.boundary_supported.iter().copied().collect();

    let unsupported = unsupported_from_region(&occupied_region, &anchored_region, &boundary_supported);

    SupportSolveResult {
        revision: job.revision,
        reason: Some(job.reason),
        unsupported,
        used_full_world,
    }
}
