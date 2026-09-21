use std::{
    f32,
    process::ExitCode,
    sync::atomic::{AtomicBool, Ordering},
};

use clap::Parser;
use glam::{Quat, Vec3};
use signal_hook::{
    consts::{SIGINT, SIGTERM, SIGUSR1},
    iterator::Signals,
};

use crate::{
    helpers_xr::XrState,
    hotas_uinput::{Hotas, ABS_RX, ABS_RY, ABS_RZ, ABS_X, ABS_Y, ABS_Z},
};

mod helpers_xr;
mod hotas_uinput;

static RUNNING: AtomicBool = AtomicBool::new(true);
static BINDING: AtomicBool = AtomicBool::new(false);

#[allow(clippy::single_match_else)]
fn setup_signal_hooks() -> anyhow::Result<()> {
    let mut signals = Signals::new([SIGINT, SIGTERM, SIGUSR1])?;

    std::thread::spawn(move || {
        for signal in signals.forever() {
            match signal {
                SIGUSR1 => {
                    let now = BINDING.fetch_not(Ordering::Relaxed);
                    log::warn!("SIGUSR1 received (binding mode: {})", !now);
                }
                _ => {
                    RUNNING.store(false, Ordering::Relaxed);
                    break;
                }
            }
        }
    });
    Ok(())
}

fn main() -> ExitCode {
    env_logger::init();
    let args = Args::parse();

    if let Err(e) = main_inner(args) {
        log::error!("{e:?}");
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn main_inner(args: Args) -> anyhow::Result<()> {
    setup_signal_hooks()?;

    let mut xr = XrState::new()?;

    let max_angle_rad = args.max_angle.to_radians();

    let triggers = [ABS_Z, ABS_RZ];
    let mut operations: [Operation; 2] = [Operation::NoThrottle, Operation::NoStick];
    let mut hotas = Hotas::new()?;
    let mut throttle = 0.0;

    while RUNNING.load(Ordering::Relaxed) {
        let binding_mode = BINDING.load(Ordering::Relaxed);

        xr.tick()?;

        for (hand_idx, operation) in operations.iter_mut().enumerate() {
            let controller = &mut xr.controllers[hand_idx];

            hotas.set_axis(triggers[hand_idx], controller.trigger)?;

            for btn in controller.buttons.iter() {
                hotas.set_button(btn.code, btn.now_active)?;
            }

            match *operation {
                Operation::NoThrottle => {
                    if controller.grip_active {
                        *operation = Operation::Throttle(controller.position);
                        log::info!("Throttle grabbed at {throttle}.");
                    }
                }

                Operation::Throttle(last_pos) => {
                    if !controller.grip_active {
                        *operation = Operation::NoThrottle;
                        log::info!("Throttle released at {throttle}.");
                        continue;
                    }
                    let frame_delta_z = (controller.position.z - last_pos.z) / args.max_distance;

                    throttle = (throttle + frame_delta_z).clamp(-1.0, 1.0);

                    // next calculation relative to this frame
                    *operation = Operation::Throttle(controller.position);

                    hotas.set_axis(ABS_RY, throttle)?;
                }
                Operation::NoStick => {
                    if controller.grip_active {
                        *operation = Operation::Stick(controller.orientation);
                        log::info!("Stick grabbed.");
                    }
                }
                Operation::Stick(start_orientation) => {
                    if !controller.grip_active {
                        hotas.set_axis(ABS_X, 0.0)?;
                        hotas.set_axis(ABS_Y, 0.0)?;
                        hotas.set_axis(ABS_RX, 0.0)?;
                        *operation = Operation::NoStick;
                        log::info!("Stick released.");
                        continue;
                    }

                    let relative_rot =
                        start_orientation.inverse() * controller.orientation.normalize();
                    let (yaw, pitch, roll) = relative_rot.to_euler(glam::EulerRot::YXZ);
                    let mut relative_pyr =
                        (Vec3::new(pitch, yaw, roll) / max_angle_rad).clamp(-Vec3::ONE, Vec3::ONE);

                    apply_binding_mode(&mut relative_pyr, binding_mode);

                    hotas.set_axis(ABS_X, -relative_pyr.z)?;
                    hotas.set_axis(ABS_Y, relative_pyr.x)?;
                    hotas.set_axis(ABS_RX, -relative_pyr.y)?;
                }
            }
        }
    }

    Ok(())
}

fn apply_binding_mode(values: &mut Vec3, binding_mode: bool) {
    if binding_mode {
        let signals = [values.x, values.y, values.z];

        let strongest = signals
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.abs().total_cmp(&b.abs()))
            .map(|(index, _)| index)
            .unwrap();

        let original_values = *values;

        *values = Vec3::ZERO;

        if signals[strongest].abs() > 0.5 {
            match strongest {
                0 => values.x = original_values.x,
                1 => values.y = original_values.y,
                2 => values.z = original_values.z,
                _ => unreachable!(),
            }
        }
    }
}

#[derive(Clone, Copy)]
enum Operation {
    Throttle(Vec3),
    Stick(Quat),
    NoThrottle,
    NoStick,
}

/// Emulate a HOTAS using your VR motion controllers!
#[derive(clap::Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Distance for maximum deflection
    #[arg(long, default_value = "0.2")]
    max_distance: f32,

    /// Angle for maximum deflection (degrees)
    #[arg(long, default_value = "60")]
    max_angle: f32,
}
