use clap::Parser;

const MAX_INTERVAL_MS: u64 = 4_000;

#[derive(Debug, Parser)]
#[command(about = "Show AMD GPU GFX utilization on a blink(1) LED")]
pub struct Cli {
    /// Select the AMD GPU by PCI BDF, for example 0000:03:00.0.
    #[arg(long, conflicts_with = "gpu_index")]
    pub pci: Option<String>,

    /// Select the AMD GPU by amdgpu_top GPU index.
    #[arg(long, conflicts_with = "pci")]
    pub gpu_index: Option<usize>,

    /// Select the blink(1) by serial number.
    #[arg(long, conflicts_with = "blink_index")]
    pub blink_serial: Option<String>,

    /// Select the blink(1) by its Blink1::list() index.
    #[arg(long, conflicts_with = "blink_serial")]
    pub blink_index: Option<usize>,

    /// Set the amdgpu_top sample interval in milliseconds, from 1 through 4000.
    #[arg(long, default_value_t = 1_000, value_parser = parse_interval)]
    pub interval_ms: u64,

    /// Set the blink(1) fade time in milliseconds.
    #[arg(long, default_value_t = 200)]
    pub fade_ms: u64,

    /// Show source, sample, and blink(1) status messages.
    #[arg(short, long)]
    pub verbose: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GpuSelector {
    Auto,
    Pci(String),
    Index(usize),
}

impl Cli {
    pub fn gpu_selector(&self) -> GpuSelector {
        match (&self.pci, self.gpu_index) {
            (Some(pci), None) => GpuSelector::Pci(pci.clone()),
            (None, Some(index)) => GpuSelector::Index(index),
            (None, None) => GpuSelector::Auto,
            (Some(_), Some(_)) => unreachable!("clap rejects conflicting GPU selectors"),
        }
    }
}

fn parse_interval(value: &str) -> Result<u64, String> {
    let milliseconds: u64 = value
        .parse()
        .map_err(|_| "the interval must be an integer".to_string())?;

    if !(1..=MAX_INTERVAL_MS).contains(&milliseconds) {
        return Err(format!(
            "the interval must be from 1 through {MAX_INTERVAL_MS} ms"
        ));
    }

    Ok(milliseconds)
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    #[test]
    fn parses_pci_selection() {
        let cli = Cli::try_parse_from(["amdgpu_blink", "--pci", "0000:03:00.0"]).unwrap();
        assert_eq!(
            cli.gpu_selector(),
            GpuSelector::Pci("0000:03:00.0".to_string())
        );
    }

    #[test]
    fn rejects_conflicting_gpu_selection() {
        assert!(
            Cli::try_parse_from(["amdgpu_blink", "--pci", "0000:03:00.0", "--gpu-index", "0"])
                .is_err()
        );
    }

    #[test]
    fn rejects_an_interval_longer_than_the_watchdog_timeout() {
        assert!(Cli::try_parse_from(["amdgpu_blink", "--interval-ms", "4001"]).is_err());
    }
}
