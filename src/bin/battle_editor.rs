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
//! - D: Freehand draw tool (stage 1)
//! - L: Line tool (stage 1)
//! - A: Arc tool - 3-click arc creation (stage 1)
//! - E: Eraser tool - circle cursor point removal (stage 1)
//! - M: Toggle mirror symmetry at x=0 (stage 1)
//! - G: Toggle grid (stage 1)
//! - [ / ]: Decrease/increase eraser radius (stage 1)
//! - P: Cycle pump profile: Elliptical / Flat / Pointed (stage 2)
//! - Up/Down arrows: Adjust inflation 0.0-1.0 (stage 2)
//! - Left/Right arrows: Adjust thickness 0.1-5.0 (stage 2)
//! - Left mouse: Draw (stage 1)
//! - Middle/right mouse drag: Pan canvas (stage 1)
//! - Scroll wheel: Zoom canvas (stage 1)
//! - Middle mouse drag: Orbit camera (stages 2-5)
//! - Right mouse drag: Pan camera (stages 2-5)
//! - Scroll wheel: Zoom camera (stages 2-5)
//! - Ctrl+I: Load reference image for tracing (stage 1)
//! - Ctrl+H: Toggle reference image visibility (stage 1)
//! - Ctrl+Scroll: Adjust reference image scale (stage 1)
//! - Ctrl+Z: Undo
//! - Ctrl+Y: Redo
//! - ESC: Exit

use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowAttributes, WindowId};

use wgpu::util::DeviceExt;

use battle_tok_engine::game::asset_editor::image_trace::{CanvasViewport, ImageTrace};
use battle_tok_engine::game::asset_editor::orbit_camera::OrbitMouseButton;
use battle_tok_engine::game::asset_editor::{AssetEditor, EditorStage};
use battle_tok_engine::game::types::Vertex;

// ============================================================================
// GPU RESOURCES (minimal for editor)
// ============================================================================

