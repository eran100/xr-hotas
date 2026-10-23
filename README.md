# XR-HOTAS — Other Games (custom Quest 3 controls)

This is Eran's **general-purpose flight-game** branch of [XR-HOTAS by galister](https://github.com/galister/xr-hotas). It translates VR controller movement into a virtual Linux gamepad named **`XR Fake-HOTAS`** for games that support standard gamepad input.

**Branch:** `eran-other-games`. For the Nuclear Option-specific throttle and keyboard-modifier mappings, see [`eran-nucler-option`](https://github.com/eran100/xr-hotas/tree/eran-nucler-option).

**This README describes the virtual controller outputs, not individual games' in-game bindings.** Assign these axes/buttons inside each game as appropriate.

## What is different about this branch?

- **Throttle direction was inverted** compared with the original setup: left-hand forward/back motion is sent to virtual `ABS_RY = throttle`.
- The left/right Quest triggers are forwarded as **two separate analog axes**, `ABS_Z` and `ABS_RZ`.
- The right trigger is a **normal analog trigger**, **not** a keyboard modifier.
- No separate virtual keyboard device or M/arrow-key modifier layer is created.
- Flight-stick pitch/roll/yaw handling and normal button mapping are shared with the Nuclear Option branch.

This branch is useful where a game's controller layout works better with a throttle on the virtual right-stick Y axis and with both analog triggers available.

## Motion controls

| Meta Quest 3 Touch input | Emulated gamepad axis | Behavior |
| --- | --- | --- |
| Hold **left grip**, move hand forward/backward | `ABS_RY = throttle` | Accumulated throttle; remains at last value when released |
| Hold **right grip**, rotate wrist for roll | `ABS_X = -roll` | Recenters on release |
| Hold **right grip**, rotate wrist for pitch | `ABS_Y = pitch` | Recenters on release |
| Hold **right grip**, rotate wrist for yaw | `ABS_RX = -yaw` | Recenters on release |
| Left analog trigger | `ABS_Z` | Normal analog trigger |
| Right analog trigger | `ABS_RZ` | Normal analog trigger |

The axis codes are Linux input codes. Games may label them differently depending on their gamepad/Steam Input/Proton integration. Note that the physical grip buttons are controls for engaging motion axes, **not** spare gamepad buttons.

## Buttons (no modifier layer)

| Quest input | Emulated gamepad button |
| --- | --- |
| Left X / Y | X / Y |
| Right A / B | A / B |
| Left thumbstick up / down / left / right | D-pad up / down / left / right |
| Left thumbstick click | L3 (left stick click) |
| Right thumbstick up | Back / Select |
| Right thumbstick down | Start |
| Right thumbstick left | LB (left bumper) |
| Right thumbstick right | RB (right bumper) |
| Right thumbstick click | R3 (right stick click) |

Unlike the Nuclear Option branch, **holding the right trigger never switches the thumbstick to keyboard arrow keys/M**. Its analog value remains on `ABS_RZ`.

## Sensitivity settings used in our setup

```sh
~/.local/bin/xr-hotas-other-games --max-distance 0.08 --max-angle 40
```

- `--max-distance 0.08`: 0.08 m / **8 cm** of accumulated left-hand motion covers the full normalized throttle deflection in either direction.
- `--max-angle 40`: approximately **40 degrees** of relative wrist rotation gives full deflection on an individual stick axis.
- Defaults without these flags: 0.2 m and 60 degrees.

### Binding/calibration mode

Send `SIGUSR1` to the running process to toggle **one-axis-at-a-time binding mode**, for assigning axes without unwanted simultaneous pitch/roll/yaw input. Toggle again to disable it for normal flight.

```sh
# Find the running process and note its PID.
pgrep -af xr-hotas

# Replace 12345 with the actual PID.
kill -USR1 12345
```

## Install/update this branch

Our Fedora GNOME setup uses a Distrobox Rust build environment and a separate executable so the two branches can coexist.

```sh
# On Fedora, enter our existing build container.
distrobox enter xr-hotas-build-f44

# Inside Distrobox: build the general-purpose branch.
cargo install --locked --force --git https://github.com/eran100/xr-hotas.git --branch eran-other-games --root ~/build-other-games

# Leave the container.
exit

# On the host: preserve the installed binary before replacing it.
cp ~/.local/bin/xr-hotas-other-games ~/.local/bin/xr-hotas-other-games.bak

# Install the new build for the current user.
install -m 755 ~/.local/share/distrobox-homes/xr-hotas-f44/build-other-games/bin/xr-hotas ~/.local/bin/xr-hotas-other-games
```

Stop any running XR-HOTAS instances first. The container/path above is specific to our Fedora installation; for a different PC, build with Cargo and install the resulting binary under a distinct name.

```sh
# Verification only: the two hashes should match.
sha256sum ~/.local/bin/xr-hotas-other-games ~/.local/share/distrobox-homes/xr-hotas-f44/build-other-games/bin/xr-hotas

# Check exactly which Git revision Cargo built.
cat ~/.local/share/distrobox-homes/xr-hotas-f44/build-other-games/.crates.toml
```

This command builds the code currently on our `eran-other-games` branch, not necessarily the latest upstream galister changes.

## How it fits into VR

A working OpenXR runtime (in our case **WiVRn**, with Quest 3 connected) provides controller position and input. XR-HOTAS creates a Linux virtual gamepad via `uinput`. Your game then reads that device as a conventional controller. [WayVR](https://github.com/wlx-team/wayvr) is optional for extra in-VR controls; it is not needed for the motion-to-gamepad translation itself.

## Original project and Linux VR community

Based on [galister's XR-HOTAS](https://github.com/galister/xr-hotas).

- [Linux VR Discord](https://discord.gg/gHwJ2vwSWV)
- Matrix: `#linux-vr-adventures:matrix.org`
