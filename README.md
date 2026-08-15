# XR HOTAS

Allows you to use your motion controllers as throttle & joystick in games that don't support motion controllers but do support gamepad input.

Example titles that work with this tool:
- Elite: Dangerous
- DCS: World

## Installing

Install via cargo:

```bash
cargo install --git https://github.com/galister/xr-hotas.git
```

## How to use

Start the program. If `xr-hotas` is not found, try `~/.cargo/bin/xr-hotas`.

Holding the grip on the controllers will activate stick input:
  - Left controller: Grab and move forward/backwards to move your throttle. The throttle stays in place when released.
  - Right controller: Grab and rotate your wrist to move the joystick. The joystick recenters when released.

There is a binding mode that will only send one axis at a time - the one that is moving the most.

Activate/deactivate binding mode: `killall -USR1 xr-hotas`

Button layout:
- Triggers → LT/RT
- AB → AB, XY → XY (XY on Index Controllers are left hand AB)
- Left stick → D-Pad
- Right stick: Left-right → LB, RB, Up-down: Select, Start

## Join the Linux VR Community

We are available on either:

- Discord: <https://discord.gg/gHwJ2vwSWV>
- Matrix Space: `#linux-vr-adventures:matrix.org`
