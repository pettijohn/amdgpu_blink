use std::io::{BufRead, BufReader};
use std::process::{Child, ChildStdout, Command, Stdio};

use serde_json::Value;
use thiserror::Error;

use crate::cli::GpuSelector;

#[derive(Debug, Error)]
pub enum SourceError {
    #[error("cannot start amdgpu_top: {0}")]
    Start(#[source] std::io::Error),
    #[error("cannot read amdgpu_top output: {0}")]
    Read(#[source] std::io::Error),
    #[error("amdgpu_top ended before it returned a sample")]
    Ended,
    #[error("amdgpu_top wrote invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("the amdgpu_top record must contain exactly one selected device, but it contains {0}")]
    DeviceCount(usize),
    #[error("the amdgpu_top record has no usable GFX percentage")]
    MissingGfx,
    #[error("the GFX percentage {0} is outside the range 0 through 100")]
    InvalidGfx(u64),
}

pub struct MetricSource {
    child: Child,
    lines: std::io::Lines<BufReader<ChildStdout>>,
}

impl MetricSource {
    pub fn start(selector: &GpuSelector, interval_ms: u64) -> Result<Self, SourceError> {
        let mut command = source_command(selector, interval_ms);
        let mut child = command
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(SourceError::Start)?;
        let stdout = child.stdout.take().expect("stdout is piped");

        Ok(Self {
            child,
            lines: BufReader::new(stdout).lines(),
        })
    }

    pub fn next_gfx(&mut self) -> Result<u16, SourceError> {
        let line = self
            .lines
            .next()
            .ok_or(SourceError::Ended)?
            .map_err(SourceError::Read)?;
        extract_gfx(&line)
    }
}

impl Drop for MetricSource {
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(None)) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

pub fn source_command(selector: &GpuSelector, interval_ms: u64) -> Command {
    let mut command = Command::new("amdgpu_top");
    command.arg("--json").arg("-s").arg(interval_ms.to_string());

    match selector {
        GpuSelector::Auto => {}
        GpuSelector::Pci(pci) => {
            command.arg("--single").arg("--pci").arg(pci);
        }
        GpuSelector::Index(index) => {
            command.arg("--single").arg("-i").arg(index.to_string());
        }
    }

    command
}

pub fn extract_gfx(json: &str) -> Result<u16, SourceError> {
    let value: Value = serde_json::from_str(json)?;
    let devices = value
        .get("devices")
        .and_then(Value::as_array)
        .ok_or(SourceError::DeviceCount(0))?;

    if devices.len() != 1 {
        return Err(SourceError::DeviceCount(devices.len()));
    }

    let gfx = devices[0]
        .get("gpu_activity")
        .and_then(|activity| activity.get("GFX"))
        .and_then(|gfx| gfx.get("value"))
        .and_then(Value::as_u64)
        .ok_or(SourceError::MissingGfx)?;

    if gfx > 100 {
        return Err(SourceError::InvalidGfx(gfx));
    }

    Ok(gfx as u16)
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = r#"{"devices":[{"gpu_activity":{"GFX":{"value":50,"unit":"%"}}}]}"#;

    #[test]
    fn extracts_gfx() {
        assert_eq!(extract_gfx(VALID).unwrap(), 50);
    }

    #[test]
    fn rejects_missing_and_invalid_gfx() {
        assert!(matches!(
            extract_gfx(r#"{"devices":[{"gpu_activity":{"GFX":{"value":null}}}]}"#),
            Err(SourceError::MissingGfx)
        ));
        assert!(matches!(
            extract_gfx(r#"{"devices":[{}]}"#),
            Err(SourceError::MissingGfx)
        ));
        assert!(matches!(
            extract_gfx(r#"{"devices":[{"gpu_activity":{"GFX":{"value":101}}}]}"#),
            Err(SourceError::InvalidGfx(101))
        ));
    }

    #[test]
    fn rejects_bad_json_and_multiple_devices() {
        assert!(matches!(extract_gfx("not json"), Err(SourceError::Json(_))));
        assert!(matches!(
            extract_gfx(r#"{"devices":[]}"#),
            Err(SourceError::DeviceCount(0))
        ));
        assert!(matches!(
            extract_gfx(r#"{"devices":[{},{}]}"#),
            Err(SourceError::DeviceCount(2))
        ));
    }

    #[test]
    fn constructs_pci_command() {
        let command = source_command(&GpuSelector::Pci("0000:03:00.0".to_string()), 1_000);
        let arguments: Vec<_> = command
            .get_args()
            .map(|argument| argument.to_string_lossy())
            .collect();
        assert_eq!(
            arguments,
            ["--json", "-s", "1000", "--single", "--pci", "0000:03:00.0"]
        );
    }

    #[test]
    fn constructs_index_command() {
        let command = source_command(&GpuSelector::Index(2), 1_000);
        let arguments: Vec<_> = command
            .get_args()
            .map(|argument| argument.to_string_lossy())
            .collect();
        assert_eq!(arguments, ["--json", "-s", "1000", "--single", "-i", "2"]);
    }
}
