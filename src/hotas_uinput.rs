use std::collections::HashSet;
use std::ffi::c_char;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::mem::{size_of, size_of_val};
use std::os::fd::AsRawFd;
use std::os::raw::{c_int, c_long, c_ulong};
use std::slice;

const EV_SYN: u16 = 0x00;
const EV_KEY: u16 = 0x01;
const EV_ABS: u16 = 0x03;
const SYN_REPORT: u16 = 0;

// left stick
pub const ABS_X: u16 = 0x00;
pub const ABS_Y: u16 = 0x01;

// right stick
pub const ABS_RX: u16 = 0x03;
pub const ABS_RY: u16 = 0x04;

// analog triggers - unipolar
pub const ABS_Z: u16 = 0x02;
pub const ABS_RZ: u16 = 0x05;

pub const BTN_SOUTH: u16 = 0x130; // Xbox A
pub const BTN_EAST: u16 = 0x131; // Xbox B
pub const BTN_NORTH: u16 = 0x133; // Xbox Y
pub const BTN_WEST: u16 = 0x134; // Xbox X
pub const BTN_TL: u16 = 0x136; // left bumper
pub const BTN_TR: u16 = 0x137; // right bumper
pub const BTN_TL2: u16 = 0x138; // left trigger button
pub const BTN_TR2: u16 = 0x139; // right trigger button
pub const BTN_SELECT: u16 = 0x13a;
pub const BTN_START: u16 = 0x13b;
pub const BTN_THUMBL: u16 = 0x13d; // left stick click
pub const BTN_THUMBR: u16 = 0x13e; // right stick click
pub const BTN_DPAD_UP: u16 = 0x220;
pub const BTN_DPAD_DOWN: u16 = 0x221;
pub const BTN_DPAD_LEFT: u16 = 0x222;
pub const BTN_DPAD_RIGHT: u16 = 0x223;

// keyboard keys used by the right-trigger modifier layer
pub const KEY_M: u16 = 50;
pub const KEY_UP: u16 = 103;
pub const KEY_LEFT: u16 = 105;
pub const KEY_RIGHT: u16 = 106;
pub const KEY_DOWN: u16 = 108;

const KEY_ALL: &[u16] = &[KEY_M, KEY_UP, KEY_LEFT, KEY_RIGHT, KEY_DOWN];

const BTN_ALL: &[u16] = &[
    BTN_SOUTH,
    BTN_EAST,
    BTN_NORTH,
    BTN_WEST,
    BTN_TL,
    BTN_TR,
    BTN_TL2,
    BTN_TR2,
    BTN_SELECT,
    BTN_START,
    BTN_THUMBL,
    BTN_THUMBR,
    BTN_DPAD_UP,
    BTN_DPAD_RIGHT,
    BTN_DPAD_DOWN,
    BTN_DPAD_LEFT,
];

const BUS_VIRTUAL: u16 = 0x06;
const UINPUT_MAX_NAME_SIZE: usize = 80;
const AXIS_LEVELS: i32 = 65_536;
const AXIS_MIN: i32 = -(AXIS_LEVELS / 2);
const AXIS_MAX: i32 = AXIS_MIN + AXIS_LEVELS - 1;

// verified for amd64 and aarch64
const IOC_NRBITS: u32 = 8;
const IOC_TYPEBITS: u32 = 8;
const IOC_SIZEBITS: u32 = 14;
const IOC_NRSHIFT: u32 = 0;
const IOC_TYPESHIFT: u32 = IOC_NRSHIFT + IOC_NRBITS;
const IOC_SIZESHIFT: u32 = IOC_TYPESHIFT + IOC_TYPEBITS;
const IOC_DIRSHIFT: u32 = IOC_SIZESHIFT + IOC_SIZEBITS;
const IOC_NONE: u32 = 0;
const IOC_WRITE: u32 = 1;
const UINPUT_IOCTL_BASE: u8 = b'U';

const fn ioc(direction: u32, kind: u8, number: u8, size: usize) -> c_ulong {
    ((direction as c_ulong) << IOC_DIRSHIFT)
        | ((kind as c_ulong) << IOC_TYPESHIFT)
        | ((number as c_ulong) << IOC_NRSHIFT)
        | ((size as c_ulong) << IOC_SIZESHIFT)
}

const fn io(kind: u8, number: u8) -> c_ulong {
    ioc(IOC_NONE, kind, number, 0)
}

const fn iow(kind: u8, number: u8, size: usize) -> c_ulong {
    ioc(IOC_WRITE, kind, number, size)
}

