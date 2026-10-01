# AMD GPU blink(1) utilization indicator

## Purpose

`amdgpu_blink` shows the AMD GPU GFX utilization on a blink(1) USB RGB LED.

The LED is green at low utilization. It changes through yellow at medium utilization. It is red at high utilization.

The program tracks the GFX engine activity that amdgpu_top shows as `GFX`. It does not use fdinfo, media, memory, or GRBM values.

## Metric source options

The current design selects Option 1. Options 1 and 2 use the amdgpu_top GFX metric. Option 3 uses the AMD SMI GFX engine activity.

### Option 1: Run amdgpu_top in JSON mode

The program starts an installed `amdgpu_top` process in JSON mode. The process emits one JSON object for each sample. The `-s` option sets the sample interval.

**Advantages:**

- This option uses amdgpu_top metric code without copying it.
- This option keeps amdgpu_top device support and hardware workarounds.
- An amdgpu_top update can add GPU support without a source copy in this project.
- This option has less AMD metric code in this project.

**Disadvantages:**

- The user must install amdgpu_top before they run this program.
- The program must start, read, and restart a child process.
- A change to the amdgpu_top JSON format can break JSON extraction.
- The source interval and the child-process lifetime add runtime dependencies.

### Option 2: Copy the relevant amdgpu_top code

The program copies the relevant MIT-licensed amdgpu_top code and reads the metric in its own process. A complete copy must include the `GpuActivity` logic and its GPU-metrics support.

**Advantages:**

- The user does not need an installed amdgpu_top executable.
- The program has one process and does not parse JSON.
- The program can report metric errors directly from its Rust code.
- The program can control metric reads and LED updates in one loop.

**Disadvantages:**

- This project must keep the copied code current with amdgpu_top.
- The project must copy or maintain GPU-metrics parsing and AMD dependencies.
- A partial copy that reads only `gpu_busy_percent` can differ from the amdgpu_top GFX value.
- The project must retain MIT license and copyright notices for copied code.

### Option 3: Use AMD SMI through its Rust binding

AMD SMI is the AMD-supported system-management library. Its Rust binding is the `amdsmi` crate in the AMD SMI source tree.

The program initializes AMD GPUs with `amdsmi_init`. It selects a GPU processor handle by PCI BDF. Each sample calls `amdsmi_get_gpu_activity`.

The call returns `AmdsmiEngineUsageT`. The program uses `gfx_activity` as the utilization percentage. AMD SMI also returns UMC and multimedia activity, which this program ignores.

The value `0x0000FFFF` means that GFX activity is unavailable. The program treats this value as an invalid sample.

**Advantages:**

- This option uses AMD's supported GPU-monitoring API.
- The program runs in one process and does not parse JSON.
- The API returns GFX activity directly as a percentage.
- The Rust binding provides safe wrappers for the AMD SMI C API.
- The program does not copy amdgpu_top source code.

**Disadvantages:**

- The host must have the AMD SMI library, `libamd_smi.so`.
- The AMDGPU driver must be loaded before AMD SMI initializes.
- The Rust binding build script requires the AMD SMI library.
- AMD provides binding version 0.1.2 in its source tree, not on crates.io.
- AMD SMI reports `gfx_activity`, but the implementation must compare it with amdgpu_top GFX on supported hardware.

AMD SMI supports Linux bare-metal systems and Linux virtual-machine guests. The program must report an unsupported metric as an error. It must not use a different activity value as a replacement.

## Option 1 metric source

The selected device provides the required value at this JSON path:

```text
devices[0].gpu_activity.GFX.value
```

The value is an integer percentage. It can be `null` when the hardware does not support the metric.

`amdgpu_top` calls this metric `GFX`. It uses `average_gfx_activity` from `gpu_metrics` when available.

If GPU metrics are not available, `amdgpu_top` reads `gpu_busy_percent` from sysfs. It does not use this fallback for Raven-family APUs.

Option 1 uses `amdgpu_top` to keep these hardware-specific rules. It does not copy the GPU metrics parser.

The program starts this child process for PCI selection:

```text
amdgpu_top --json --single --pci <PCI-BDF> -s <interval-ms>
```

For GPU-index selection, it starts this process:

```text
amdgpu_top --json --single -i <index> -s <interval-ms>
```

The user must select a GPU if more than one AMD GPU exists. The program selects the GPU automatically if exactly one AMD GPU exists.

For Option 1, `amdgpu_top` is a runtime requirement. This project does not install it.

The program reads JSON records from the child process standard output. It reports standard error output for diagnosis. It does not parse standard error output.

The program accepts a record only if all conditions are true:

- The selected result contains one device.
- The GFX value is numeric.
- The GFX value is from 0 through 100.

## Color scale

Let `u` be the valid GFX percentage. Blue is always zero. The program calculates the color as follows:

