use std::{
    thread,
    time::{Duration, Instant},
};

use anyhow::Context;
use glam::{Quat, Vec3};
use openxr::{self as xr, SpaceLocationFlags};

use crate::hotas_uinput::{
    BTN_DPAD_DOWN, BTN_DPAD_LEFT, BTN_DPAD_RIGHT, BTN_DPAD_UP, BTN_EAST, BTN_NORTH, BTN_SELECT,
    BTN_SOUTH, BTN_START, BTN_THUMBL, BTN_THUMBR, BTN_TL, BTN_TL2, BTN_TR, BTN_TR2, BTN_WEST,
};

const BOOL_THRESHOLD_LO: f32 = 0.6;
const BOOL_THRESHOLD_HI: f32 = 0.4;

fn xr_init() -> anyhow::Result<(xr::Instance, xr::SystemId)> {
    let entry = xr::Entry::linked();

    let available_extensions = entry
        .enumerate_extensions()
        .context("Failed to enumerate XR extensions")?;

    if !available_extensions.mnd_headless {
        anyhow::bail!("Missing MND_headless extension!")
    }

    let mut enabled_extensions = xr::ExtensionSet::default();
    enabled_extensions.mnd_headless = true;
    enabled_extensions.khr_convert_timespec_time = true;

    if available_extensions.ext_hand_tracking {
        enabled_extensions.ext_hand_tracking = true;
    }

    let instance = entry
        .create_instance(
            &xr::ApplicationInfo {
                api_version: xr::Version::new(1, 0, 0),
                application_name: "xr-hotas",
                application_version: 0,
                engine_name: "xr-hotas",
                engine_version: 0,
            },
            &enabled_extensions,
            &[],
        )
        .context("Failed to create OpenXR instance")?;

    let instance_props = instance
        .properties()
        .context("Failed to query OpenXR instance properties")?;
    log::info!(
        "Using OpenXR runtime: {} {}",
        instance_props.runtime_name,
        instance_props.runtime_version
    );

    let system = instance
        .system(xr::FormFactor::HEAD_MOUNTED_DISPLAY)
        .context("Failed to access OpenXR HMD system")?;

    Ok((instance, system))
}

pub enum DpadDirection {
    North,
    East,
    South,
    West,
}

pub enum ButtonAction {
    Boolean(xr::Action<bool>),
    Float(xr::Action<f32>),
    Dpad(xr::Action<f32>, DpadDirection),
}

pub struct XrButton {
    actions: Box<[ButtonAction]>,
    pub now_active: bool,
    pub was_active: bool,
    pub code: u16,
}

impl XrButton {
    fn new_bool_or_float(
        side: &str,
        code: u16,
        name: &str,
        actions: &xr::ActionSet,
    ) -> anyhow::Result<Self> {
        let bool_name = format!("{side}_{name}_bool");
        let bool_action = actions.create_action(&bool_name, &bool_name, &[])?;

        let float_name = format!("{side}_{name}_float");
        let float_action = actions.create_action(&float_name, &float_name, &[])?;

        Ok(Self {
            actions: [
                ButtonAction::Boolean(bool_action),
                ButtonAction::Float(float_action),
            ]
            .into(),
            code,
            now_active: false,
            was_active: false,
        })
    }

    fn new_dpad_from_vec2f(
        side: &str,
        code: [u16; 4],
        name: &str,
        actions: &xr::ActionSet,
    ) -> anyhow::Result<Vec<Self>> {
        let name_x = format!("{side}_{name}_x");
        let name_y = format!("{side}_{name}_y");
        let actions = [
            actions.create_action(&name_y, &name_y, &[])?,
            actions.create_action(&name_x, &name_x, &[])?,
        ];

        Ok([
            DpadDirection::North,
            DpadDirection::East,
            DpadDirection::South,
            DpadDirection::West,
        ]
        .into_iter()
        .enumerate()
        .map(|(i, dir)| Self {
            actions: [ButtonAction::Dpad(actions[i % 2].clone(), dir)].into(),
            code: code[i],
            now_active: false,
            was_active: false,
        })
        .collect())
    }

    fn bool_action(&self) -> Option<xr::Action<bool>> {
        self.actions.iter().find_map(|x| match x {
            ButtonAction::Boolean(a) => Some(a.clone()),
            _ => None,
        })
    }

