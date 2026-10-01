mod cli;
mod color;
mod light;
mod source;

use std::process::Command;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use clap::Parser;

use crate::cli::Cli;
use crate::color::gfx_to_color;
use crate::light::Light;
use crate::source::{MetricSource, source_command};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let stop = Arc::new(AtomicBool::new(false));
    let signal_stop = Arc::clone(&stop);
    ctrlc::set_handler(move || signal_stop.store(true, Ordering::SeqCst))?;

    let selector = cli.gpu_selector();
    let mut light: Option<Light> = None;
    let mut retry_delay = Duration::from_secs(1);

    while !stop.load(Ordering::SeqCst) {
        if light.is_none() {
            verbose(cli.verbose, "opening blink(1)");
            match Light::open(cli.blink_serial.as_deref(), cli.blink_index, cli.fade_ms) {
                Ok(opened_light) => {
                    let description = opened_light.description();
                    light = Some(opened_light);
                    verbose(
                        cli.verbose,
                        format!("blink(1) opened and watchdog armed: {description}"),
                    );
                }
                Err(error) => {
                    eprintln!("cannot open blink(1): {error}");
                    wait_for_retry(&stop, retry_delay);
                    retry_delay = next_retry_delay(retry_delay);
                    continue;
                }
            }
        }

        let command = source_command(&selector, cli.interval_ms);
        verbose(
            cli.verbose,
            format!("starting {}", format_command(&command)),
        );
        let mut source = match MetricSource::start(&selector, cli.interval_ms) {
            Ok(source) => source,
            Err(error) => {
                eprintln!("{error}");
                wait_for_retry(&stop, retry_delay);
                retry_delay = next_retry_delay(retry_delay);
                continue;
            }
        };

        let mut got_sample = false;
        while !stop.load(Ordering::SeqCst) {
            match source.next_gfx() {
                Ok(gfx) => {
                    got_sample = true;
                    let color = gfx_to_color(gfx);
                    let result = light.as_mut().expect("the light is open").show(color);
                    if let Err(error) = result {
                        eprintln!("cannot update blink(1): {error}");
                        light = None;
                        break;
                    }
                    verbose(cli.verbose, format!("GFX is {gfx}%, set {color}"));
                }
                Err(error) => {
                    if !stop.load(Ordering::SeqCst) {
                        eprintln!("{error}");
                    }
                    break;
                }
            }
        }

        if got_sample {
            retry_delay = Duration::from_secs(1);
        }

        if !stop.load(Ordering::SeqCst) {
            wait_for_retry(&stop, retry_delay);
            retry_delay = next_retry_delay(retry_delay);
        }
    }

    if let Some(mut light) = light
        && let Err(error) = light.shutdown()
    {
        eprintln!("cannot turn off blink(1): {error}");
    }

    Ok(())
}

fn verbose(enabled: bool, message: impl std::fmt::Display) {
    if enabled {
        eprintln!("verbose: {message}");
    }
}

fn format_command(command: &Command) -> String {
    std::iter::once(command.get_program())
        .chain(command.get_args())
        .map(|part| part.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ")
}

fn next_retry_delay(current: Duration) -> Duration {
    (current * 2).min(Duration::from_secs(30))
}

fn wait_for_retry(stop: &AtomicBool, delay: Duration) {
    let step = Duration::from_millis(100);
    let mut remaining = delay;

    while remaining > Duration::ZERO && !stop.load(Ordering::SeqCst) {
        let sleep_for = remaining.min(step);
        std::thread::sleep(sleep_for);
        remaining = remaining.saturating_sub(sleep_for);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caps_the_retry_delay() {
        assert_eq!(
            next_retry_delay(Duration::from_secs(1)),
            Duration::from_secs(2)
        );
        assert_eq!(
            next_retry_delay(Duration::from_secs(20)),
            Duration::from_secs(30)
        );
    }
}
