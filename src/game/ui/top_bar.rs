//! Top Bar UI - Glassmorphism Style
//!
//! Semi-transparent frosted glass effect top bar showing:
//! - Day number and time with elegant styling
//! - Time of day phase with glow effect
//! - Resources with smooth icon designs
//! - Population counter

use crate::game::types::{Mesh, Vertex};
use crate::game::economy::{Resources, ResourceType, DayCycle, TimeOfDay};
use crate::game::population::Population;
use super::text::{add_quad, draw_text};

/// Height of the top bar in pixels
pub const TOP_BAR_HEIGHT: f32 = 56.0;

/// Padding from edges
const PADDING: f32 = 16.0;

/// Icon size for resources
const ICON_SIZE: f32 = 24.0;

/// Spacing between resource groups
const RESOURCE_SPACING: f32 = 24.0;

/// Semi-transparent top bar UI with glassmorphism effect
pub struct TopBar {
    /// Is the top bar visible?
    pub visible: bool,
}

impl Default for TopBar {
    fn default() -> Self {
        Self { visible: true }
    }
}

impl TopBar {
    pub fn new() -> Self {
        Self::default()
    }

    /// Generate UI mesh for the top bar with glassmorphism effect
    pub fn generate_ui_mesh(
        &self,
        screen_width: f32,
        screen_height: f32,
        resources: &Resources,
        day_cycle: &DayCycle,
        population: &Population,
    ) -> Mesh {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        if !self.visible {
            return Mesh { vertices, indices };
        }

        // Helper to convert screen coords to NDC
        let to_ndc = |x: f32, y: f32| -> [f32; 3] {
            [
                (x / screen_width) * 2.0 - 1.0,
                1.0 - (y / screen_height) * 2.0,
                0.0,
            ]
        };

        // === GLASSMORPHISM BACKGROUND ===
        // Layer 1: Dark base with more transparency
        let base_color = [0.02, 0.02, 0.05, 0.65];
        add_quad(
            &mut vertices,
            &mut indices,
            to_ndc(0.0, 0.0),
            to_ndc(screen_width, 0.0),
            to_ndc(screen_width, TOP_BAR_HEIGHT),
            to_ndc(0.0, TOP_BAR_HEIGHT),
            base_color,
        );

        // Layer 2: Subtle gradient overlay from top
        let gradient_top = [0.15, 0.15, 0.25, 0.15];
        add_quad(
            &mut vertices,
            &mut indices,
            to_ndc(0.0, 0.0),
            to_ndc(screen_width, 0.0),
            to_ndc(screen_width, TOP_BAR_HEIGHT * 0.3),
            to_ndc(0.0, TOP_BAR_HEIGHT * 0.3),
            gradient_top,
        );

        // Layer 3: Frosted blur effect (subtle light strip)
        let frost_color = [0.4, 0.4, 0.5, 0.08];
        add_quad(
            &mut vertices,
            &mut indices,
            to_ndc(0.0, TOP_BAR_HEIGHT * 0.1),
            to_ndc(screen_width, TOP_BAR_HEIGHT * 0.1),
            to_ndc(screen_width, TOP_BAR_HEIGHT * 0.15),
            to_ndc(0.0, TOP_BAR_HEIGHT * 0.15),
            frost_color,
        );

        // Bottom border - glowing edge
        let border_color = [0.3, 0.35, 0.5, 0.5];
        add_quad(
            &mut vertices,
            &mut indices,
            to_ndc(0.0, TOP_BAR_HEIGHT - 2.0),
            to_ndc(screen_width, TOP_BAR_HEIGHT - 2.0),
            to_ndc(screen_width, TOP_BAR_HEIGHT),
            to_ndc(0.0, TOP_BAR_HEIGHT),
            border_color,
        );

        // Inner glow line
        let glow_color = [0.5, 0.55, 0.7, 0.2];
        add_quad(
            &mut vertices,
            &mut indices,
            to_ndc(0.0, TOP_BAR_HEIGHT - 3.0),
            to_ndc(screen_width, TOP_BAR_HEIGHT - 3.0),
            to_ndc(screen_width, TOP_BAR_HEIGHT - 2.0),
            to_ndc(0.0, TOP_BAR_HEIGHT - 2.0),
            glow_color,
        );

        let y_center = TOP_BAR_HEIGHT / 2.0;
        let mut x_offset = PADDING;

        // === LEFT SIDE: Day & Time Panel ===

        // Day panel background (pill shape approximation)
        let day_panel_width = 180.0;
        let panel_height = 36.0;
        let panel_y = (TOP_BAR_HEIGHT - panel_height) / 2.0;

        // Panel background
        Self::draw_rounded_panel(
            &mut vertices,
            &mut indices,
            x_offset,
            panel_y,
            day_panel_width,
            panel_height,
            [0.1, 0.12, 0.18, 0.6],
            screen_width,
            screen_height,
        );

        // Day number with subtle glow
        let day_text = format!("DAY {}", day_cycle.day());
        draw_text(
            &mut vertices,
            &mut indices,
            &day_text,
            x_offset + 12.0,
            y_center - 8.0,
            2.5,
            [1.0, 1.0, 1.0, 0.95],
            screen_width,
            screen_height,
        );

        // Time string (HH:MM)
        let time_text = day_cycle.time_string();
        draw_text(
            &mut vertices,
            &mut indices,
            &time_text,
            x_offset + 80.0,
            y_center - 8.0,
            2.5,
            [0.7, 0.75, 0.85, 0.9],
            screen_width,
            screen_height,
        );

        // Phase indicator with color glow
        let (phase_text, phase_color, phase_glow) = match day_cycle.time_of_day() {
            TimeOfDay::Dawn => ("DAWN", [1.0, 0.75, 0.45, 1.0], [1.0, 0.6, 0.2, 0.3]),
            TimeOfDay::Day => ("DAY", [1.0, 0.95, 0.7, 1.0], [1.0, 0.9, 0.4, 0.3]),
            TimeOfDay::Dusk => ("DUSK", [1.0, 0.55, 0.35, 1.0], [1.0, 0.4, 0.2, 0.3]),
            TimeOfDay::Night => ("NIGHT", [0.55, 0.6, 0.9, 1.0], [0.3, 0.35, 0.7, 0.3]),
        };

        x_offset = PADDING + day_panel_width + 16.0;

        // Phase glow background
        let phase_width = 70.0;
        Self::draw_rounded_panel(
            &mut vertices,
            &mut indices,
            x_offset,
            panel_y,
            phase_width,
            panel_height,
            phase_glow,
            screen_width,
            screen_height,
        );

        draw_text(
            &mut vertices,
            &mut indices,
            phase_text,
            x_offset + 10.0,
            y_center - 8.0,
            2.5,
            phase_color,
            screen_width,
            screen_height,
        );

        // Day progress bar
        x_offset += phase_width + 16.0;
        let bar_width = 100.0;
        let bar_height = 6.0;
        let bar_y = y_center - bar_height / 2.0;

        // Progress bar background
        Self::draw_rounded_panel(
            &mut vertices,
            &mut indices,
            x_offset,
            bar_y,
            bar_width,
            bar_height,
            [0.15, 0.15, 0.2, 0.6],
            screen_width,
            screen_height,
        );

        // Progress bar fill
        let fill_width = bar_width * day_cycle.time();
        if fill_width > 2.0 {
            let fill_color = match day_cycle.time_of_day() {
                TimeOfDay::Dawn => [1.0, 0.6, 0.2, 0.9],
                TimeOfDay::Day => [1.0, 0.9, 0.4, 0.9],
                TimeOfDay::Dusk => [1.0, 0.45, 0.2, 0.9],
                TimeOfDay::Night => [0.35, 0.4, 0.7, 0.9],
            };
            Self::draw_rounded_panel(
                &mut vertices,
                &mut indices,
                x_offset + 1.0,
                bar_y + 1.0,
                fill_width - 2.0,
                bar_height - 2.0,
                fill_color,
                screen_width,
                screen_height,
            );
        }

        // === RIGHT SIDE: Resources & Population ===
        let mut rx_offset = screen_width - PADDING;

        // Population panel
        let pop_text = format!("{}", population.total());
        let pop_width = 70.0;
        rx_offset -= pop_width;

        Self::draw_rounded_panel(
            &mut vertices,
            &mut indices,
            rx_offset,
            panel_y,
            pop_width,
            panel_height,
            [0.1, 0.12, 0.18, 0.6],
            screen_width,
            screen_height,
        );

        // Population icon (person silhouette - improved)
        Self::draw_person_icon(
            &mut vertices,
            &mut indices,
            rx_offset + 8.0,
            y_center - ICON_SIZE / 2.0,
            ICON_SIZE,
            [0.85, 0.9, 1.0, 0.9],
            screen_width,
            screen_height,
        );

        // Population number
        draw_text(
            &mut vertices,
            &mut indices,
            &pop_text,
            rx_offset + 36.0,
            y_center - 8.0,
            2.5,
            [0.9, 0.95, 1.0, 1.0],
            screen_width,
            screen_height,
        );

        rx_offset -= 20.0;

        // Resources panel
        let resource_order = [
            ResourceType::Iron,
            ResourceType::Food,
            ResourceType::Wood,
            ResourceType::Stone,
            ResourceType::Gold,
        ];

        for res_type in resource_order {
            let amount = resources.get(res_type);
            let net = resources.get_net(res_type);

            let amount_text = format!("{}", amount);
            let item_width = 65.0 + (amount_text.len() as f32 - 2.0).max(0.0) * 10.0;
            rx_offset -= item_width;

            // Resource panel background
            let res_color = res_type.color();
            let panel_bg = [
                res_color[0] as f32 / 255.0 * 0.15,
                res_color[1] as f32 / 255.0 * 0.15,
                res_color[2] as f32 / 255.0 * 0.15,
                0.4,
            ];
            Self::draw_rounded_panel(
                &mut vertices,
                &mut indices,
                rx_offset,
                panel_y,
                item_width - 4.0,
                panel_height,
                panel_bg,
                screen_width,
                screen_height,
            );

            // Resource icon (improved)
            Self::draw_resource_icon(
                &mut vertices,
                &mut indices,
                res_type,
                rx_offset + 6.0,
                y_center - ICON_SIZE / 2.0,
                ICON_SIZE,
                screen_width,
                screen_height,
            );

            // Resource amount
            let text_color = [
                res_color[0] as f32 / 255.0 * 0.8 + 0.2,
                res_color[1] as f32 / 255.0 * 0.8 + 0.2,
                res_color[2] as f32 / 255.0 * 0.8 + 0.2,
                1.0,
            ];
            draw_text(
                &mut vertices,
                &mut indices,
                &amount_text,
                rx_offset + 32.0,
                y_center - 10.0,
                2.5,
                text_color,
                screen_width,
                screen_height,
            );

            // Net change indicator (smaller, below amount)
            if net != 0 {
                let net_text = if net > 0 {
                    format!("+{}", net)
                } else {
                    format!("{}", net)
                };
                let net_color = if net > 0 {
                    [0.3, 0.9, 0.4, 0.8]
                } else {
                    [0.95, 0.3, 0.3, 0.8]
                };

                draw_text(
                    &mut vertices,
                    &mut indices,
                    &net_text,
                    rx_offset + 32.0,
                    y_center + 6.0,
                    1.5,
                    net_color,
                    screen_width,
                    screen_height,
                );
            }

            rx_offset -= RESOURCE_SPACING - 4.0;
        }

        Mesh { vertices, indices }
    }

