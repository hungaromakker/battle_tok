//! Ember/Ash Particle System for Atmospheric Effects
//!
//! This module provides a GPU-instanced particle system for rendering floating
//! embers and ash rising from lava. Particles are rendered as camera-facing
//! billboards with additive blending for a glowing effect.


/// Maximum number of particles supported (for 500+ at 60fps requirement)
pub const MAX_PARTICLES: usize = 1024;

/// Size of a single GPU particle in bytes (32 bytes)
pub const GPU_PARTICLE_SIZE: usize = std::mem::size_of::<GpuParticle>();

/// Total buffer size in bytes for particle data
pub const PARTICLE_BUFFER_SIZE: usize = MAX_PARTICLES * GPU_PARTICLE_SIZE;

/// Particle uniforms buffer size (128 bytes for view + proj matrices)
pub const PARTICLE_UNIFORMS_SIZE: usize = 128;

/// GPU-compatible particle data structure.
///
/// Layout (32 bytes total, properly aligned for GPU compatibility):
/// - position: vec3<f32> (12 bytes) - World position of the particle
/// - lifetime: f32 (4 bytes) - Remaining lifetime (1.0 = just spawned, 0.0 = dead)
/// - size: f32 (4 bytes) - Billboard size in world units
/// - color: vec3<f32> (12 bytes) - RGB color (HDR values > 1.0 for glow)
///
/// Total: 12 + 4 + 4 + 12 = 32 bytes
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuParticle {
    /// World position (x, y, z) - 12 bytes
    pub position: [f32; 3],
    /// Remaining lifetime (1.0 = just spawned, 0.0 = dead) - 4 bytes
    pub lifetime: f32,
    /// Billboard size in world units - 4 bytes
    pub size: f32,
    /// RGB color (HDR values for glow) - 12 bytes
    pub color: [f32; 3],
}

// Compile-time assertion to verify struct size is exactly 32 bytes
const _: () = {
    assert!(
        std::mem::size_of::<GpuParticle>() == 32,
        "GpuParticle must be exactly 32 bytes for GPU compatibility"
    );
};

impl Default for GpuParticle {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0, 0.0],
            lifetime: 0.0, // Dead by default
            size: 0.1,
            color: [2.0, 0.6, 0.1], // Warm ember orange (HDR)
        }
    }
}

/// CPU-side particle data with additional physics state.
///
/// This struct tracks velocity and other state not needed on GPU.
#[derive(Copy, Clone, Debug)]
pub struct Particle {
    /// World position
    pub position: [f32; 3],
    /// Velocity (m/s)
    pub velocity: [f32; 3],
    /// Remaining lifetime (1.0 = just spawned, 0.0 = dead)
    pub lifetime: f32,
    /// Lifetime decay rate (per second)
    pub decay_rate: f32,
    /// Billboard size in world units
    pub size: f32,
    /// RGB color (HDR values for glow)
    pub color: [f32; 3],
}

impl Default for Particle {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0, 0.0],
            velocity: [0.0, 1.0, 0.0], // Rise upward by default
            lifetime: 1.0,
            decay_rate: 0.33, // ~3 second lifetime
            size: 0.15,
            color: [2.0, 0.6, 0.1], // Warm ember orange (HDR)
        }
    }
}

impl Particle {
    /// Check if this particle is still alive.
    #[inline]
    pub fn is_alive(&self) -> bool {
        self.lifetime > 0.0
    }

    /// Convert to GPU-compatible format.
    pub fn to_gpu(&self) -> GpuParticle {
        GpuParticle {
            position: self.position,
            lifetime: self.lifetime,
            size: self.size,
            color: self.color,
        }
    }
}

/// GPU-compatible particle uniforms structure.
///
/// Layout (128 bytes total):
/// - view: mat4x4<f32> (64 bytes) - View matrix for billboard calculation
/// - proj: mat4x4<f32> (64 bytes) - Projection matrix
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ParticleUniforms {
    /// View matrix (64 bytes)
    pub view: [[f32; 4]; 4],
    /// Projection matrix (64 bytes)
    pub proj: [[f32; 4]; 4],
}

