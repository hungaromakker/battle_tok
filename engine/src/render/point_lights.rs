//! Point Light System for Torch Rendering
//!
//! This module provides GPU-compatible point light management for rendering
//! up to 16 dynamic torch lights with flickering effects.

/// Maximum number of point lights supported (16 torches for two castles)
pub const MAX_POINT_LIGHTS: usize = 16;

/// Total buffer size in bytes for point light data (16 * 32 = 512 bytes)
pub const POINT_LIGHT_BUFFER_SIZE: usize = MAX_POINT_LIGHTS * std::mem::size_of::<PointLight>();

/// Size of the light count uniform buffer (u32 padded to 16 bytes for alignment)
pub const LIGHT_COUNT_BUFFER_SIZE: usize = 16;

/// GPU-compatible point light data structure.
///
/// Layout (32 bytes total, properly aligned for GPU compatibility):
/// - position:  vec3<f32> (12 bytes) - World position of the light
/// - radius:    f32 (4 bytes) - Light influence radius
/// - color:     vec3<f32> (12 bytes) - RGB color of the light
/// - intensity: f32 (4 bytes) - Current intensity (modified by flicker)
///
/// Total: 12 + 4 + 12 + 4 = 32 bytes
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PointLight {
    /// World position (x, y, z) - 12 bytes
    pub position: [f32; 3],
    /// Light influence radius - 4 bytes
    pub radius: f32,
    /// RGB color components - 12 bytes
    pub color: [f32; 3],
    /// Current intensity (modified by flicker each frame) - 4 bytes
    pub intensity: f32,
}

// Compile-time assertion to verify struct size is exactly 32 bytes
const _: () = {
    assert!(
        std::mem::size_of::<PointLight>() == 32,
        "PointLight must be exactly 32 bytes for GPU compatibility"
    );
};

impl Default for PointLight {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0, 0.0],
            radius: 5.0,
            color: [1.0, 0.7, 0.3], // Warm torch color
            intensity: 1.0,
        }
    }
}

impl PointLight {
    /// Create a new point light with the given parameters.
    pub fn new(position: [f32; 3], color: [f32; 3], radius: f32, intensity: f32) -> Self {
        Self {
            position,
            radius,
            color,
            intensity,
        }
    }

    /// Create a torch light at a position with default warm color.
    pub fn torch(position: [f32; 3]) -> Self {
        Self {
            position,
            radius: 8.0,
            color: [1.0, 0.6, 0.2], // Warm orange torch color
            intensity: 1.0,
        }
    }

    /// Set position and return self for chaining.
    pub fn with_position(mut self, position: [f32; 3]) -> Self {
        self.position = position;
        self
    }

    /// Set color and return self for chaining.
    pub fn with_color(mut self, color: [f32; 3]) -> Self {
        self.color = color;
        self
    }

    /// Set radius and return self for chaining.
    pub fn with_radius(mut self, radius: f32) -> Self {
        self.radius = radius;
        self
    }

    /// Set intensity and return self for chaining.
    pub fn with_intensity(mut self, intensity: f32) -> Self {
        self.intensity = intensity;
        self
    }
}

/// Internal light data for tracking base intensity (used for flickering).
#[derive(Clone, Debug)]
struct LightData {
    /// The GPU-visible light data
    light: PointLight,
    /// Base intensity before flicker is applied
    base_intensity: f32,
}

/// Point Light Manager for handling up to 16 dynamic torch lights.
///
/// This manager:
/// - Stores light positions, colors, and radii in a GPU buffer
/// - Updates each frame to apply sin-based flickering
/// - Exposes a bind group that material shaders can sample
pub struct PointLightManager {
    /// Internal light data with base intensities for flickering
    lights: Vec<LightData>,
    /// GPU buffer for point light data (storage buffer)
    buffer: wgpu::Buffer,
    /// GPU buffer for light count uniform
    light_count_buffer: wgpu::Buffer,
    /// Bind group layout for shader binding
    bind_group_layout: wgpu::BindGroupLayout,
    /// Bind group for shader access
    bind_group: wgpu::BindGroup,
}

