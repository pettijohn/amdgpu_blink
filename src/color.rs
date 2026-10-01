use blink1rs::Color;

/// Convert a GFX-utilization percentage to the indicator color.
pub fn gfx_to_color(utilization: u16) -> Color {
    let utilization = utilization.min(100);

    match utilization {
        0..=20 => Color::GREEN,
        21..=50 => Color::rgb(interpolate(0, 255, utilization - 20, 30), 255, 0),
        51..=80 => Color::rgb(255, interpolate(255, 0, utilization - 50, 30), 0),
        _ => Color::RED,
    }
}

fn interpolate(start: u8, end: u8, position: u16, length: u16) -> u8 {
    let length = i32::from(length);
    let value =
        i32::from(start) * length + (i32::from(end) - i32::from(start)) * i32::from(position);
    ((value + length / 2) / length).clamp(0, 255) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_anchor_values() {
        assert_eq!(gfx_to_color(0), Color::GREEN);
        assert_eq!(gfx_to_color(20), Color::GREEN);
        assert_eq!(gfx_to_color(50), Color::YELLOW);
        assert_eq!(gfx_to_color(80), Color::RED);
        assert_eq!(gfx_to_color(100), Color::RED);
    }

    #[test]
    fn interpolates_between_anchors() {
        assert_eq!(gfx_to_color(35), Color::rgb(128, 255, 0));
        assert_eq!(gfx_to_color(65), Color::rgb(255, 128, 0));
    }

    #[test]
    fn clamps_values_above_one_hundred() {
        assert_eq!(gfx_to_color(101), Color::RED);
        assert_eq!(gfx_to_color(u16::MAX), Color::RED);
    }
}
