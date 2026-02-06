//! Battle Editor - Asset Creation Tool
//!
//! Standalone binary for creating game assets through a multi-stage pipeline.
//! Separate from the game binary (battle_arena).
//!
//! Run with: `cargo run --bin battle_editor`
//!
//! Controls:
//! - 1: Draw 2D stage
//! - 2: Extrude stage
//! - 3: Sculpt stage
//! - 4: Color stage
//! - 5: Save stage
//! - Middle mouse drag: Orbit camera (stages 2-5)
//! - Right mouse drag: Pan camera (stages 2-5)
//! - Scroll wheel: Zoom camera (stages 2-5)
//! - Ctrl+Z: Undo
//! - Ctrl+Y: Redo
//! - ESC: Exit

use std::sync::Arc;
use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowAttributes, WindowId};

use battle_tok_engine::game::asset_editor::orbit_camera::OrbitMouseButton;
use battle_tok_engine::game::asset_editor::AssetEditor;

// ============================================================================
// GPU RESOURCES (minimal for editor)
// ============================================================================

/// Minimal GPU state for the editor window.
struct EditorGpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
}

impl EditorGpu {
    fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.surface_config.width = new_size.width;
            self.surface_config.height = new_size.height;
            self.surface.configure(&self.device, &self.surface_config);
        }
    }
}

// ============================================================================
// APPLICATION
// ============================================================================

/// The main editor application struct.
struct BattleEditorApp {
    window: Option<Arc<Window>>,
    gpu: Option<EditorGpu>,
    editor: AssetEditor,

    // Input state
    /// Whether Ctrl key is currently held (for keyboard shortcuts)
    ctrl_held: bool,

    // Timing
    start_time: Instant,
    last_frame: Instant,
    frame_count: u64,
    fps: f32,
    last_fps_update: Instant,
}

impl BattleEditorApp {
    fn new() -> Self {
        Self {
            window: None,
            gpu: None,
            editor: AssetEditor::new(),
            ctrl_held: false,
            start_time: Instant::now(),
            last_frame: Instant::now(),
            frame_count: 0,
            fps: 0.0,
            last_fps_update: Instant::now(),
        }
    }

    /// Initialize wgpu device and surface. Follows the battle_arena.rs pattern.
    fn initialize(&mut self, window: Arc<Window>) {
        let size = window.inner_size();

        // Create wgpu instance
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance.create_surface(Arc::clone(&window)).unwrap();

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("Failed to find GPU adapter");

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("Battle Editor Device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::Performance,
            ..Default::default()
        }))
        .expect("Failed to create GPU device");

        // Configure surface
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let present_mode = if surface_caps
            .present_modes
            .contains(&wgpu::PresentMode::AutoVsync)
        {
            wgpu::PresentMode::AutoVsync
        } else {
            surface_caps.present_modes[0]
        };

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        self.gpu = Some(EditorGpu {
            device,
            queue,
            surface,
            surface_config,
        });

        // Set initial window title
        window.set_title(&self.editor.window_title());
        self.window = Some(window);

        println!("GPU initialized successfully");
        println!(
            "Surface format: {:?}, Present mode: {:?}",
            surface_format, present_mode
        );
    }

    /// Handle keyboard input.
    fn handle_key(&mut self, key: KeyCode, pressed: bool) {
        // Track Ctrl modifier state (pressed or released)
        match key {
            KeyCode::ControlLeft | KeyCode::ControlRight => {
                self.ctrl_held = pressed;
                return;
            }
            _ => {}
        }

        if !pressed {
            return;
        }

        // Ctrl+Z => Undo
        if key == KeyCode::KeyZ && self.ctrl_held {
            if let Some(cmd) = self.editor.undo_stack.undo() {
                println!("Undo: {:?}", std::mem::discriminant(cmd));
            }
            return;
        }

        // Ctrl+Y => Redo
        if key == KeyCode::KeyY && self.ctrl_held {
            if let Some(cmd) = self.editor.undo_stack.redo() {
                println!("Redo: {:?}", std::mem::discriminant(cmd));
            }
            return;
        }

        let stage_key = match key {
            KeyCode::Digit1 => Some(1u32),
            KeyCode::Digit2 => Some(2),
            KeyCode::Digit3 => Some(3),
            KeyCode::Digit4 => Some(4),
            KeyCode::Digit5 => Some(5),
            _ => None,
        };

        if let Some(key_num) = stage_key {
            if self.editor.set_stage_by_key(key_num) {
                // Update window title to reflect new stage
                if let Some(window) = &self.window {
                    window.set_title(&self.editor.window_title());
                }
            }
        }
    }

    /// Render a frame -- clear to dark gray background.
    fn render(&mut self) {
        let gpu = match &self.gpu {
            Some(gpu) => gpu,
            None => return,
        };

        let output = match gpu.surface.get_current_texture() {
            Ok(output) => output,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                // Reconfigure surface on lost/outdated
                gpu.surface.configure(&gpu.device, &gpu.surface_config);
                return;
            }
            Err(wgpu::SurfaceError::OutOfMemory) => {
                eprintln!("Out of GPU memory!");
                return;
            }
            Err(e) => {
                eprintln!("Surface error: {:?}", e);
                return;
            }
        };

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Editor Render Encoder"),
            });

        // Clear to dark gray background (0.12, 0.12, 0.14)
        {
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Editor Clear Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.12,
                            g: 0.12,
                            b: 0.14,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            // Render pass ends here (drop)
        }

        gpu.queue.submit(std::iter::once(encoder.finish()));
        output.present();
    }
}

