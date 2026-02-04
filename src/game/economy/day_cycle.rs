//! Day/Night Cycle
//!
//! 1 in-game day = 10 real minutes (600 seconds)
//! Day is divided into phases: Dawn, Day, Dusk, Night

/// Duration of one in-game day in real seconds
pub const DAY_DURATION_SECONDS: f32 = 600.0; // 10 minutes

/// Time phases of the day
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeOfDay {
    /// Early morning (0.0 - 0.15)
    Dawn,
    /// Daytime (0.15 - 0.65)
    Day,
    /// Evening (0.65 - 0.8)
    Dusk,
    /// Nighttime (0.8 - 1.0)
    Night,
}

impl TimeOfDay {
    /// Get sun intensity (0-1) for this time
    pub fn sun_intensity(&self) -> f32 {
        match self {
            TimeOfDay::Dawn => 0.3,
            TimeOfDay::Day => 1.0,
            TimeOfDay::Dusk => 0.4,
            TimeOfDay::Night => 0.05,
        }
    }

    /// Get ambient light color
    pub fn ambient_color(&self) -> [f32; 3] {
        match self {
            TimeOfDay::Dawn => [1.0, 0.7, 0.5],   // Orange
            TimeOfDay::Day => [1.0, 1.0, 0.95],   // Slightly warm white
            TimeOfDay::Dusk => [1.0, 0.5, 0.3],   // Red-orange
            TimeOfDay::Night => [0.2, 0.2, 0.4],  // Blue-ish
        }
    }

    /// Get sky color at horizon
    pub fn sky_color(&self) -> [f32; 3] {
        match self {
            TimeOfDay::Dawn => [0.8, 0.5, 0.3],
            TimeOfDay::Day => [0.5, 0.7, 1.0],
            TimeOfDay::Dusk => [0.8, 0.3, 0.2],
            TimeOfDay::Night => [0.05, 0.05, 0.15],
        }
    }
}

/// Day/Night cycle manager
#[derive(Debug, Clone)]
pub struct DayCycle {
    /// Current time in day (0.0 to 1.0)
    time: f32,
    /// Day number (starts at 1)
    day_number: u32,
    /// Is time paused?
    paused: bool,
    /// Time scale (1.0 = normal, 2.0 = double speed)
    time_scale: f32,
    /// Total elapsed time (for stats)
    total_elapsed: f32,
}

impl Default for DayCycle {
    fn default() -> Self {
        Self::new()
    }
}

impl DayCycle {
    /// Create starting at dawn of day 1
    pub fn new() -> Self {
        Self {
            time: 0.1, // Start at dawn
            day_number: 1,
            paused: false,
            time_scale: 1.0,
            total_elapsed: 0.0,
        }
    }

    /// Update the cycle
    /// Returns true if a new day started
    pub fn update(&mut self, delta_seconds: f32) -> bool {
        if self.paused {
            return false;
        }

        self.total_elapsed += delta_seconds;

        let time_delta = (delta_seconds * self.time_scale) / DAY_DURATION_SECONDS;
        self.time += time_delta;

        if self.time >= 1.0 {
            self.time -= 1.0;
            self.day_number += 1;
            return true;
        }

        false
    }

    /// Get current time of day phase
    pub fn time_of_day(&self) -> TimeOfDay {
        if self.time < 0.15 {
            TimeOfDay::Dawn
        } else if self.time < 0.65 {
            TimeOfDay::Day
        } else if self.time < 0.8 {
            TimeOfDay::Dusk
        } else {
            TimeOfDay::Night
        }
    }

    /// Get current time (0-1)
    pub fn time(&self) -> f32 {
        self.time
    }

    /// Get current day number
    pub fn day(&self) -> u32 {
        self.day_number
    }

    /// Get sun angle (radians, 0 = horizon east, PI = horizon west)
    pub fn sun_angle(&self) -> f32 {
        // Sun rises at dawn (0.0), peaks at noon (0.4), sets at dusk (0.8)
        let sun_time = (self.time - 0.1).clamp(0.0, 0.7) / 0.7;
        sun_time * std::f32::consts::PI
    }

    /// Get sun direction vector
    pub fn sun_direction(&self) -> [f32; 3] {
        let angle = self.sun_angle();
        let y = angle.sin(); // Height
        let x = angle.cos(); // East-West
        [x, y.max(0.0), 0.3] // Slight south offset
    }

    /// Get interpolated sun intensity (smoother than phase-based)
    pub fn sun_intensity(&self) -> f32 {
        let base = self.time_of_day().sun_intensity();

        // Smooth transitions
        match self.time {
            t if t < 0.15 => {
                // Dawn transition
                let progress = t / 0.15;
                0.05 + progress * 0.25
            }
            t if t >= 0.15 && t < 0.4 => {
                // Morning rise
                let progress = (t - 0.15) / 0.25;
                0.3 + progress * 0.7
            }
            t if t >= 0.4 && t < 0.5 => {
                // Peak
                1.0
            }
            t if t >= 0.5 && t < 0.65 => {
                // Afternoon decline
                let progress = (t - 0.5) / 0.15;
                1.0 - progress * 0.3
            }
            t if t >= 0.65 && t < 0.8 => {
                // Dusk
                let progress = (t - 0.65) / 0.15;
                0.7 - progress * 0.3
            }
            _ => {
                // Night
                base
            }
        }
    }

    /// Get formatted time string (HH:MM)
    pub fn time_string(&self) -> String {
        let hours = (self.time * 24.0) as u32;
        let minutes = ((self.time * 24.0 * 60.0) % 60.0) as u32;
        format!("{:02}:{:02}", hours, minutes)
    }

    /// Get remaining time in day (seconds)
    pub fn remaining_seconds(&self) -> f32 {
        (1.0 - self.time) * DAY_DURATION_SECONDS / self.time_scale
    }

    /// Pause/unpause time
    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }

    /// Is time paused?
    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// Set time scale
    pub fn set_time_scale(&mut self, scale: f32) {
        self.time_scale = scale.max(0.1).min(10.0);
    }

    /// Get time scale
    pub fn time_scale(&self) -> f32 {
        self.time_scale
    }

    /// Skip to next dawn
    pub fn skip_to_dawn(&mut self) {
        if self.time < 0.1 {
            self.time = 0.1;
        } else {
            self.time = 0.1;
            self.day_number += 1;
        }
    }

    /// Get total elapsed real time
    pub fn total_elapsed(&self) -> f32 {
        self.total_elapsed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_day_cycle() {
        let mut cycle = DayCycle::new();

        assert_eq!(cycle.day(), 1);
        assert_eq!(cycle.time_of_day(), TimeOfDay::Dawn);

        // Advance to midday
        cycle.update(300.0); // 5 minutes = half day
        assert_eq!(cycle.time_of_day(), TimeOfDay::Day);

        // Advance to next day
        let new_day = cycle.update(400.0);
        assert!(new_day);
        assert_eq!(cycle.day(), 2);
    }

    #[test]
    fn test_time_string() {
        let mut cycle = DayCycle::new();
        cycle.time = 0.5; // Noon

        let time_str = cycle.time_string();
        assert_eq!(time_str, "12:00");
    }

    #[test]
    fn test_sun_direction() {
        let mut cycle = DayCycle::new();

        // At dawn, sun should be low
        cycle.time = 0.1;
        let dir = cycle.sun_direction();
        assert!(dir[1] < 0.3); // Low Y

        // At noon, sun should be high
        cycle.time = 0.4;
        let dir = cycle.sun_direction();
        assert!(dir[1] > 0.8); // High Y
    }
}