#[repr(C)]
#[derive(Clone, Copy)]
struct InputId {
    bustype: u16,
    vendor: u16,
    product: u16,
    version: u16,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct InputAbsInfo {
    value: i32,
    minimum: i32,
    maximum: i32,
    fuzz: i32,
    flat: i32,
    resolution: i32,
}

#[repr(C)]
struct UinputSetup {
    id: InputId,
    name: [u8; UINPUT_MAX_NAME_SIZE],
    ff_effects_max: u32,
}

#[repr(C)]
struct UinputAbsSetup {
    code: u16,
    absinfo: InputAbsInfo,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct TimeVal {
    tv_sec: c_long,
    tv_usec: c_long,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct InputEvent {
    time: TimeVal,
    event_type: u16,
    code: u16,
    value: i32,
}

const UI_DEV_CREATE: c_ulong = io(UINPUT_IOCTL_BASE, 1);
const UI_DEV_DESTROY: c_ulong = io(UINPUT_IOCTL_BASE, 2);
const UI_DEV_SETUP: c_ulong = iow(UINPUT_IOCTL_BASE, 3, size_of::<UinputSetup>());
const UI_ABS_SETUP: c_ulong = iow(UINPUT_IOCTL_BASE, 4, size_of::<UinputAbsSetup>());
const UI_SET_EVBIT: c_ulong = iow(UINPUT_IOCTL_BASE, 100, size_of::<c_int>());
const UI_SET_KEYBIT: c_ulong = iow(UINPUT_IOCTL_BASE, 101, size_of::<c_int>());
const UI_SET_ABSBIT: c_ulong = iow(UINPUT_IOCTL_BASE, 103, size_of::<c_int>());
const UI_SET_PHYS: c_ulong = 0x4008_556c;

unsafe extern "C" {
    fn ioctl(fd: c_int, request: c_ulong, ...) -> c_int;
}

pub struct Hotas {
    file: File,
}

pub struct Keyboard {
    file: File,
    pressed: HashSet<u16>,
}

impl Hotas {
    pub fn new() -> io::Result<Self> {
        let file = open_uinput()?;
        let fd = file.as_raw_fd();

        ioctl_int(fd, UI_SET_EVBIT, EV_ABS as c_int, "enable EV_ABS")?;
        setup_axis(fd, ABS_X, AXIS_MIN, AXIS_MAX, 0)?;
        setup_axis(fd, ABS_Y, AXIS_MIN, AXIS_MAX, 0)?;
        setup_axis(fd, ABS_Z, AXIS_MIN, AXIS_MAX, 0)?;
        setup_axis(fd, ABS_RX, AXIS_MIN, AXIS_MAX, 0)?;
        setup_axis(fd, ABS_RY, AXIS_MIN, AXIS_MAX, 0)?;
        setup_axis(fd, ABS_RZ, AXIS_MIN, AXIS_MAX, 0)?;

        ioctl_int(fd, UI_SET_EVBIT, EV_KEY as c_int, "enable EV_KEY")?;
        for code in BTN_ALL {
            ioctl_int(fd, UI_SET_KEYBIT, *code as c_int, "enable a HOTAS button")?;
        }

        let mut setup: UinputSetup = unsafe { std::mem::zeroed() };
        setup.id = InputId {
            bustype: BUS_VIRTUAL,
            vendor: 0x045e,  // microsoft
            product: 0x028e, // xbox 360 wired controller
            version: 0x0001,
        };
        let name = b"XR Fake-HOTAS\0";
        setup.name[..name.len()].copy_from_slice(name);

        let phys = b"xr-hotas\0";
        let rc = unsafe { ioctl(fd, UI_SET_PHYS, phys.as_ptr() as *const c_char) };

        if rc < 0 {
            return Err(io::Error::last_os_error());
        }

        ioctl_ptr(fd, UI_DEV_SETUP, &setup, "configure the HOTAS")?;
        ioctl_none(fd, UI_DEV_CREATE, "create the HOTAS")?;

        Ok(Self { file })
    }

    pub fn set_button(&mut self, button_code: u16, pressed: bool) -> io::Result<()> {
        self.emit_synced(EV_KEY, button_code, if pressed { 1 } else { 0 })
    }

    pub fn set_axis(&mut self, axis_code: u16, value: f32) -> io::Result<()> {
        self.emit_synced(EV_ABS, axis_code, normalized_axis_value(value)?)
    }

    fn emit_synced(&mut self, event_type: u16, code: u16, value: i32) -> io::Result<()> {
        let events = [
            InputEvent::new(event_type, code, value),
            InputEvent::new(EV_SYN, SYN_REPORT, 0),
        ];

        let bytes =
            unsafe { slice::from_raw_parts(events.as_ptr().cast::<u8>(), size_of_val(&events)) };
        self.file.write_all(bytes)
    }
}

impl Keyboard {
    pub fn new() -> io::Result<Self> {
        let file = open_uinput()?;
        let fd = file.as_raw_fd();

        ioctl_int(fd, UI_SET_EVBIT, EV_KEY as c_int, "enable EV_KEY")?;
        for code in KEY_ALL {
            ioctl_int(fd, UI_SET_KEYBIT, *code as c_int, "enable a keyboard key")?;
        }

        let mut setup: UinputSetup = unsafe { std::mem::zeroed() };
        setup.id = InputId {
            bustype: BUS_VIRTUAL,
            vendor: 0,
            product: 0,
            version: 0x0001,
        };
        let name = b"XR HOTAS Keyboard\0";
        setup.name[..name.len()].copy_from_slice(name);

        let phys = b"xr-hotas-keyboard\0";
        let rc = unsafe { ioctl(fd, UI_SET_PHYS, phys.as_ptr() as *const c_char) };
        if rc < 0 {
            return Err(io::Error::last_os_error());
        }

        ioctl_ptr(fd, UI_DEV_SETUP, &setup, "configure the keyboard")?;
        ioctl_none(fd, UI_DEV_CREATE, "create the keyboard")?;

        Ok(Self {
            file,
            pressed: HashSet::new(),
        })
    }

    pub fn set_key(&mut self, key_code: u16, pressed: bool) -> io::Result<()> {
        if !KEY_ALL.contains(&key_code) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "keyboard key is not enabled on this virtual device",
            ));
        }