    fn float_action(&self) -> Option<xr::Action<f32>> {
        self.actions.iter().find_map(|x| match x {
            ButtonAction::Float(a) => Some(a.clone()),
            _ => None,
        })
    }

    fn dpad_action(&self) -> Option<xr::Action<f32>> {
        self.actions.iter().find_map(|x| match x {
            ButtonAction::Dpad(a, ..) => Some(a.clone()),
            _ => None,
        })
    }

    fn update<G>(&mut self, session: &xr::Session<G>) -> anyhow::Result<()> {
        self.was_active = self.now_active;
        let mut max_value = f32::MIN;

        for action in self.actions.iter() {
            let value = match action {
                ButtonAction::Boolean(action) => {
                    if action.state(session, xr::Path::NULL)?.current_state {
                        1.0
                    } else {
                        0.0
                    }
                }
                ButtonAction::Float(action) => action.state(session, xr::Path::NULL)?.current_state,
                ButtonAction::Dpad(action, dir) => {
                    let state = action.state(session, xr::Path::NULL)?.current_state;
                    match dir {
                        DpadDirection::North => state,
                        DpadDirection::East => state,
                        DpadDirection::South => -state,
                        DpadDirection::West => -state,
                    }
                }
            };

            max_value = max_value.max(value);
        }

        let threshold = if self.was_active {
            BOOL_THRESHOLD_HI
        } else {
            BOOL_THRESHOLD_LO
        };
        self.now_active = max_value >= threshold;

        Ok(())
    }
}

pub struct XrController {
    space: xr::Space,
    pose_action: xr::Action<xr::Posef>,
    grip_action: xr::Action<f32>,
    trigger_action: xr::Action<f32>,

    pub position: Vec3,
    pub orientation: Quat,

    pub grip_active: bool,
    grip_before: bool,

    pub trigger: f32,

    pub buttons: Box<[XrButton]>,
}

impl XrController {
    pub const T2: usize = 0; // trigger click
    pub const A: usize = 1; // a or x
    pub const B: usize = 2; // b or y
    pub const THUMB: usize = 3; // stick depress
    pub const DPAD_Y: usize = 4;
    pub const DPAD_X: usize = 5;

    fn new_left<G>(session: &xr::Session<G>, actions: &xr::ActionSet) -> anyhow::Result<Self> {
        let aim_name = "left_aim";
        let grip_name = "left_grip";
        let trigger_name = "left_trigger";

        let pose_action = actions.create_action(aim_name, aim_name, &[])?;
        let grip_action = actions.create_action(grip_name, grip_name, &[])?;
        let trigger_action = actions.create_action(trigger_name, trigger_name, &[])?;

        let space = pose_action.create_space(session, xr::Path::NULL, xr::Posef::IDENTITY)?;

        let side = "left";

        let mut buttons = vec![
            XrButton::new_bool_or_float(side, BTN_TR2, "trigger_2nd", actions)?,
            XrButton::new_bool_or_float(side, BTN_WEST, "x", actions)?,
            XrButton::new_bool_or_float(side, BTN_NORTH, "y", actions)?,
            XrButton::new_bool_or_float(side, BTN_THUMBL, "stick_depress", actions)?,
        ];

        buttons.extend(XrButton::new_dpad_from_vec2f(
            side,
            [BTN_DPAD_UP, BTN_DPAD_RIGHT, BTN_DPAD_DOWN, BTN_DPAD_LEFT],
            "stick_dpad",
            actions,
        )?);

        Ok(Self {
            space,
            pose_action,
            grip_action,
            trigger_action,
            trigger: 0.0,
            grip_active: false,
            grip_before: false,
            position: Vec3::ZERO,
            orientation: Quat::IDENTITY,
            buttons: buttons.into_boxed_slice(),
        })
    }