    /// Draw a rounded panel (approximated with rectangles)
    fn draw_rounded_panel(
        vertices: &mut Vec<Vertex>,
        indices: &mut Vec<u32>,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: [f32; 4],
        screen_width: f32,
        screen_height: f32,
    ) {
        let to_ndc = |px: f32, py: f32| -> [f32; 3] {
            [
                (px / screen_width) * 2.0 - 1.0,
                1.0 - (py / screen_height) * 2.0,
                0.0,
            ]
        };

        let corner = 4.0_f32.min(width * 0.15).min(height * 0.15);

        // Main body (horizontal)
        add_quad(
            vertices,
            indices,
            to_ndc(x + corner, y),
            to_ndc(x + width - corner, y),
            to_ndc(x + width - corner, y + height),
            to_ndc(x + corner, y + height),
            color,
        );

        // Left edge
        add_quad(
            vertices,
            indices,
            to_ndc(x, y + corner),
            to_ndc(x + corner, y + corner),
            to_ndc(x + corner, y + height - corner),
            to_ndc(x, y + height - corner),
            color,
        );

        // Right edge
        add_quad(
            vertices,
            indices,
            to_ndc(x + width - corner, y + corner),
            to_ndc(x + width, y + corner),
            to_ndc(x + width, y + height - corner),
            to_ndc(x + width - corner, y + height - corner),
            color,
        );
    }

