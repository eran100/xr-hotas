# XR-HOTAS — Nuclear Option (custom Quest 3 controls)

This is Eran's **Nuclear Option-specific** branch of [XR-HOTAS by galister](https://github.com/galister/xr-hotas). It turns tracked VR hand-controller movement into a virtual gamepad for Linux/Proton games. It is tailored to **Meta Quest 3 Touch controllers, WiVRn and Nuclear Option**.

**Branch:** `eran-nucler-option` (the spelling is intentional; use it exactly).

**This README documents the inputs produced by XR-HOTAS, not Nuclear Option's in-game action bindings.** Configure the desired aircraft functions in the game's Controls menu.

For conventional flight games, see the sibling [`eran-other-games` branch](https://github.com/eran100/xr-hotas/tree/eran-other-games). **Do not install the unmodified upstream branch if you want these controls.**

## What this branch changes

- Left-hand grip + forward/back motion controls a **latched virtual throttle** on `ABS_Z` (inverted output: `-throttle`). It stays at the last value when the grip is released.
- Right-hand grip + wrist rotation controls flight-stick **roll** (`ABS_X`), **pitch** (`ABS_Y`) and **yaw** (`ABS_RX`). These axes recenter when the grip is released.
- The **right trigger is a dedicated keyboard modifier**, not an ordinary RT/gamepad analog trigger.
- While right trigger is held, the right thumbstick directions send **keyboard arrow keys**, and right thumbstick click sends **M**.
- A separate virtual keyboard device, **`XR HOTAS Keyboard`**, is created in addition to the gamepad, **`XR Fake-HOTAS`**.
- Unlike the other-games branch, this branch does **not** forward either Quest trigger as an analog gamepad trigger axis. In particular, `ABS_Z` is reserved for hand-motion throttle here.

## Motion controls

| Meta Quest 3 Touch input | Emulated output | Behavior |
| --- | --- | --- |
| Hold **left grip**, move hand forward/backward | `ABS_Z = -throttle` | Accumulated throttle; remains in place on release |
| Hold **right grip**, rotate wrist for roll | `ABS_X = -roll` | Recenters on release |
| Hold **right grip**, rotate wrist for pitch | `ABS_Y = pitch` | Recenters on release |
| Hold **right grip**, rotate wrist for yaw | `ABS_RX = -yaw` | Recenters on release |

The axis names are Linux input codes; depending on Steam Input/Proton and the game, they may appear under different controller labels. Bind/check the axes in Nuclear Option rather than assuming a particular aircraft action is assigned.

## Normal buttons (right trigger **not** held)

| Quest input | Emulated gamepad button |
| --- | --- |
| Left X / Y | X / Y |
| Right A / B | A / B |
| Left thumbstick up / down / left / right | D-pad up / down / left / right |
| Left thumbstick click | Left stick click (L3) |
| Right thumbstick up | Back / Select |
| Right thumbstick down | Start |
| Right thumbstick left | Left bumper (LB) |
| Right thumbstick right | Right bumper (RB) |
| Right thumbstick click | Right stick click (R3) |

The grip buttons engage virtual throttle/stick movement; they are not spare gamepad buttons. The physical **right trigger** is reserved for the modifier below. The Quest left trigger does not have an assigned analog gamepad output in this branch.

## Keyboard modifier: hold the **right Quest trigger**

| While holding right trigger… | Keyboard output |
| --- | --- |
| Right thumbstick up | Up arrow (`KEY_UP`) |
| Right thumbstick down | Down arrow (`KEY_DOWN`) |
| Right thumbstick left | Left arrow (`KEY_LEFT`) |
| Right thumbstick right | Right arrow (`KEY_RIGHT`) |
| Right thumbstick click | M (`KEY_M`) |

When this layer is active, these inputs **do not also press their normal gamepad buttons**. Releasing the trigger releases modifier-generated keys. If you release the trigger while still holding a thumbstick control, its original gamepad button remains suppressed until you release that control; this avoids accidental inputs.

The modifier activates at approximately 10% physical trigger travel and deactivates below approximately 5% (hysteresis). The five keyboard outputs only perform actions that you actually bind to those keys inside the game.

## Sensitivity settings used in our setup

```sh
~/.local/bin/xr-hotas-nuclear-option --max-distance 0.08 --max-angle 40
```

- `--max-distance 0.08`: 0.08 m / **8 cm** of accumulated left-hand movement spans the full throttle range (in either direction across the normalized range).
- `--max-angle 40`: about **40 degrees** of wrist rotation produces maximum deflection for an individual stick axis.
- Program defaults without flags are 0.2 m and 60 degrees.

### Binding/calibration mode

Send `SIGUSR1` to the running XR-HOTAS process to toggle **one-axis-at-a-time binding mode**. This suppresses weaker stick axes while binding inputs; it is not intended for normal flight. Toggle again to disable it.

```sh
# Find the running process and note its PID.
pgrep -af xr-hotas

# Replace 12345 with the correct PID from the result.
kill -USR1 12345
```

## Nuclear Option — Steam launch options

These are the **recorded launch options from our Fedora/WiVRn + Proton + NOVR/OpenComposite setup**. Check the currently configured Steam launch options before replacing them: runtime paths and required VR-patch overrides can change with your installation.

Open **Steam → Library → Nuclear Option → Properties → General → Launch Options**, then use:

```sh
sh -c '~/.local/bin/xr-hotas-nuclear-option --max-distance 0.08 --max-angle 40 & xrpid=$!; trap "kill $xrpid 2>/dev/null; wait $xrpid 2>/dev/null" EXIT INT TERM; sleep 1; WINEDLLOVERRIDES="winhttp=n,b" VR_OVERRIDE=/run/host/usr/lib64/opencomposite/runtime OXR_RECENTER_STAGE=1 "$@"; rc=$?; exit $rc' sh %command%
```

The command intentionally uses `sh -c`, even if your interactive shell is **fish**. `%command%` is substituted by Steam.

| Part | Purpose |
| --- | --- |
| `xr-hotas-nuclear-option --max-distance 0.08 --max-angle 40` | Start our custom HOTAS in the background with our sensitivity settings |
| `xrpid=$!` and `trap ...` | Track and stop XR-HOTAS when the Steam game wrapper exits |
| `sleep 1` | Give XR-HOTAS a brief startup interval |
| `WINEDLLOVERRIDES="winhttp=n,b"` | Load the native `winhttp` DLL used by our BepInEx/NOVR setup |
| `VR_OVERRIDE=/run/host/usr/lib64/opencomposite/runtime` | Use this OpenComposite runtime path inside the recorded Steam environment |
| `OXR_RECENTER_STAGE=1` | Enable the stage-recentering behavior used in our setup |
| `"$@"` / `%command%` | Forward and run Steam's game command |

**VR and input are separate:** WiVRn/OpenXR + the game's VR mod/OpenComposite provide the VR experience; XR-HOTAS only supplies simulated flight-controller input. Updating XR-HOTAS does not update the game's VR mod.

## Install/update these custom builds

XR-HOTAS is written in Rust. If using our existing Fedora Distrobox environment, rebuild the **correct** branch, then copy its output to the dedicated executable name:

```sh
# On Fedora, enter the existing build container.
distrobox enter xr-hotas-build-f44

# Inside Distrobox: build exactly this branch.
cargo install --locked --force --git https://github.com/eran100/xr-hotas.git --branch eran-nucler-option --root ~/build-nuclear-option

# Leave the container.
exit

# On the Fedora host: back up the installed executable before replacing it.
cp ~/.local/bin/xr-hotas-nuclear-option ~/.local/bin/xr-hotas-nuclear-option.bak

# Install the newly compiled binary for the current user.
install -m 755 ~/.local/share/distrobox-homes/xr-hotas-f44/build-nuclear-option/bin/xr-hotas ~/.local/bin/xr-hotas-nuclear-option
```

Make sure the game/XR-HOTAS is stopped before replacing its executable. The build container/path above is **specific to our Fedora installation**; adapt it on other systems. `cargo install` installs the code currently on this branch, but it does **not** automatically incorporate later upstream changes.

To confirm the installed executable matches the build output:

```sh
sha256sum ~/.local/bin/xr-hotas-nuclear-option ~/.local/share/distrobox-homes/xr-hotas-f44/build-nuclear-option/bin/xr-hotas
cat ~/.local/share/distrobox-homes/xr-hotas-f44/build-nuclear-option/.crates.toml
```

The checksums should match. Cargo's metadata includes the Git commit used for the build.

## Original project and Linux VR community

Thanks to [galister's XR-HOTAS](https://github.com/galister/xr-hotas) for the original project. For additional VR controls, a [WayVR](https://github.com/wlx-team/wayvr) panel can provide access to switches/buttons that are impractical on motion controllers.

- [Linux VR Discord](https://discord.gg/gHwJ2vwSWV)
- Matrix: `#linux-vr-adventures:matrix.org`