    fn new_right<G>(session: &xr::Session<G>, actions: &xr::ActionSet) -> anyhow::Result<Self> {
        let aim_name = "right_aim";
        let grip_name = "right_grip";
        let trigger_name = "right_trigger";

        let pose_action = actions.create_action(aim_name, aim_name, &[])?;
        let grip_action = actions.create_action(grip_name, grip_name, &[])?;
        let trigger_action = actions.create_action(trigger_name, trigger_name, &[])?;

        let space = pose_action.create_space(session, xr::Path::NULL, xr::Posef::IDENTITY)?;

        let side = "right";

        let mut buttons = vec![
            XrButton::new_bool_or_float(side, BTN_TL2, "trigger_2nd", actions)?,
            XrButton::new_bool_or_float(side, BTN_SOUTH, "a", actions)?,
            XrButton::new_bool_or_float(side, BTN_EAST, "b", actions)?,
            XrButton::new_bool_or_float(side, BTN_THUMBR, "stick_depress", actions)?,
        ];

        buttons.extend(XrButton::new_dpad_from_vec2f(
            side,
            [BTN_SELECT, BTN_TR, BTN_START, BTN_TL],
            "stick_dpad",
            actions,
        )?);

        Ok(Self {
            space,
            pose_action,
            grip_action,
            trigger_action,
            trigger: 0.0,
            grip_active: false,
            grip_before: false,
            position: Vec3::ZERO,
            orientation: Quat::IDENTITY,
            buttons: buttons.into_boxed_slice(),
        })
    }

    fn update<G>(
        &mut self,
        session: &xr::Session<G>,
        space: &xr::Space,
        time: xr::Time,
    ) -> anyhow::Result<()> {
        let loc = self.space.locate(space, time)?;

        if loc
            .location_flags
            .contains(SpaceLocationFlags::ORIENTATION_VALID)
        {
            self.orientation =
                glam::Quat::from(mint::Quaternion::<f32>::from(loc.pose.orientation));
        }

        if loc
            .location_flags
            .contains(SpaceLocationFlags::POSITION_VALID)
        {
            self.position = glam::Vec3::from(mint::Vector3::<f32>::from(loc.pose.position));
        }

        self.trigger = self
            .trigger_action
            .state(session, xr::Path::NULL)?
            .current_state;

        self.grip_before = self.grip_active;

        let threshold = if self.grip_before {
            BOOL_THRESHOLD_HI
        } else {
            BOOL_THRESHOLD_LO
        };
        self.grip_active = self
            .grip_action
            .state(session, xr::Path::NULL)?
            .current_state
            >= threshold;

        for button in self.buttons.iter_mut() {
            button.update(session)?;
        }
        Ok(())
    }
}

pub struct XrState {
    instance: xr::Instance,
    session: xr::Session<xr::Headless>,
    stage_space: xr::Space,
    actions: xr::ActionSet,
    events: xr::EventDataBuffer,
    session_running: bool,
    pub controllers: [XrController; 2],
    next_frame: Instant,
}

impl XrState {
    pub fn new() -> anyhow::Result<Self> {
        let (instance, system) = xr_init()?;

        let actions = instance.create_action_set("xr-hotas", "xr-hotas", 0)?;

        let (session, _frame_waiter, _frame_stream) =
            unsafe { instance.create_session(system, &xr::headless::SessionCreateInfo {})? };

        let stage_space =
            session.create_reference_space(xr::ReferenceSpaceType::STAGE, xr::Posef::IDENTITY)?;

        let controllers = [
            XrController::new_left(&session, &actions)?,
            XrController::new_right(&session, &actions)?,
        ];

        let me = Self {
            instance,
            session,
            stage_space,
            actions,
            events: xr::EventDataBuffer::new(),
            session_running: false,
            controllers,
            next_frame: Instant::now(),
        };

        me.suggest_bindings()?;
        me.session.attach_action_sets(&[&me.actions])?;

        Ok(me)
    }

    pub fn tick(&mut self) -> anyhow::Result<()> {
        while let Some(event) = self.instance.poll_event(&mut self.events)? {
            use xr::Event::*;
            match event {
                SessionStateChanged(e) => match e.state() {
                    xr::SessionState::READY => {
                        self.session
                            .begin(xr::ViewConfigurationType::PRIMARY_STEREO)?;
                        self.session_running = true;
                        log::info!("XrSession started.")
                    }
                    xr::SessionState::STOPPING => {
                        self.session.end()?;
                        self.session_running = false;
                        log::warn!("XrSession stopped.")
                    }
                    xr::SessionState::EXITING | xr::SessionState::LOSS_PENDING => {
                        anyhow::bail!("XR session exiting");
                    }
                    _ => {}
                },
                InstanceLossPending(_) => {
                    anyhow::bail!("XR instance loss pending");
                }
                EventsLost(e) => {
                    log::warn!("lost {} events", e.lost_event_count());
                }
                _ => {}
            }
        }

        thread::sleep(self.next_frame.duration_since(Instant::now()));
        self.next_frame = Instant::now() + Duration::from_millis(5);

        if !self.session_running {
            return Ok(());
        }

        self.session.sync_actions(&[(&self.actions).into()])?;

        for c in self.controllers.iter_mut() {
            c.update(&self.session, &self.stage_space, self.instance.now()?)?;
        }

        Ok(())
    }