| GFX range | RGB value | Color |
| --- | --- | --- |
| `0..=20` | `(0, 255, 0)` | Green |
| `20..=50` | `(round(255 × (u - 20) / 30), 255, 0)` | Green to yellow |
| `50..=80` | `(255, round(255 × (80 - u) / 30), 0)` | Yellow to red |
| `80..=100` | `(255, 0, 0)` | Red |

At 20%, the color is green. At 50%, the color is yellow (`#ffff00`). At 80%, the color is red.

The program uses `blink1rs::Color::rgb(r, g, 0)`. blink1rs gamma correction stays off. Thus, the program sends the calculated RGB bytes to the device.

## Application design

The Rust binary contains these modules:

- `cli` parses GPU, blink(1), and timing options.
- `source` starts `amdgpu_top`, reads records, and extracts the GFX value.
- `color` contains `gfx_to_color(u16) -> Color`.
- `light` opens and updates the blink(1) device.
- `main` connects the source and light modules. It also handles signals and retries.

The program has this command-line interface:

```text
amdgpu_blink [--pci BDF | --gpu-index N] [--blink-serial SERIAL | --blink-index N]
             [--interval-ms N] [--fade-ms N] [--verbose]
```

The default source interval is 1000 ms. The default fade time is 200 ms.

The first device in `Blink1::list()` order is the default blink(1). A serial number gives stable selection in a multi-device installation.

The program uses `Blink1::open_serial` for a serial number. It uses `Blink1::open_index` for an index. It calls `fade` for each valid color.

blink1rs makes `Blink1` `Send`, but not `Sync`. The first version keeps the device handle in one update thread.

`Cargo.toml` will include this dependency:

```toml
blink1rs = { version = "0.2", default-features = false }
```

This setting excludes the blink1rs command-line program. The implementation will select Rust crates for JSON parsing and option parsing. This design task installs no software.

## Error behavior

A stale green light must not report healthy monitoring. The program configures the blink(1) watchdog at startup.

Mk2 firmware 203 or later uses a solid-blue RAM pattern (`#0000ff`). The watchdog starts this pattern after five seconds without a tickle.

The program fills every RAM pattern line with blue. It does not save the pattern to flash. This supports firmware 203, which uses the full pattern when the watchdog end position is zero.

After the program parses a valid GFX value, it tickles the watchdog with the keep-current-color flag. This flag prevents the tickle from clearing the LED. The `blink1rs` name for this flag is `OnTimeout::StayLit`. On mk2, the flag controls the tickle, not the timeout action.

The program sends a fade command only when the target color changes. The fade starts from the current color. After a watchdog timeout, the program sends the utilization color again, even if the target is unchanged.

Firmware 204 limits the practical timeout to about 62 seconds.

The program reports an error and retries if one of these conditions occurs:

- `amdgpu_top` exits.
- The child process writes invalid JSON.
- The GFX value is missing or invalid.
- The LED update fails.

The retry delay increases from one second to 30 seconds. The program does not tickle the watchdog while it retries.

On `SIGINT` or `SIGTERM`, the program stops the child process. It disables the watchdog and turns the LED off.

If the process stops unexpectedly, the watchdog changes the LED to blue on mk2 firmware 203 or later. Older firmware retains the previous watchdog configuration.

Linux users need permission for the selected DRM device and the blink(1) HID device. blink1rs documents a udev rule for the HID device. This project documents the rule but does not change system rules.

## Tests

Unit tests do not require a GPU or a blink(1) device. The tests cover these cases:

- The color anchors at 0, 20, 50, 80, and 100.
- Values between the color anchors.
- RGB channels from 0 through 255.
- Valid GFX JSON records.
- `null`, missing, invalid, and out-of-range GFX values.
- Invalid JSON records.
- Records with more than one device.
- PCI and GPU-index command construction.
- Conflicting GPU selector options.

Hardware tests are ignored by default. They open a connected blink(1) and send a set or fade command.

Hardware tests must run one at a time. The device has one HID handle.

A manual integration test runs the program with `amdgpu_top --json`. At idle, the LED is green. During a GPU workload, the LED moves toward yellow or red.

## Non-goals

This project does not do the following:

- Show compute, memory, media, temperature, process, or GRBM data.
- Parse AMD `gpu_metrics` data.
- Copy amdgpu_top device selection or hardware workarounds.
- Install amdgpu_top, AMD SMI, HID libraries, udev rules, or a system service.
- Save blink(1) patterns after the program stops.

## Implementation order

1. Create the Cargo binary. Add color and JSON extraction tests.
2. Add child-process control and JSON-line parsing.
3. Add blink1rs device control, fades, and watchdog ticks.
4. Add signal cleanup, retry messages, user documentation, and ignored hardware tests.
