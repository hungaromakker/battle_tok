//! Scene Coordinator
//!
//! High-level scene management that coordinates render passes,
//! camera state, and frame submission.

use glam::{Mat4, Vec3};
use std::sync::Arc;
use std::time::Instant;
use winit::window::Window;

use super::gpu_context::{GpuContext, GpuContextConfig};
use super::render_pass::{FrameContext, RenderContext, RenderPassManager};

/// Camera data for rendering
#[derive(Clone, Copy)]
pub struct CameraState {
    pub position: Vec3,
    pub view_matrix: Mat4,
    pub projection_matrix: Mat4,
}

impl Default for CameraState {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            view_matrix: Mat4::IDENTITY,
            projection_matrix: Mat4::perspective_rh(
                std::f32::consts::FRAC_PI_4,
                16.0 / 9.0,
                0.1,
                1000.0,
            ),
        }
    }
}

/// Scene coordinator manages the entire rendering pipeline
pub struct SceneCoordinator {
    gpu: GpuContext,
    passes: RenderPassManager,
    camera: CameraState,
    start_time: Instant,
    last_frame: Instant,
    frame_count: u64,
    fps: f32,
    last_fps_update: Instant,
}

impl SceneCoordinator {
    /// Create a new scene coordinator
    pub fn new(window: Arc<Window>, config: GpuContextConfig) -> Self {
        let gpu = GpuContext::new(window, config);
        let now = Instant::now();

        Self {
            gpu,
            passes: RenderPassManager::new(),
            camera: CameraState::default(),
            start_time: now,
            last_frame: now,
            frame_count: 0,
            fps: 0.0,
            last_fps_update: now,
        }
    }

    /// Get reference to GPU context
    pub fn gpu(&self) -> &GpuContext {
        &self.gpu
    }

    /// Get mutable reference to GPU context
    pub fn gpu_mut(&mut self) -> &mut GpuContext {
        &mut self.gpu
    }

    /// Get the device for external resource creation
    pub fn device(&self) -> &wgpu::Device {
        &self.gpu.device
    }

    /// Get the queue for buffer writes
    pub fn queue(&self) -> &wgpu::Queue {
        &self.gpu.queue
    }

    /// Get surface format
    pub fn format(&self) -> wgpu::TextureFormat {
        self.gpu.format()
    }

    /// Get current dimensions
    pub fn dimensions(&self) -> (u32, u32) {
        self.gpu.dimensions()
    }

    /// Get render pass manager for adding passes
    pub fn passes_mut(&mut self) -> &mut RenderPassManager {
        &mut self.passes
    }

    /// Initialize all render passes
    pub fn initialize_passes(&mut self) {
        let (width, height) = self.gpu.dimensions();
        let ctx = RenderContext {
            device: &self.gpu.device,
            queue: &self.gpu.queue,
            surface_format: self.gpu.format(),
            width,
            height,
        };
        self.passes.initialize(&ctx);
    }

    /// Update camera state
    pub fn set_camera(&mut self, position: Vec3, view: Mat4, projection: Mat4) {
        self.camera = CameraState {
            position,
            view_matrix: view,
            projection_matrix: projection,
        };
    }

    /// Handle window resize
    pub fn resize(&mut self, width: u32, height: u32) {
        self.gpu.resize(width, height);
        let ctx = RenderContext {
            device: &self.gpu.device,
            queue: &self.gpu.queue,
            surface_format: self.gpu.format(),
            width,
            height,
        };
        self.passes.resize(&ctx, width, height);
    }

    /// Get elapsed time since start
    pub fn elapsed_time(&self) -> f32 {
        self.start_time.elapsed().as_secs_f32()
    }

    /// Get current FPS
    pub fn fps(&self) -> f32 {
        self.fps
    }

    /// Get frame count
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    /// Update passes and calculate delta time
    pub fn update(&mut self) -> f32 {
        let now = Instant::now();
        let delta_time = (now - self.last_frame).as_secs_f32();
        self.last_frame = now;
        self.frame_count += 1;

        // Update FPS every second
        let fps_elapsed = (now - self.last_fps_update).as_secs_f32();
        if fps_elapsed >= 1.0 {
            self.fps = self.frame_count as f32 / fps_elapsed;
            self.frame_count = 0;
            self.last_fps_update = now;
        }

        // Update all passes
        let (width, height) = self.gpu.dimensions();
        let ctx = RenderContext {
            device: &self.gpu.device,
            queue: &self.gpu.queue,
            surface_format: self.gpu.format(),
            width,
            height,
        };
        self.passes.update(&ctx, delta_time);

        delta_time
    }

    /// Render a frame using all enabled passes
    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.gpu.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Scene Render Encoder"),
            });

        // Create frame context
        let view_proj =
            (self.camera.projection_matrix * self.camera.view_matrix).to_cols_array_2d();
        let camera_pos = self.camera.position.to_array();
        let time = self.elapsed_time();
        let (width, height) = self.gpu.dimensions();

        let ctx = RenderContext {
            device: &self.gpu.device,
            queue: &self.gpu.queue,
            surface_format: self.gpu.format(),
            width,
            height,
        };
        let mut frame = FrameContext {
            encoder: &mut encoder,
            color_view: &view,
            depth_view: &self.gpu.depth_view,
            time,
            delta_time: 0.016, // Approximate, actual delta calculated in update()
            view_proj,
            camera_pos,
        };

        // Render all passes
        self.passes.render(&ctx, &mut frame);

        // Submit command buffer
        self.gpu.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }

    /// Begin a custom render pass (for manual control)
    pub fn begin_frame(
        &mut self,
    ) -> Result<(wgpu::SurfaceTexture, wgpu::CommandEncoder), wgpu::SurfaceError> {
        let output = self.gpu.get_current_texture()?;
        let encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Custom Render Encoder"),
            });
        Ok((output, encoder))
    }

    /// End a custom render pass
    pub fn end_frame(&self, output: wgpu::SurfaceTexture, encoder: wgpu::CommandEncoder) {
        self.gpu.queue.submit(std::iter::once(encoder.finish()));
        output.present();
    }
}