        let changed = if pressed {
            self.pressed.insert(key_code)
        } else {
            self.pressed.remove(&key_code)
        };

        if !changed {
            return Ok(());
        }

        self.emit_synced(EV_KEY, key_code, if pressed { 1 } else { 0 })
    }

    fn emit_synced(&mut self, event_type: u16, code: u16, value: i32) -> io::Result<()> {
        let events = [
            InputEvent::new(event_type, code, value),
            InputEvent::new(EV_SYN, SYN_REPORT, 0),
        ];

        let bytes =
            unsafe { slice::from_raw_parts(events.as_ptr().cast::<u8>(), size_of_val(&events)) };
        self.file.write_all(bytes)
    }
}

impl InputEvent {
    const fn new(event_type: u16, code: u16, value: i32) -> Self {
        Self {
            // linux ignores timestamps written through uinput
            time: TimeVal {
                tv_sec: 0,
                tv_usec: 0,
            },
            event_type,
            code,
            value,
        }
    }
}

impl Drop for Hotas {
    fn drop(&mut self) {
        let _ = ioctl_none(self.file.as_raw_fd(), UI_DEV_DESTROY, "destroy the HOTAS");
    }
}

impl Drop for Keyboard {
    fn drop(&mut self) {
        let pressed: Vec<u16> = self.pressed.iter().copied().collect();
        for code in pressed {
            let _ = self.emit_synced(EV_KEY, code, 0);
        }
        let _ = ioctl_none(self.file.as_raw_fd(), UI_DEV_DESTROY, "destroy the keyboard");
    }
}

fn open_uinput() -> io::Result<File> {
    let primary = OpenOptions::new().write(true).open("/dev/uinput");
    match primary {
        Ok(file) => Ok(file),
        Err(primary_error) => match OpenOptions::new().write(true).open("/dev/input/uinput") {
            Ok(file) => Ok(file),
            Err(fallback_error) => Err(io::Error::new(
                fallback_error.kind(),
                format!(
                    "could not open /dev/uinput ({primary_error}) or /dev/input/uinput ({fallback_error})"
                ),
            )),
        },
    }
}

fn setup_axis(fd: c_int, code: u16, minimum: i32, maximum: i32, flat: i32) -> io::Result<()> {
    ioctl_int(fd, UI_SET_ABSBIT, code as c_int, "enable a HOTAS axis")?;

    // zeroing first also initializes the abi padding after `code`
    let mut setup: UinputAbsSetup = unsafe { std::mem::zeroed() };
    setup.code = code;
    setup.absinfo = InputAbsInfo {
        value: 0,
        minimum,
        maximum,
        fuzz: 0,
        flat,
        resolution: 0,
    };
    ioctl_ptr(fd, UI_ABS_SETUP, &setup, "configure a HOTAS axis")
}

fn normalized_axis_value(value: f32) -> io::Result<i32> {
    if !value.is_finite() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "HOTAS axis value must be finite",
        ));
    }

    let normalized = value.clamp(-1.0, 1.0);
    let offset = ((normalized + 1.0) * 0.5 * (AXIS_LEVELS - 1) as f32).round() as i32;
    Ok(AXIS_MIN + offset)
}

fn ioctl_none(fd: c_int, request: c_ulong, context: &str) -> io::Result<()> {
    // SAFETY: `fd` is an open uinput descriptor and this request has no third arg as per input.h
    let result = unsafe { ioctl(fd, request) };
    ioctl_result(result, context)
}

fn ioctl_int(fd: c_int, request: c_ulong, value: c_int, context: &str) -> io::Result<()> {
    // SAFETY: these ioctls take an integer value
    let result = unsafe { ioctl(fd, request, value) };
    ioctl_result(result, context)
}

fn ioctl_ptr<T>(fd: c_int, request: c_ulong, value: &T, context: &str) -> io::Result<()> {
    // SAFETY: the referenced value remains alive for the ioctl
    let result = unsafe { ioctl(fd, request, value as *const T) };
    ioctl_result(result, context)
}

fn ioctl_result(result: c_int, context: &str) -> io::Result<()> {
    if result < 0 {
        let source = io::Error::last_os_error();
        Err(io::Error::new(
            source.kind(),
            format!("{context}: {source}"),
        ))
    } else {
        Ok(())
    }
}
