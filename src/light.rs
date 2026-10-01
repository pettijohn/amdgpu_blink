use std::time::{Duration, Instant};

use blink1rs::{Blink1, Color, Error, Led, OnTimeout};

const WATCHDOG_TIMEOUT: Duration = Duration::from_secs(5);
const BLUE_WATCHDOG_MINIMUM_FIRMWARE: u16 = 203;

// Separate the update sequence from USB transport for regression tests.
trait ColorDevice {
    fn tickle(&mut self) -> Result<(), Error>;
    fn fade_to(&mut self, color: Color, duration: Duration) -> Result<(), Error>;
}

impl ColorDevice for Blink1 {
    fn tickle(&mut self) -> Result<(), Error> {
        self.watchdog_tickle()
    }

    fn fade_to(&mut self, color: Color, duration: Duration) -> Result<(), Error> {
        self.fade(color, duration)
    }
}

fn update_color(
    device: &mut impl ColorDevice,
    previous: &mut Option<Color>,
    color: Color,
    fade: Duration,
) -> Result<(), Error> {
    // StayLit sets the wire protocol's 'st' flag. On mk2 this preserves
    // the current color during tickles, NOT after the watchdog expires.
    device.tickle()?;
    if *previous != Some(color) {
        device.fade_to(color, fade)?;
        *previous = Some(color);
    }
    Ok(())
}

pub struct Light {
    device: Blink1,
    fade: Duration,
    firmware: u16,
    watchdog_fallback: &'static str,
    previous: Option<Color>,
    last_update: Option<Instant>,
}

impl Light {
    pub fn open(serial: Option<&str>, index: Option<usize>, fade_ms: u64) -> Result<Self, Error> {
        let mut device = match (serial, index) {
            (Some(serial), None) => Blink1::open_serial(serial)?,
            (None, Some(index)) => Blink1::open_index(index)?,
            (None, None) => Blink1::open()?,
            (Some(_), Some(_)) => unreachable!("clap rejects conflicting blink(1) selectors"),
        };

        // A stored pattern can continue to overwrite immediate color commands.
        device.stop()?;
        let firmware = device.firmware_version()?;
        let watchdog_fallback = if firmware >= BLUE_WATCHDOG_MINIMUM_FIRMWARE {
            // StayLit sends start=0/end=0. Firmware 203 interprets this as
            // the entire pattern, and starts it when the watchdog expires.
            // Fill every RAM line so no old pattern colors can appear.
            // A nonzero duration keeps the pattern scheduler advancing.
            for line in 0..device.pattern_max() {
                device.write_pattern_line(
                    line,
                    Color::BLUE,
                    Duration::from_millis(100),
                    Led::All,
                )?;
            }
            device.watchdog_enable(WATCHDOG_TIMEOUT, OnTimeout::StayLit)?;
            "blue"
        } else {
            // Keep the existing behavior for firmware not checked here.
            device.watchdog_enable(WATCHDOG_TIMEOUT, OnTimeout::Off)?;
            "off (firmware cannot play a blue watchdog pattern)"
        };

        Ok(Self {
            device,
            fade: Duration::from_millis(fade_ms),
            firmware,
            watchdog_fallback,
            previous: None,
            last_update: None,
        })
    }

    pub fn description(&self) -> String {
        format!(
            "serial {}, kind {}, firmware {}, watchdog fallback {}",
            self.device.serial(),
            self.device.kind(),
            self.firmware,
            self.watchdog_fallback,
        )
    }

    pub fn show(&mut self, color: Color) -> Result<(), Error> {
        let now = Instant::now();
        if self.firmware < BLUE_WATCHDOG_MINIMUM_FIRMWARE
            || self
                .last_update
                .is_some_and(|last| now.duration_since(last) >= WATCHDOG_TIMEOUT)
        {
            // A timeout can replace the target with blue. Re-send even if
            // the first recovered sample has the same utilization color.
            self.previous = None;
        }
        update_color(&mut self.device, &mut self.previous, color, self.fade)?;
        self.last_update = Some(now);
        Ok(())
    }

    pub fn shutdown(&mut self) -> Result<(), Error> {
        let watchdog_result = self.device.watchdog_disable();
        let off_result = self.device.off();
        watchdog_result.and(off_result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Firmware203 {
        color: Color,
        commands: Vec<char>,
    }

    impl ColorDevice for Firmware203 {
        fn tickle(&mut self) -> Result<(), Error> {
            self.commands.push('D');
            // Model the mk2 'D' command with st=1: do not clear color.
            Ok(())
        }

        fn fade_to(&mut self, color: Color, _: Duration) -> Result<(), Error> {
            self.commands.push('c');
            self.color = color;
            Ok(())
        }
    }

    #[test]
    fn mk2_firmware_203_keeps_color_after_watchdog_tickle() {
        for fade in [Duration::ZERO, Duration::from_millis(200)] {
            let mut device = Firmware203::default();
            let mut previous = None;
            for color in [Color::GREEN, Color::YELLOW, Color::RED] {
                update_color(&mut device, &mut previous, color, fade).unwrap();
                assert_eq!(device.color, color);
            }
            assert_eq!(device.commands, ['D', 'c', 'D', 'c', 'D', 'c']);
        }
    }

    #[test]
    fn unchanged_color_only_tickles_watchdog() {
        let mut device = Firmware203::default();
        let mut previous = None;
        for _ in 0..10 {
            update_color(
                &mut device,
                &mut previous,
                Color::RED,
                Duration::from_millis(200),
            )
            .unwrap();
            assert_eq!(device.color, Color::RED);
        }
        assert_eq!(device.commands.iter().filter(|&&c| c == 'c').count(), 1);
        assert_eq!(device.commands.iter().filter(|&&c| c == 'D').count(), 10);
    }

    #[test]
    fn changed_color_fades_from_existing_color() {
        let mut device = Firmware203 {
            color: Color::RED,
            commands: vec![],
        };
        device.tickle().unwrap();
        assert_eq!(device.color, Color::RED);
        let mut previous = Some(Color::RED);
        update_color(
            &mut device,
            &mut previous,
            Color::GREEN,
            Duration::from_millis(200),
        )
        .unwrap();
        assert_eq!(device.color, Color::GREEN);
    }

    #[test]
    fn invalidated_cache_restores_color_after_timeout() {
        let mut device = Firmware203 {
            color: Color::BLUE,
            commands: vec![],
        };
        let mut previous = None;
        update_color(&mut device, &mut previous, Color::RED, Duration::ZERO).unwrap();
        assert_eq!(device.color, Color::RED);
        assert_eq!(device.commands, ['D', 'c']);
    }
}
