# amdgpu_blink

`amdgpu_blink` shows AMD GPU GFX utilization on a blink(1) USB LED.

The LED is green from 0% through 20%. It changes from green to yellow from 20% through 50%. It changes from yellow to red from 50% through 80%. It is red from 80% through 100%.

## Requirements

- Linux with an AMD GPU and the `amdgpu` driver.
- `amdgpu_top` in `PATH`.
- A blink(1) USB LED with permission for the current user.

The program starts `amdgpu_top --json` and reads `gpu_activity.GFX.value`. It does not install amdgpu_top.

blink1rs documents a udev rule for blink(1) access. Install this rule on the host if the program cannot find the LED:

```sh
sudo tee /etc/udev/rules.d/51-blink1.rules <<'EOF'
ATTRS{idVendor}=="27b8", ATTRS{idProduct}=="01ed", \
  MODE:="666", GROUP="plugdev"
EOF
sudo udevadm control --reload
sudo udevadm trigger
```

Replug the blink(1) after you install the rule.

## Build

```sh
cargo build --release
```

## Run

Select a GPU by PCI BDF:

```sh
cargo run --release -- --pci 0000:03:00.0
```

Or select a GPU by amdgpu_top index:

```sh
cargo run --release -- --gpu-index 0
```

If the system has one AMD GPU, you can omit the GPU option. If the system has more than one AMD GPU, the program reports an error.

Use `--blink-serial` to select one blink(1). Use `--blink-index` to select it by its blink1rs index.

```sh
cargo run --release -- --pci 0000:03:00.0 --blink-serial 20001234
```

The default sample interval is 1000 ms. The default fade time is 200 ms. The interval must be 4000 ms or less because the blink(1) watchdog acts after five seconds.

Use `--verbose` to show the selected blink(1) serial number, device type, firmware version, the `amdgpu_top` command, each GFX value, and its LED color.

At startup, the program stops a stored blink(1) pattern. A stored pattern can overwrite immediate color commands. Use `--fade-ms 0` to set each color immediately during diagnosis.

The program keeps an unchanged color steady. When the color changes, the LED fades from its current color without an off step.

On a normal stop, the program turns the LED off. On mk2 firmware 203 or later, an unexpected stop changes the LED to blue (`#0000ff`) after five seconds.

The program replaces the RAM color pattern with blue. It does not save this pattern to flash. Older firmware retains the previous watchdog configuration.

## Test

```sh
cargo test
```

The tests do not need a GPU, `amdgpu_top`, or a blink(1) device.