    /// Draw an improved person icon (silhouette)
    fn draw_person_icon(
        vertices: &mut Vec<Vertex>,
        indices: &mut Vec<u32>,
        x: f32,
        y: f32,
        size: f32,
        color: [f32; 4],
        screen_width: f32,
        screen_height: f32,
    ) {
        let to_ndc = |px: f32, py: f32| -> [f32; 3] {
            [
                (px / screen_width) * 2.0 - 1.0,
                1.0 - (py / screen_height) * 2.0,
                0.0,
            ]
        };

        // Head (rounded using multiple rectangles to approximate circle)
        let head_size = size * 0.35;
        let head_cx = x + size / 2.0;
        let head_cy = y + head_size / 2.0;

        // Main head
        add_quad(
            vertices,
            indices,
            to_ndc(head_cx - head_size * 0.35, head_cy - head_size * 0.4),
            to_ndc(head_cx + head_size * 0.35, head_cy - head_size * 0.4),
            to_ndc(head_cx + head_size * 0.35, head_cy + head_size * 0.4),
            to_ndc(head_cx - head_size * 0.35, head_cy + head_size * 0.4),
            color,
        );
        add_quad(
            vertices,
            indices,
            to_ndc(head_cx - head_size * 0.4, head_cy - head_size * 0.3),
            to_ndc(head_cx + head_size * 0.4, head_cy - head_size * 0.3),
            to_ndc(head_cx + head_size * 0.4, head_cy + head_size * 0.3),
            to_ndc(head_cx - head_size * 0.4, head_cy + head_size * 0.3),
            color,
        );

        // Body (shoulders and torso)
        let body_top = y + head_size + size * 0.05;
        let body_width = size * 0.6;
        let body_height = size * 0.55;
        let body_cx = x + size / 2.0;

        // Shoulders (wider top)
        add_quad(
            vertices,
            indices,
            to_ndc(body_cx - body_width * 0.5, body_top),
            to_ndc(body_cx + body_width * 0.5, body_top),
            to_ndc(body_cx + body_width * 0.4, body_top + body_height * 0.4),
            to_ndc(body_cx - body_width * 0.4, body_top + body_height * 0.4),
            color,
        );

        // Torso (narrower at bottom)
        add_quad(
            vertices,
            indices,
            to_ndc(body_cx - body_width * 0.4, body_top + body_height * 0.35),
            to_ndc(body_cx + body_width * 0.4, body_top + body_height * 0.35),
            to_ndc(body_cx + body_width * 0.35, body_top + body_height),
            to_ndc(body_cx - body_width * 0.35, body_top + body_height),
            color,
        );
    }