impl Default for ParticleUniforms {
    fn default() -> Self {
        Self {
            view: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            proj: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }
}

// Compile-time assertion for uniforms size
const _: () = {
    assert!(
        std::mem::size_of::<ParticleUniforms>() == 128,
        "ParticleUniforms must be exactly 128 bytes"
    );
};

/// Simple pseudo-random number generator for particle variation.
/// Uses a basic xorshift algorithm for fast, deterministic randomness.
struct SimpleRng {
    state: u32,
}

impl SimpleRng {
    fn new(seed: u32) -> Self {
        Self { state: seed.max(1) }
    }

    /// Generate a random f32 in [0.0, 1.0)
    fn next_f32(&mut self) -> f32 {
        // xorshift32
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        // Convert to [0, 1) range
        (x as f32) / (u32::MAX as f32)
    }

    /// Generate a random f32 in [min, max)
    fn range(&mut self, min: f32, max: f32) -> f32 {
        min + self.next_f32() * (max - min)
    }
}

/// Particle System Manager for ember/ash effects.
///
/// This manager:
/// - Maintains a pool of particles with CPU-side physics
/// - Updates particle positions and lifetimes each frame
/// - Spawns new particles at configurable positions
/// - Uploads active particles to GPU buffer for rendering
pub struct ParticleSystem {
    /// CPU-side particle pool
    particles: Vec<Particle>,
    /// Random number generator for variation
    rng: SimpleRng,
    /// GPU buffer for particle data (storage buffer)
    particle_buffer: wgpu::Buffer,
    /// GPU buffer for particle uniforms
    uniform_buffer: wgpu::Buffer,
    /// Bind group layout for shader binding
    bind_group_layout: wgpu::BindGroupLayout,
    /// Bind group for shader access
    bind_group: wgpu::BindGroup,
    /// Render pipeline for particle rendering
    pipeline: wgpu::RenderPipeline,
    /// Number of currently active (alive) particles
    active_count: usize,
    /// Spawn positions for embers (typically above lava)
    spawn_positions: Vec<[f32; 3]>,
    /// Time accumulator for spawn rate control
    spawn_accumulator: f32,
    /// Particles to spawn per second
    spawn_rate: f32,
}

impl ParticleSystem {
    /// Create a new ParticleSystem with empty particles.
    ///
    /// # Arguments
    /// * `device` - The wgpu device to create GPU resources on
    /// * `surface_format` - The format of the render target
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        // Create buffer for particles (storage buffer, read-only in shader)
        let particle_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Particle Storage Buffer"),
            size: PARTICLE_BUFFER_SIZE as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create buffer for uniforms (view/projection matrices)
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Particle Uniform Buffer"),
            size: PARTICLE_UNIFORMS_SIZE as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Particle Bind Group Layout"),
            entries: &[
                // Binding 0: Particle uniforms (view/projection matrices)
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Binding 1: Particle storage buffer (read-only)
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // Create bind group
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Particle Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: particle_buffer.as_entire_binding(),
                },
            ],
        });

        // Load shader
        let shader_source = include_str!("../../../shaders/ember_particle.wgsl");
        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Ember Particle Shader"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Particle Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create render pipeline with additive blending
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Particle Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: Some("vs_particle"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[], // No vertex buffers, all data from storage buffer
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None, // Don't cull - billboards should be visible from both sides
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false, // Don't write to depth - particles are translucent
                depth_compare: wgpu::CompareFunction::Less, // Still test depth
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader_module,
                entry_point: Some("fs_particle"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    // Additive blending: result = src * srcAlpha + dst * 1
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::One, // Additive!
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent::OVER,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview: None,
            cache: None,
        });

        Self {
            particles: Vec::with_capacity(MAX_PARTICLES),
            rng: SimpleRng::new(12345), // Deterministic seed
            particle_buffer,
            uniform_buffer,
            bind_group_layout,
            bind_group,
            pipeline,
            active_count: 0,
            spawn_positions: Vec::new(),
            spawn_accumulator: 0.0,
            spawn_rate: 50.0, // 50 particles per second by default
        }
    }

    /// Add a spawn position where embers will be emitted.
    ///
    /// Embers spawn near these positions with slight random offset.
    pub fn add_spawn_position(&mut self, position: [f32; 3]) {
        self.spawn_positions.push(position);
    }

    /// Set all spawn positions at once.
    pub fn set_spawn_positions(&mut self, positions: Vec<[f32; 3]>) {
        self.spawn_positions = positions;
    }

    /// Clear all spawn positions.
    pub fn clear_spawn_positions(&mut self) {
        self.spawn_positions.clear();
    }

    /// Set the spawn rate (particles per second).
    pub fn set_spawn_rate(&mut self, rate: f32) {
        self.spawn_rate = rate.max(0.0);
    }

    /// Spawn a single ember particle at the given position.
    ///
    /// The particle will float upward with slight random drift.
    pub fn spawn_ember(&mut self, position: [f32; 3]) {
        if self.particles.len() >= MAX_PARTICLES {
            // Pool is full, overwrite oldest dead particle or skip
            // Find a dead particle to reuse
            if let Some(dead_idx) = self.particles.iter().position(|p| !p.is_alive()) {
                let ember = self.create_ember(position);
                self.particles[dead_idx] = ember;
            }
            // If no dead particles, we're at capacity - skip spawning
            return;
        }

        let ember = self.create_ember(position);
        self.particles.push(ember);
    }

    /// Create a new ember particle with randomized properties.
    fn create_ember(&mut self, base_position: [f32; 3]) -> Particle {
        // Random offset from spawn position (small radius)
        let offset_x = self.rng.range(-0.5, 0.5);
        let offset_z = self.rng.range(-0.5, 0.5);

        // Random upward velocity with slight horizontal drift
        let vel_y = self.rng.range(0.8, 1.5); // Upward speed
        let vel_x = self.rng.range(-0.3, 0.3); // Horizontal drift
        let vel_z = self.rng.range(-0.3, 0.3);

        // Random lifetime between 2-4 seconds
        let lifetime_seconds = self.rng.range(2.0, 4.0);
        let decay_rate = 1.0 / lifetime_seconds;

        // Random size
        let size = self.rng.range(0.08, 0.2);

        // Color variation - orange to yellow
        let color_variation = self.rng.next_f32();
        let color = [
            2.0 + color_variation * 0.5, // R: 2.0 - 2.5
            0.4 + color_variation * 0.4, // G: 0.4 - 0.8 (more yellow when high)
            0.1,                          // B: constant low
        ];

        Particle {
            position: [
                base_position[0] + offset_x,
                base_position[1],
                base_position[2] + offset_z,
            ],
            velocity: [vel_x, vel_y, vel_z],
            lifetime: 1.0,
            decay_rate,
            size,
            color,
        }
    }

    /// Update all particles and spawn new ones.
    ///
    /// # Arguments
    /// * `dt` - Delta time in seconds
    pub fn update(&mut self, dt: f32) {
        // Update existing particles
        for particle in self.particles.iter_mut() {
            if particle.is_alive() {
                // Update position based on velocity
                particle.position[0] += particle.velocity[0] * dt;
                particle.position[1] += particle.velocity[1] * dt;
                particle.position[2] += particle.velocity[2] * dt;

                // Add slight sinusoidal drift for more organic movement
                // Use position as seed for variation
                let drift_seed = particle.position[0] + particle.position[2];
                let drift = (drift_seed * 3.0).sin() * 0.1 * dt;
                particle.position[0] += drift;

                // Decay lifetime
                particle.lifetime -= particle.decay_rate * dt;
                if particle.lifetime < 0.0 {
                    particle.lifetime = 0.0;
                }

                // Slow down horizontal velocity over time (drag)
                particle.velocity[0] *= 1.0 - 0.5 * dt;
                particle.velocity[2] *= 1.0 - 0.5 * dt;
            }
        }

        // Spawn new particles based on spawn rate
        if !self.spawn_positions.is_empty() {
            self.spawn_accumulator += self.spawn_rate * dt;

            while self.spawn_accumulator >= 1.0 {
                self.spawn_accumulator -= 1.0;

                // Pick a random spawn position
                let spawn_idx = (self.rng.next_f32() * self.spawn_positions.len() as f32) as usize;
                let spawn_idx = spawn_idx.min(self.spawn_positions.len() - 1);
                let spawn_pos = self.spawn_positions[spawn_idx];

                self.spawn_ember(spawn_pos);
            }
        }

        // Count active particles
        self.active_count = self.particles.iter().filter(|p| p.is_alive()).count();
    }

    /// Upload particle data to GPU buffer.
    ///
    /// # Arguments
    /// * `queue` - The wgpu queue for submitting buffer writes
    pub fn upload_particles(&self, queue: &wgpu::Queue) {
        if self.active_count == 0 {
            return;
        }

        // Collect alive particles into GPU format
        let gpu_particles: Vec<GpuParticle> = self
            .particles
            .iter()
            .filter(|p| p.is_alive())
            .map(|p| p.to_gpu())
            .collect();

        // Write to buffer
        queue.write_buffer(&self.particle_buffer, 0, bytemuck::cast_slice(&gpu_particles));
    }

    /// Update uniform buffer with view and projection matrices.
    ///
    /// # Arguments
    /// * `queue` - The wgpu queue for submitting buffer writes
    /// * `view` - View matrix (4x4)
    /// * `proj` - Projection matrix (4x4)
    pub fn update_uniforms(&self, queue: &wgpu::Queue, view: [[f32; 4]; 4], proj: [[f32; 4]; 4]) {
        let uniforms = ParticleUniforms { view, proj };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
    }

    /// Render all active particles.
    ///
    /// # Arguments
    /// * `render_pass` - The render pass to draw into
    pub fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        if self.active_count == 0 {
            return;
        }

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);

        // Draw: 6 vertices per particle (two triangles), N instances
        let vertex_count = 6;
        let instance_count = self.active_count as u32;
        render_pass.draw(0..vertex_count, 0..instance_count);
    }

    /// Get the current number of active particles.
    pub fn active_count(&self) -> usize {
        self.active_count
    }

    /// Get the total particle capacity.
    pub fn capacity(&self) -> usize {
        MAX_PARTICLES
    }

    /// Clear all particles.
    pub fn clear(&mut self) {
        self.particles.clear();
        self.active_count = 0;
    }

    /// Get the bind group layout for external pipeline creation.
    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    /// Get the render pipeline.
    pub fn pipeline(&self) -> &wgpu::RenderPipeline {
        &self.pipeline
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gpu_particle_size() {
        assert_eq!(std::mem::size_of::<GpuParticle>(), 32);
    }

    #[test]
    fn test_particle_buffer_size() {
        assert_eq!(PARTICLE_BUFFER_SIZE, 32768); // 1024 * 32 = 32 KB
    }

    #[test]
    fn test_max_particles() {
        assert_eq!(MAX_PARTICLES, 1024);
    }

    #[test]
    fn test_default_gpu_particle() {
        let particle = GpuParticle::default();
        assert_eq!(particle.position, [0.0, 0.0, 0.0]);
        assert_eq!(particle.lifetime, 0.0); // Dead by default
        assert_eq!(particle.size, 0.1);
    }

    #[test]
    fn test_particle_to_gpu() {
        let particle = Particle {
            position: [1.0, 2.0, 3.0],
            velocity: [0.0, 1.0, 0.0],
            lifetime: 0.5,
            decay_rate: 0.33,
            size: 0.2,
            color: [2.0, 0.6, 0.1],
        };

        let gpu = particle.to_gpu();
        assert_eq!(gpu.position, [1.0, 2.0, 3.0]);
        assert_eq!(gpu.lifetime, 0.5);
        assert_eq!(gpu.size, 0.2);
        assert_eq!(gpu.color, [2.0, 0.6, 0.1]);
    }

    #[test]
    fn test_particle_is_alive() {
        let alive = Particle {
            lifetime: 0.5,
            ..Default::default()
        };
        let dead = Particle {
            lifetime: 0.0,
            ..Default::default()
        };

        assert!(alive.is_alive());
        assert!(!dead.is_alive());
    }

    #[test]
    fn test_uniforms_size() {
        assert_eq!(std::mem::size_of::<ParticleUniforms>(), 128);
    }

    #[test]
    fn test_simple_rng() {
        let mut rng = SimpleRng::new(42);

        // Should produce deterministic results
        let v1 = rng.next_f32();
        let v2 = rng.next_f32();

        // Values should be in [0, 1) range
        assert!(v1 >= 0.0 && v1 < 1.0);
        assert!(v2 >= 0.0 && v2 < 1.0);

        // Values should be different
        assert_ne!(v1, v2);

        // Same seed should produce same sequence
        let mut rng2 = SimpleRng::new(42);
        assert_eq!(rng2.next_f32(), v1);
    }

    #[test]
    fn test_simple_rng_range() {
        let mut rng = SimpleRng::new(42);

        for _ in 0..100 {
            let v = rng.range(10.0, 20.0);
            assert!(v >= 10.0 && v < 20.0);
        }
    }
}