impl PointLightManager {
    /// Create a new PointLightManager with empty lights.
    ///
    /// # Arguments
    /// * `device` - The wgpu device to create GPU resources on
    pub fn new(device: &wgpu::Device) -> Self {
        // Create buffer for up to 16 lights (512 bytes)
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Point Lights Buffer"),
            size: POINT_LIGHT_BUFFER_SIZE as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create buffer for light count uniform (padded to 16 bytes)
        let light_count_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Point Light Count Buffer"),
            size: LIGHT_COUNT_BUFFER_SIZE as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Point Lights Bind Group Layout"),
            entries: &[
                // Binding 0: Point lights storage buffer (read-only)
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Binding 1: Light count uniform
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // Create bind group
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Point Lights Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: light_count_buffer.as_entire_binding(),
                },
            ],
        });

        Self {
            lights: Vec::with_capacity(MAX_POINT_LIGHTS),
            buffer,
            light_count_buffer,
            bind_group_layout,
            bind_group,
        }
    }

    /// Add a torch light at the given position with specified color and radius.
    ///
    /// # Arguments
    /// * `position` - World position of the torch
    /// * `color` - RGB color of the light
    /// * `radius` - Light influence radius
    ///
    /// # Returns
    /// The index of the added torch, or None if max lights reached.
    pub fn add_torch(&mut self, position: [f32; 3], color: [f32; 3], radius: f32) -> Option<usize> {
        if self.lights.len() >= MAX_POINT_LIGHTS {
            return None;
        }

        let light = PointLight::new(position, color, radius, 1.0);
        let index = self.lights.len();
        self.lights.push(LightData {
            light,
            base_intensity: 1.0,
        });

        Some(index)
    }

    /// Add a torch light with custom base intensity.
    ///
    /// # Arguments
    /// * `position` - World position of the torch
    /// * `color` - RGB color of the light
    /// * `radius` - Light influence radius
    /// * `intensity` - Base intensity (before flicker)
    ///
    /// # Returns
    /// The index of the added torch, or None if max lights reached.
    pub fn add_torch_with_intensity(
        &mut self,
        position: [f32; 3],
        color: [f32; 3],
        radius: f32,
        intensity: f32,
    ) -> Option<usize> {
        if self.lights.len() >= MAX_POINT_LIGHTS {
            return None;
        }

        let light = PointLight::new(position, color, radius, intensity);
        let index = self.lights.len();
        self.lights.push(LightData {
            light,
            base_intensity: intensity,
        });

        Some(index)
    }

    /// Remove a torch at the given index.
    ///
    /// # Arguments
    /// * `index` - The index of the torch to remove
    ///
    /// # Returns
    /// true if the torch was removed, false if the index was invalid.
    pub fn remove_torch(&mut self, index: usize) -> bool {
        if index < self.lights.len() {
            self.lights.remove(index);
            true
        } else {
            false
        }
    }

    /// Clear all torches.
    pub fn clear(&mut self) {
        self.lights.clear();
    }

    /// Get the current number of active lights.
    pub fn light_count(&self) -> usize {
        self.lights.len()
    }

    /// Check if max lights have been reached.
    pub fn is_full(&self) -> bool {
        self.lights.len() >= MAX_POINT_LIGHTS
    }

    /// Update light intensities with flicker effect and write to GPU buffer.
    ///
    /// This should be called each frame to update the flickering.
    ///
    /// # Arguments
    /// * `queue` - The wgpu queue for submitting buffer writes
    /// * `time` - Current time in seconds (used for flicker animation)
    pub fn update(&mut self, queue: &wgpu::Queue, time: f32) {
        // Apply flicker to each light
        for (i, light_data) in self.lights.iter_mut().enumerate() {
            // Each torch has unique flicker phase based on index
            let phase = i as f32 * 1.7;
            // Flicker oscillates between 0.85 and 1.0 of base intensity
            let flicker = 0.85 + 0.15 * (time * 12.0 + phase).sin();
            light_data.light.intensity = light_data.base_intensity * flicker;
        }

        // Write lights to GPU buffer
        if !self.lights.is_empty() {
            let gpu_lights: Vec<PointLight> = self.lights.iter().map(|ld| ld.light).collect();
            queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&gpu_lights));
        }

        // Write light count to uniform buffer (padded to 16 bytes)
        let count_data: [u32; 4] = [self.lights.len() as u32, 0, 0, 0];
        queue.write_buffer(
            &self.light_count_buffer,
            0,
            bytemuck::cast_slice(&count_data),
        );
    }

    /// Get the bind group for shader access.
    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    /// Get the bind group layout for pipeline creation.
    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    /// Get a reference to a light at the given index.
    pub fn get_light(&self, index: usize) -> Option<&PointLight> {
        self.lights.get(index).map(|ld| &ld.light)
    }

    /// Update the position of a torch at the given index.
    pub fn set_torch_position(&mut self, index: usize, position: [f32; 3]) -> bool {
        if let Some(light_data) = self.lights.get_mut(index) {
            light_data.light.position = position;
            true
        } else {
            false
        }
    }

    /// Update the color of a torch at the given index.
    pub fn set_torch_color(&mut self, index: usize, color: [f32; 3]) -> bool {
        if let Some(light_data) = self.lights.get_mut(index) {
            light_data.light.color = color;
            true
        } else {
            false
        }
    }

    /// Update the base intensity of a torch at the given index.
    pub fn set_torch_intensity(&mut self, index: usize, intensity: f32) -> bool {
        if let Some(light_data) = self.lights.get_mut(index) {
            light_data.base_intensity = intensity;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point_light_size() {
        assert_eq!(std::mem::size_of::<PointLight>(), 32);
    }

    #[test]
    fn test_point_light_buffer_size() {
        assert_eq!(POINT_LIGHT_BUFFER_SIZE, 512); // 16 * 32 = 512 bytes
    }

    #[test]
    fn test_max_point_lights() {
        assert_eq!(MAX_POINT_LIGHTS, 16);
    }

    #[test]
    fn test_default_point_light() {
        let light = PointLight::default();
        assert_eq!(light.position, [0.0, 0.0, 0.0]);
        assert_eq!(light.radius, 5.0);
        assert_eq!(light.color, [1.0, 0.7, 0.3]); // Warm torch color
        assert_eq!(light.intensity, 1.0);
    }

    #[test]
    fn test_torch_light() {
        let light = PointLight::torch([1.0, 2.0, 3.0]);
        assert_eq!(light.position, [1.0, 2.0, 3.0]);
        assert_eq!(light.radius, 8.0);
        assert_eq!(light.color, [1.0, 0.6, 0.2]); // Warm orange
        assert_eq!(light.intensity, 1.0);
    }

    #[test]
    fn test_builder_pattern() {
        let light = PointLight::default()
            .with_position([5.0, 10.0, 15.0])
            .with_color([1.0, 0.5, 0.0])
            .with_radius(12.0)
            .with_intensity(0.8);

        assert_eq!(light.position, [5.0, 10.0, 15.0]);
        assert_eq!(light.color, [1.0, 0.5, 0.0]);
        assert_eq!(light.radius, 12.0);
        assert_eq!(light.intensity, 0.8);
    }
}