// ============================================================================
// APPLICATION HANDLER
// ============================================================================

impl ApplicationHandler for BattleEditorApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let attrs = WindowAttributes::default()
                .with_title("Battle T\u{00f6}k \u{2014} Asset Editor")
                .with_inner_size(PhysicalSize::new(1280, 800));
            let window = Arc::new(event_loop.create_window(attrs).unwrap());
            self.initialize(window);
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(key) = event.physical_key {
                    if key == KeyCode::Escape && event.state == ElementState::Pressed {
                        event_loop.exit();
                        return;
                    }
                    self.handle_key(key, event.state == ElementState::Pressed);
                }
            }

            // -- Mouse input: forward to orbit camera for stages 2-5 --
            WindowEvent::MouseInput { button, state, .. } => {
                if self.editor.uses_orbit_camera() {
                    let orbit_btn = match button {
                        MouseButton::Middle => Some(OrbitMouseButton::Middle),
                        MouseButton::Right => Some(OrbitMouseButton::Right),
                        _ => None,
                    };
                    if let Some(btn) = orbit_btn {
                        self.editor
                            .camera
                            .handle_mouse_drag(btn, state == ElementState::Pressed);
                    }
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                if self.editor.uses_orbit_camera() {
                    self.editor
                        .camera
                        .handle_mouse_move(position.x as f32, position.y as f32);
                }
            }

            WindowEvent::MouseWheel { delta, .. } => {
                if self.editor.uses_orbit_camera() {
                    let scroll = match delta {
                        MouseScrollDelta::LineDelta(_, y) => y,
                        MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / 100.0,
                    };
                    self.editor.camera.handle_scroll(scroll);
                }
            }

            WindowEvent::Resized(new_size) => {
                if let Some(ref mut gpu) = self.gpu {
                    gpu.resize(new_size);
                }
                // Update orbit camera aspect ratio
                self.editor.camera.resize(new_size.width, new_size.height);
            }

            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let delta_time = now.duration_since(self.last_frame).as_secs_f32();
                self.last_frame = now;

                // FPS tracking
                self.frame_count += 1;
                if now.duration_since(self.last_fps_update).as_secs_f32() >= 1.0 {
                    self.fps = self.frame_count as f32
                        / now.duration_since(self.last_fps_update).as_secs_f32();
                    self.frame_count = 0;
                    self.last_fps_update = now;
                }

                // Update editor state
                self.editor.update(delta_time);

                // Render
                self.render();
            }

            _ => {}
        }
    }

    fn about_to_wait(&mut self, _: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

// ============================================================================
// MAIN
// ============================================================================

fn main() {
    // Suppress unused field warning for start_time -- will be used later
    let _ = std::mem::offset_of!(BattleEditorApp, start_time);

    println!("===========================================");
    println!("   Battle T\u{00f6}k \u{2014} Asset Editor");
    println!("===========================================");
    println!();
    println!("Controls:");
    println!("  1-5: Switch editor stage");
    println!("  Middle mouse drag: Orbit (stages 2-5)");
    println!("  Right mouse drag: Pan (stages 2-5)");
    println!("  Scroll wheel: Zoom (stages 2-5)");
    println!("  Ctrl+Z: Undo");
    println!("  Ctrl+Y: Redo");
    println!("  ESC: Exit");
    println!();

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = BattleEditorApp::new();
    event_loop.run_app(&mut app).unwrap();
}