/// Minimal GPU state for the editor window.
struct EditorGpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    /// Render pipeline for 2D canvas overlays (vertex-colored triangles).
    overlay_pipeline: wgpu::RenderPipeline,
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
    /// Current mouse position in screen pixels
    current_mouse: (f32, f32),

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
            current_mouse: (0.0, 0.0),
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

        // Create overlay shader and pipeline for canvas rendering
        let overlay_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Editor Overlay Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../../shaders/editor_overlay.wgsl").into(),
            ),
        });

        let overlay_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Editor Overlay Pipeline Layout"),
                bind_group_layouts: &[],
                push_constant_ranges: &[],
            });

        let overlay_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Editor Overlay Pipeline"),
            layout: Some(&overlay_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &overlay_shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 0,
                            shader_location: 0, // position
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 12,
                            shader_location: 1, // normal
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: 24,
                            shader_location: 2, // color
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &overlay_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None, // No culling for 2D overlays
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        self.gpu = Some(EditorGpu {
            device,
            queue,
            surface,
            surface_config,
            overlay_pipeline,
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
            if self.editor.stage == EditorStage::Draw2D {
                // In Draw2D stage, undo removes the last drawn outline
                self.editor.canvas.undo();
            } else if let Some(cmd) = self.editor.undo_stack.undo() {
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

        // Ctrl+I => Load reference image for tracing (Draw2D stage)
        if key == KeyCode::KeyI && self.ctrl_held {
            if self.editor.stage == EditorStage::Draw2D {
                self.load_trace_image();
            }
            return;
        }

        // Ctrl+H => Toggle reference image visibility (Draw2D stage)
        if key == KeyCode::KeyH && self.ctrl_held {
            if self.editor.stage == EditorStage::Draw2D {
                if let Some(ref mut trace) = self.editor.canvas.image_trace {
                    trace.toggle_visible();
                } else {
                    println!("ImageTrace: no image loaded (use Ctrl+I first)");
                }
            }
            return;
        }

        // F10: toggle asset library panel
        if key == KeyCode::F10 {
            self.editor.library.visible = !self.editor.library.visible;
            println!(
                "Library panel: {}",
                if self.editor.library.visible {
                    "visible"
                } else {
                    "hidden"
                }
            );
            return;
        }

        // If library is visible, route character keys to search
        if self.editor.library.visible {
            match key {
                KeyCode::Backspace => {
                    self.editor.library.handle_backspace();
                    return;
                }
                k => {
                    let c = match k {
                        KeyCode::KeyA => Some('a'),
                        KeyCode::KeyB => Some('b'),
                        KeyCode::KeyC => Some('c'),
                        KeyCode::KeyD => Some('d'),
                        KeyCode::KeyE => Some('e'),
                        KeyCode::KeyF => Some('f'),
                        KeyCode::KeyG => Some('g'),
                        KeyCode::KeyH => Some('h'),
                        KeyCode::KeyI => Some('i'),
                        KeyCode::KeyJ => Some('j'),
                        KeyCode::KeyK => Some('k'),
                        KeyCode::KeyL => Some('l'),
                        KeyCode::KeyM => Some('m'),
                        KeyCode::KeyN => Some('n'),
                        KeyCode::KeyO => Some('o'),
                        KeyCode::KeyP => Some('p'),
                        KeyCode::KeyQ => Some('q'),
                        KeyCode::KeyR => Some('r'),
                        KeyCode::KeyS => Some('s'),
                        KeyCode::KeyT => Some('t'),
                        KeyCode::KeyU => Some('u'),
                        KeyCode::KeyV => Some('v'),
                        KeyCode::KeyW => Some('w'),
                        KeyCode::KeyX => Some('x'),
                        KeyCode::KeyY => Some('y'),
                        KeyCode::KeyZ => Some('z'),
                        KeyCode::Space => Some(' '),
                        KeyCode::Minus => Some('-'),
                        KeyCode::Digit0 => Some('0'),
                        KeyCode::Digit1 => Some('1'),
                        KeyCode::Digit2 => Some('2'),
                        KeyCode::Digit3 => Some('3'),
                        KeyCode::Digit4 => Some('4'),
                        KeyCode::Digit5 => Some('5'),
                        KeyCode::Digit6 => Some('6'),
                        KeyCode::Digit7 => Some('7'),
                        KeyCode::Digit8 => Some('8'),
                        KeyCode::Digit9 => Some('9'),
                        _ => None,
                    };
                    if let Some(ch) = c {
                        self.editor.library.handle_char(ch);
                        return;
                    }
                }
            }
        }

        // Extrude-stage keys (Stage 2)
        if self.editor.stage == EditorStage::Extrude {
            match key {
                KeyCode::KeyP => {
                    self.editor.cycle_pump_profile();
                    return;
                }
                KeyCode::ArrowUp => {
                    self.editor.adjust_inflation(0.05);
                    return;
                }
                KeyCode::ArrowDown => {
                    self.editor.adjust_inflation(-0.05);
                    return;
                }
                KeyCode::ArrowRight => {
                    self.editor.adjust_thickness(0.1);
                    return;
                }
                KeyCode::ArrowLeft => {
                    self.editor.adjust_thickness(-0.1);
                    return;
                }
                _ => {}
            }
        }

        // Canvas-specific keys when in Draw2D stage
        if self.editor.stage == EditorStage::Draw2D {
            match key {
                KeyCode::KeyD => {
                    self.editor.canvas.select_freehand();
                    return;
                }
                KeyCode::KeyL => {
                    self.editor.canvas.select_line();
                    return;
                }
                KeyCode::KeyA => {
                    self.editor.canvas.select_arc();
                    return;
                }
                KeyCode::KeyE => {
                    self.editor.canvas.select_eraser();
                    return;
                }
                KeyCode::KeyM => {
                    self.editor.canvas.toggle_mirror();
                    return;
                }
                KeyCode::KeyG => {
                    self.editor.canvas.toggle_grid();
                    return;
                }
                KeyCode::BracketLeft => {
                    self.editor.canvas.decrease_eraser_radius();
                    return;
                }
                KeyCode::BracketRight => {
                    self.editor.canvas.increase_eraser_radius();
                    return;
                }
                _ => {}
            }
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

    /// Load a reference image for tracing on the 2D canvas.
    ///
    /// Uses the `TRACE_IMAGE` environment variable if set, otherwise
    /// prints instructions to stdout.
    fn load_trace_image(&mut self) {
        let gpu = match &self.gpu {
            Some(gpu) => gpu,
            None => {
                eprintln!("ImageTrace: GPU not initialized yet");
                return;
            }
        };

        let path_str = std::env::var("TRACE_IMAGE").unwrap_or_default();
        if path_str.is_empty() {
            println!("ImageTrace: set TRACE_IMAGE env var to load a reference image");
            println!("  Example: TRACE_IMAGE=reference.png cargo run --bin battle_editor");
            return;
        }

        let path = Path::new(&path_str);
        if !path.exists() {
            eprintln!("ImageTrace: file not found: {}", path_str);
            return;
        }

        match ImageTrace::load(path, &gpu.device, &gpu.queue, gpu.surface_config.format) {
            Ok(trace) => {
                println!(
                    "ImageTrace: loaded {}x{} from {}",
                    trace.image_width, trace.image_height, path_str
                );
                self.editor.canvas.image_trace = Some(trace);
            }
            Err(e) => {
                eprintln!("ImageTrace: {}", e);
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

        // Render reference image trace (if loaded and in Draw2D stage)
        if self.editor.stage == EditorStage::Draw2D {
            if let Some(ref trace) = self.editor.canvas.image_trace {
                let canvas_vp = CanvasViewport {
                    zoom: self.editor.canvas.zoom,
                    pan: self.editor.canvas.pan,
                    width: gpu.surface_config.width as f32,
                    height: gpu.surface_config.height as f32,
                };
                trace.render(&mut encoder, &view, &gpu.queue, &canvas_vp);
            }
        }

        // Render canvas overlay (grid, outlines, tool previews)
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        self.editor.render_stage(&mut vertices, &mut indices);

        // Render library panel overlay
        let screen_w = gpu.surface_config.width as f32;
        let screen_h = gpu.surface_config.height as f32;
        let (lib_verts, lib_idxs) = self.editor.library.generate_panel(screen_w, screen_h);
        if !lib_verts.is_empty() {
            let base = vertices.len() as u32;
            vertices.extend_from_slice(&lib_verts);
            indices.extend(lib_idxs.iter().map(|i| i + base));
        }

        if !vertices.is_empty() && !indices.is_empty() {
            let vertex_buffer = gpu
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Canvas Vertex Buffer"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });
            let index_buffer = gpu
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Canvas Index Buffer"),
                    contents: bytemuck::cast_slice(&indices),
                    usage: wgpu::BufferUsages::INDEX,
                });

            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Editor Canvas Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        depth_slice: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load, // Preserve clear + image trace
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

                pass.set_pipeline(&gpu.overlay_pipeline);
                pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..indices.len() as u32, 0, 0..1);
            }
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

            // -- Mouse input: library panel gets first dibs, then canvas/camera --
            WindowEvent::MouseInput { button, state, .. } => {
                let pressed = state == ElementState::Pressed;
                let (mx, my) = self.current_mouse;

                // Library panel click handling (left click only)
                if button == MouseButton::Left && pressed {
                    let sw = self
                        .gpu
                        .as_ref()
                        .map(|g| g.surface_config.width as f32)
                        .unwrap_or(1280.0);
                    let sh = self
                        .gpu
                        .as_ref()
                        .map(|g| g.surface_config.height as f32)
                        .unwrap_or(800.0);
                    if self.editor.library.handle_click(mx, my, sw, sh).is_some() {
                        return; // Consumed by library
                    }
                }

                if self.editor.stage == EditorStage::Draw2D {
                    // Draw2D stage: forward to Canvas2D
                    match button {
                        MouseButton::Left => {
                            if pressed {
                                self.editor.canvas.on_left_press(mx, my);
                            } else {
                                self.editor.canvas.on_left_release();
                            }
                        }
                        MouseButton::Middle => {
                            if pressed {
                                self.editor.canvas.on_middle_press(mx, my);
                            } else {
                                self.editor.canvas.on_middle_release();
                            }
                        }
                        MouseButton::Right => {
                            if pressed {
                                self.editor.canvas.on_right_press(mx, my);
                            } else {
                                self.editor.canvas.on_right_release();
                            }
                        }
                        _ => {}
                    }
                } else {
                    // Stages 2-5: forward to orbit camera
                    let orbit_btn = match button {
                        MouseButton::Middle => Some(OrbitMouseButton::Middle),
                        MouseButton::Right => Some(OrbitMouseButton::Right),
                        _ => None,
                    };
                    if let Some(btn) = orbit_btn {
                        self.editor.camera.handle_mouse_drag(btn, pressed);
                    }
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                let (mx, my) = (position.x as f32, position.y as f32);
                self.current_mouse = (mx, my);

                if self.editor.stage == EditorStage::Draw2D {
                    self.editor.canvas.on_mouse_move(mx, my);
                } else {
                    self.editor.camera.handle_mouse_move(mx, my);
                }
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let scroll = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / 100.0,
                };

                // Library panel scroll
                if self.editor.library.visible {
                    let sw = self
                        .gpu
                        .as_ref()
                        .map(|g| g.surface_config.width as f32)
                        .unwrap_or(1280.0);
                    let (mx, _my) = self.current_mouse;
                    if self.editor.library.contains_point(mx, sw) {
                        self.editor.library.handle_scroll(scroll);
                        return;
                    }
                }

                if self.editor.stage == EditorStage::Draw2D {
                    if self.ctrl_held {
                        // Ctrl+Scroll: adjust reference image scale
                        if let Some(ref mut trace) = self.editor.canvas.image_trace {
                            let factor = if scroll > 0.0 { 1.1 } else { 1.0 / 1.1 };
                            trace.adjust_scale(factor);
                        }
                    } else {
                        self.editor.canvas.on_scroll(scroll);
                    }
                } else {
                    self.editor.camera.handle_scroll(scroll);
                }
            }

            WindowEvent::Resized(new_size) => {
                if let Some(ref mut gpu) = self.gpu {
                    gpu.resize(new_size);
                }
                // Update orbit camera aspect ratio
                self.editor.camera.resize(new_size.width, new_size.height);
                // Update canvas viewport dimensions
                self.editor
                    .canvas
                    .set_viewport_size(new_size.width as f32, new_size.height as f32);
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
    println!("  D: Freehand draw tool (stage 1)");
    println!("  L: Line tool (stage 1)");
    println!("  A: Arc tool (stage 1)");
    println!("  E: Eraser tool (stage 1)");
    println!("  M: Toggle mirror symmetry (stage 1)");
    println!("  G: Toggle grid (stage 1)");
    println!("  [/]: Adjust eraser radius (stage 1)");
    println!("  P: Cycle pump profile (stage 2)");
    println!("  Up/Down: Adjust inflation (stage 2)");
    println!("  Left/Right: Adjust thickness (stage 2)");
    println!("  Left mouse: Draw (stage 1)");
    println!("  Middle/right drag: Pan canvas (stage 1)");
    println!("  Scroll wheel: Zoom canvas (stage 1)");
    println!("  Middle mouse drag: Orbit (stages 2-5)");
    println!("  Right mouse drag: Pan (stages 2-5)");
    println!("  Scroll wheel: Zoom (stages 2-5)");
    println!("  Ctrl+I: Load reference image (stage 1)");
    println!("  Ctrl+H: Toggle reference image (stage 1)");
    println!("  Ctrl+Scroll: Adjust image scale (stage 1)");
    println!("  F10: Toggle asset library panel");
    println!("  Ctrl+Z: Undo");
    println!("  Ctrl+Y: Redo");
    println!("  ESC: Exit");
    println!();

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = BattleEditorApp::new();
    event_loop.run_app(&mut app).unwrap();
}
