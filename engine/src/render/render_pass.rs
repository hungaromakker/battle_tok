//! Render Pass Abstraction
//!
//! Provides a trait-based system for defining reusable render passes.
//! Each pass can be enabled/disabled and has a defined execution order.

use wgpu::{CommandEncoder, Device, Queue, TextureView};

/// Render pass execution priority (lower = earlier)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RenderPassPriority {
    /// Background rendering (skybox, environment)
    Background = 0,
    /// Main geometry pass (terrain, meshes)
    Geometry = 100,
    /// Translucent/alpha objects
    Translucent = 200,
    /// Post-processing effects
    PostProcess = 300,
    /// UI overlay (always on top)
    UI = 400,
}

/// GPU context shared between render passes
pub struct RenderContext<'a> {
    pub device: &'a Device,
    pub queue: &'a Queue,
    pub surface_format: wgpu::TextureFormat,
    pub width: u32,
    pub height: u32,
}

/// Frame context for a single render frame
pub struct FrameContext<'a> {
    pub encoder: &'a mut CommandEncoder,
    pub color_view: &'a TextureView,
    pub depth_view: &'a TextureView,
    pub time: f32,
    pub delta_time: f32,
    pub view_proj: [[f32; 4]; 4],
    pub camera_pos: [f32; 3],
}

/// Trait for implementing render passes
pub trait RenderPass {
    /// Unique name for this pass (for debugging/profiling)
    fn name(&self) -> &'static str;

    /// Execution priority (determines render order)
    fn priority(&self) -> RenderPassPriority;

    /// Whether this pass is currently enabled
    fn is_enabled(&self) -> bool {
        true
    }

    /// Set enabled state
    fn set_enabled(&mut self, _enabled: bool) {}

    /// Initialize GPU resources (called once on creation)
    fn initialize(&mut self, ctx: &RenderContext);

    /// Handle window resize
    fn resize(&mut self, _ctx: &RenderContext, _width: u32, _height: u32) {}

    /// Update pass state (called each frame before render)
    fn update(&mut self, _ctx: &RenderContext, _delta_time: f32) {}

    /// Execute the render pass
    fn render(&self, ctx: &RenderContext, frame: &mut FrameContext);
}

/// Manages a collection of render passes with automatic ordering
pub struct RenderPassManager {
    passes: Vec<Box<dyn RenderPass>>,
    sorted: bool,
}

impl RenderPassManager {
    pub fn new() -> Self {
        Self {
            passes: Vec::new(),
            sorted: false,
        }
    }

    /// Add a render pass to the manager
    pub fn add_pass(&mut self, pass: Box<dyn RenderPass>) {
        self.passes.push(pass);
        self.sorted = false;
    }

    /// Initialize all passes
    pub fn initialize(&mut self, ctx: &RenderContext) {
        for pass in &mut self.passes {
            pass.initialize(ctx);
        }
        self.sort_passes();
    }

    /// Sort passes by priority
    fn sort_passes(&mut self) {
        if !self.sorted {
            self.passes.sort_by_key(|p| p.priority());
            self.sorted = true;
        }
    }

    /// Handle window resize
    pub fn resize(&mut self, ctx: &RenderContext, width: u32, height: u32) {
        for pass in &mut self.passes {
            pass.resize(ctx, width, height);
        }
    }

    /// Update all passes
    pub fn update(&mut self, ctx: &RenderContext, delta_time: f32) {
        for pass in &mut self.passes {
            if pass.is_enabled() {
                pass.update(ctx, delta_time);
            }
        }
    }

    /// Render all enabled passes in priority order
    pub fn render(&self, ctx: &RenderContext, frame: &mut FrameContext) {
        for pass in &self.passes {
            if pass.is_enabled() {
                pass.render(ctx, frame);
            }
        }
    }

    /// Get a pass by name (for configuration)
    pub fn get_pass_mut(&mut self, name: &str) -> Option<&mut Box<dyn RenderPass>> {
        self.passes.iter_mut().find(|p| p.name() == name)
    }

    /// Enable/disable a pass by name
    pub fn set_pass_enabled(&mut self, name: &str, enabled: bool) {
        if let Some(pass) = self.get_pass_mut(name) {
            pass.set_enabled(enabled);
        }
    }

    /// List all pass names with their enabled status
    pub fn list_passes(&self) -> Vec<(&'static str, bool)> {
        self.passes
            .iter()
            .map(|p| (p.name(), p.is_enabled()))
            .collect()
    }
}

impl Default for RenderPassManager {
    fn default() -> Self {
        Self::new()
    }
}