    fn suggest_bindings(&self) -> anyhow::Result<()> {
        // this is super cursed but needs to all be here because of fukken borrow checker crap

        let _ = self
            .instance
            .suggest_interaction_profile_bindings(
                self.instance
                    .string_to_path("/interaction_profiles/oculus/touch_controller")?,
                &[
                    // Left hand
                    xr::Binding::new(
                        &self.controllers[0].pose_action,
                        self.instance
                            .string_to_path("/user/hand/left/input/aim/pose")?,
                    ),
                    xr::Binding::new(
                        &self.controllers[0].grip_action,
                        self.instance
                            .string_to_path("/user/hand/left/input/squeeze/value")?,
                    ),
                    xr::Binding::new(
                        &self.controllers[0].trigger_action,
                        self.instance
                            .string_to_path("/user/hand/left/input/trigger/value")?,
                    ),
                    xr::Binding::new(
                        &self.controllers[0].buttons[XrController::A]
                            .bool_action()
                            .unwrap(),
                        self.instance
                            .string_to_path("/user/hand/left/input/x/click")?,
                    ),
                    xr::Binding::new(
                        &self.controllers[0].buttons[XrController::B]
                            .bool_action()
                            .unwrap(),
                        self.instance
                            .string_to_path("/user/hand/left/input/y/click")?,
                    ),
                    xr::Binding::new(
                        &self.controllers[0].buttons[XrController::THUMB]
                            .bool_action()
                            .unwrap(),
                        self.instance
                            .string_to_path("/user/hand/left/input/thumbstick/click")?,
                    ),
                    xr::Binding::new(
                        &self.controllers[0].buttons[XrController::DPAD_Y]
                            .dpad_action()
                            .unwrap(),
                        self.instance
                            .string_to_path("/user/hand/left/input/thumbstick/y")?,
                    ),
                    xr::Binding::new(
                        &self.controllers[0].buttons[XrController::DPAD_X]
                            .dpad_action()
                            .unwrap(),
                        self.instance
                            .string_to_path("/user/hand/left/input/thumbstick/x")?,
                    ),
                    // Right hand
                    xr::Binding::new(
                        &self.controllers[1].pose_action,
                        self.instance
                            .string_to_path("/user/hand/right/input/aim/pose")?,
                    ),
                    xr::Binding::new(
                        &self.controllers[1].grip_action,
                        self.instance
                            .string_to_path("/user/hand/right/input/squeeze/value")?,
                    ),
                    xr::Binding::new(
                        &self.controllers[1].trigger_action,
                        self.instance
                            .string_to_path("/user/hand/right/input/trigger/value")?,
                    ),
                    xr::Binding::new(
                        &self.controllers[1].buttons[XrController::A]
                            .bool_action()
                            .unwrap(),
                        self.instance
                            .string_to_path("/user/hand/right/input/a/click")?,
                    ),
                    xr::Binding::new(
                        &self.controllers[1].buttons[XrController::B]
                            .bool_action()
                            .unwrap(),
                        self.instance
                            .string_to_path("/user/hand/right/input/b/click")?,
                    ),
                    xr::Binding::new(
                        &self.controllers[1].buttons[XrController::THUMB]
                            .bool_action()
                            .unwrap(),
                        self.instance
                            .string_to_path("/user/hand/right/input/thumbstick/click")?,
                    ),
                    xr::Binding::new(
                        &self.controllers[1].buttons[XrController::DPAD_Y]
                            .dpad_action()
                            .unwrap(),
                        self.instance
                            .string_to_path("/user/hand/right/input/thumbstick/y")?,
                    ),
                    xr::Binding::new(
                        &self.controllers[1].buttons[XrController::DPAD_X]
                            .dpad_action()
                            .unwrap(),
                        self.instance
                            .string_to_path("/user/hand/right/input/thumbstick/x")?,
                    ),
                ],
            )
            .inspect_err(|e| log::warn!("Could not bind oculus/touch_controller: {e:?}"));

        let _ = self
            .instance
            .suggest_interaction_profile_bindings(
                self.instance
                    .string_to_path("/interaction_profiles/valve/index_controller")?,
                &[
                    // Left hand
                    xr::Binding::new(
                        &self.controllers[0].pose_action,
                        self.instance
                            .string_to_path("/user/hand/left/input/aim/pose")?,
                    ),
                    xr::Binding::new(
                        &self.controllers[0].grip_action,
                        self.instance
                            .string_to_path("/user/hand/left/input/squeeze/value")?,
                    ),
                    xr::Binding::new(
                        &self.controllers[0].trigger_action,
                        self.instance
                            .string_to_path("/user/hand/left/input/trigger/value")?,
                    ),
                    xr::Binding::new(
                        &self.controllers[0].buttons[XrController::T2]
                            .bool_action()
                            .unwrap(),
                        self.instance
                            .string_to_path("/user/hand/left/input/trigger/click")?,
                    ),
                    xr::Binding::new(
                        &self.controllers[0].buttons[XrController::A]
                            .bool_action()
                            .unwrap(),
                        self.instance
                            .string_to_path("/user/hand/left/input/a/click")?,
                    ),
                    xr::Binding::new(
                        &self.controllers[0].buttons[XrController::B]
                            .bool_action()
                            .unwrap(),
                        self.instance
                            .string_to_path("/user/hand/left/input/b/click")?,
                    ),
                    xr::Binding::new(
                        &self.controllers[0].buttons[XrController::THUMB]
                            .bool_action()
                            .unwrap(),
                        self.instance
                            .string_to_path("/user/hand/left/input/thumbstick/click")?,
                    ),
                    xr::Binding::new(
                        &self.controllers[0].buttons[XrController::DPAD_Y]
                            .dpad_action()
                            .unwrap(),
                        self.instance
                            .string_to_path("/user/hand/left/input/thumbstick/y")?,
                    ),
                    xr::Binding::new(
                        &self.controllers[0].buttons[XrController::DPAD_X]
                            .dpad_action()
                            .unwrap(),
                        self.instance
                            .string_to_path("/user/hand/left/input/thumbstick/x")?,
                    ),
                    // Right hand
                    xr::Binding::new(
                        &self.controllers[1].pose_action,
                        self.instance
                            .string_to_path("/user/hand/right/input/aim/pose")?,
                    ),
                    xr::Binding::new(
                        &self.controllers[1].grip_action,
                        self.instance
                            .string_to_path("/user/hand/right/input/squeeze/value")?,
                    ),
                    xr::Binding::new(
                        &self.controllers[1].trigger_action,
                        self.instance
                            .string_to_path("/user/hand/right/input/trigger/value")?,
                    ),
                    xr::Binding::new(
                        &self.controllers[1].buttons[XrController::T2]
                            .bool_action()
                            .unwrap(),
                        self.instance
                            .string_to_path("/user/hand/right/input/trigger/click")?,
                    ),
                    xr::Binding::new(
                        &self.controllers[1].buttons[XrController::A]
                            .bool_action()
                            .unwrap(),
                        self.instance
                            .string_to_path("/user/hand/right/input/a/click")?,
                    ),
                    xr::Binding::new(
                        &self.controllers[1].buttons[XrController::B]
                            .bool_action()
                            .unwrap(),
                        self.instance
                            .string_to_path("/user/hand/right/input/b/click")?,
                    ),
                    xr::Binding::new(
                        &self.controllers[1].buttons[XrController::THUMB]
                            .bool_action()
                            .unwrap(),
                        self.instance
                            .string_to_path("/user/hand/right/input/thumbstick/click")?,
                    ),
                    xr::Binding::new(
                        &self.controllers[1].buttons[XrController::DPAD_Y]
                            .dpad_action()
                            .unwrap(),
                        self.instance
                            .string_to_path("/user/hand/right/input/thumbstick/y")?,
                    ),
                    xr::Binding::new(
                        &self.controllers[1].buttons[XrController::DPAD_X]
                            .dpad_action()
                            .unwrap(),
                        self.instance
                            .string_to_path("/user/hand/right/input/thumbstick/x")?,
                    ),
                ],
            )
            .inspect_err(|e| log::warn!("Could not bind valve/index_controller: {e:?}"));

        Ok(())
    }
}