    /// Draw an improved resource icon
    fn draw_resource_icon(
        vertices: &mut Vec<Vertex>,
        indices: &mut Vec<u32>,
        res_type: ResourceType,
        x: f32,
        y: f32,
        size: f32,
        screen_width: f32,
        screen_height: f32,
    ) {
        let color = res_type.color();
        let icon_color = [
            color[0] as f32 / 255.0,
            color[1] as f32 / 255.0,
            color[2] as f32 / 255.0,
            1.0,
        ];
        let highlight_color = [
            (icon_color[0] + 0.3).min(1.0),
            (icon_color[1] + 0.3).min(1.0),
            (icon_color[2] + 0.3).min(1.0),
            0.8,
        ];

        let to_ndc = |px: f32, py: f32| -> [f32; 3] {
            [
                (px / screen_width) * 2.0 - 1.0,
                1.0 - (py / screen_height) * 2.0,
                0.0,
            ]
        };

        match res_type {
            ResourceType::Gold => {
                // Gold coin - circular with shine
                let cx = x + size / 2.0;
                let cy = y + size / 2.0;
                let r = size * 0.4;

                // Main coin body (octagon approximation)
                add_quad(
                    vertices,
                    indices,
                    to_ndc(cx - r * 0.7, cy - r),
                    to_ndc(cx + r * 0.7, cy - r),
                    to_ndc(cx + r * 0.7, cy + r),
                    to_ndc(cx - r * 0.7, cy + r),
                    icon_color,
                );
                add_quad(
                    vertices,
                    indices,
                    to_ndc(cx - r, cy - r * 0.7),
                    to_ndc(cx + r, cy - r * 0.7),
                    to_ndc(cx + r, cy + r * 0.7),
                    to_ndc(cx - r, cy + r * 0.7),
                    icon_color,
                );

                // Shine highlight
                add_quad(
                    vertices,
                    indices,
                    to_ndc(cx - r * 0.3, cy - r * 0.6),
                    to_ndc(cx + r * 0.2, cy - r * 0.6),
                    to_ndc(cx + r * 0.1, cy - r * 0.2),
                    to_ndc(cx - r * 0.4, cy - r * 0.2),
                    highlight_color,
                );
            }
            ResourceType::Stone => {
                // Rock - irregular polygon
                let cx = x + size / 2.0;
                let cy = y + size / 2.0;

                // Main rock body
                add_quad(
                    vertices,
                    indices,
                    to_ndc(cx - size * 0.35, cy - size * 0.25),
                    to_ndc(cx + size * 0.3, cy - size * 0.35),
                    to_ndc(cx + size * 0.4, cy + size * 0.3),
                    to_ndc(cx - size * 0.3, cy + size * 0.35),
                    icon_color,
                );

                // Dark shadow facet
                let shadow = [
                    icon_color[0] * 0.6,
                    icon_color[1] * 0.6,
                    icon_color[2] * 0.6,
                    1.0,
                ];
                add_quad(
                    vertices,
                    indices,
                    to_ndc(cx - size * 0.3, cy + size * 0.1),
                    to_ndc(cx, cy + size * 0.05),
                    to_ndc(cx + size * 0.1, cy + size * 0.35),
                    to_ndc(cx - size * 0.25, cy + size * 0.35),
                    shadow,
                );
            }
            ResourceType::Wood => {
                // Log - cylinder with rings
                let cx = x + size / 2.0;
                let cy = y + size / 2.0;

                // Log body
                add_quad(
                    vertices,
                    indices,
                    to_ndc(cx - size * 0.4, cy - size * 0.25),
                    to_ndc(cx + size * 0.4, cy - size * 0.25),
                    to_ndc(cx + size * 0.4, cy + size * 0.25),
                    to_ndc(cx - size * 0.4, cy + size * 0.25),
                    icon_color,
                );

                // End grain (lighter)
                add_quad(
                    vertices,
                    indices,
                    to_ndc(cx + size * 0.25, cy - size * 0.2),
                    to_ndc(cx + size * 0.4, cy - size * 0.2),
                    to_ndc(cx + size * 0.4, cy + size * 0.2),
                    to_ndc(cx + size * 0.25, cy + size * 0.2),
                    highlight_color,
                );

                // Growth ring
                let darker = [
                    icon_color[0] * 0.7,
                    icon_color[1] * 0.7,
                    icon_color[2] * 0.7,
                    1.0,
                ];
                add_quad(
                    vertices,
                    indices,
                    to_ndc(cx + size * 0.28, cy - size * 0.08),
                    to_ndc(cx + size * 0.36, cy - size * 0.08),
                    to_ndc(cx + size * 0.36, cy + size * 0.08),
                    to_ndc(cx + size * 0.28, cy + size * 0.08),
                    darker,
                );
            }
            ResourceType::Food => {
                // Wheat - stem with grain head
                let cx = x + size / 2.0;

                // Stem
                let stem_color = [0.45, 0.65, 0.25, 1.0];
                add_quad(
                    vertices,
                    indices,
                    to_ndc(cx - size * 0.04, y + size * 0.4),
                    to_ndc(cx + size * 0.04, y + size * 0.4),
                    to_ndc(cx + size * 0.04, y + size * 0.95),
                    to_ndc(cx - size * 0.04, y + size * 0.95),
                    stem_color,
                );

                // Grain head (golden)
                add_quad(
                    vertices,
                    indices,
                    to_ndc(cx - size * 0.15, y + size * 0.05),
                    to_ndc(cx + size * 0.15, y + size * 0.05),
                    to_ndc(cx + size * 0.1, y + size * 0.45),
                    to_ndc(cx - size * 0.1, y + size * 0.45),
                    icon_color,
                );

                // Grain details
                add_quad(
                    vertices,
                    indices,
                    to_ndc(cx - size * 0.25, y + size * 0.15),
                    to_ndc(cx - size * 0.12, y + size * 0.1),
                    to_ndc(cx - size * 0.08, y + size * 0.35),
                    to_ndc(cx - size * 0.2, y + size * 0.35),
                    icon_color,
                );
                add_quad(
                    vertices,
                    indices,
                    to_ndc(cx + size * 0.12, y + size * 0.1),
                    to_ndc(cx + size * 0.25, y + size * 0.15),
                    to_ndc(cx + size * 0.2, y + size * 0.35),
                    to_ndc(cx + size * 0.08, y + size * 0.35),
                    icon_color,
                );
            }
            ResourceType::Iron => {
                // Iron ingot - 3D trapezoid
                let cx = x + size / 2.0;
                let cy = y + size / 2.0;

                // Top face (lighter)
                add_quad(
                    vertices,
                    indices,
                    to_ndc(cx - size * 0.25, cy - size * 0.3),
                    to_ndc(cx + size * 0.25, cy - size * 0.3),
                    to_ndc(cx + size * 0.35, cy - size * 0.05),
                    to_ndc(cx - size * 0.35, cy - size * 0.05),
                    highlight_color,
                );

                // Front face
                add_quad(
                    vertices,
                    indices,
                    to_ndc(cx - size * 0.35, cy - size * 0.05),
                    to_ndc(cx + size * 0.35, cy - size * 0.05),
                    to_ndc(cx + size * 0.3, cy + size * 0.35),
                    to_ndc(cx - size * 0.3, cy + size * 0.35),
                    icon_color,
                );

                // Side face (darker)
                let darker = [
                    icon_color[0] * 0.65,
                    icon_color[1] * 0.65,
                    icon_color[2] * 0.65,
                    1.0,
                ];
                add_quad(
                    vertices,
                    indices,
                    to_ndc(cx + size * 0.25, cy - size * 0.3),
                    to_ndc(cx + size * 0.35, cy - size * 0.05),
                    to_ndc(cx + size * 0.3, cy + size * 0.35),
                    to_ndc(cx + size * 0.2, cy + size * 0.1),
                    darker,
                );
            }
        }
    }

    /// Toggle visibility
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_top_bar_default() {
        let bar = TopBar::default();
        assert!(bar.visible);
    }

    #[test]
    fn test_top_bar_toggle() {
        let mut bar = TopBar::new();
        assert!(bar.visible);
        bar.toggle();
        assert!(!bar.visible);
        bar.toggle();
        assert!(bar.visible);
    }

    #[test]
    fn test_generate_mesh() {
        let bar = TopBar::new();
        let resources = Resources::new();
        let day_cycle = DayCycle::new();
        let population = Population::new();

        let mesh = bar.generate_ui_mesh(1920.0, 1080.0, &resources, &day_cycle, &population);

        // Should have vertices and indices
        assert!(!mesh.vertices.is_empty());
        assert!(!mesh.indices.is_empty());
    }
}
