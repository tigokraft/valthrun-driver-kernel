use obfstr::obfstr;
use vtd_protocol::{
    command::{
        DriverCommandInitialize,
        InitializeResult,
        VersionInfo,
    },
    types::DriverFeature,
    PROTOCOL_VERSION,
};

use crate::{
    KEYBOARD_INPUT,
    METRICS_CLIENT,
    MOUSE_INPUT,
};

fn driver_version() -> VersionInfo {
    let mut info = VersionInfo::default();
    info.set_application_name(obfstr!("kernel-driver"));

    info.version_major = env!("CARGO_PKG_VERSION_MAJOR").parse::<u32>().unwrap();
    info.version_minor = env!("CARGO_PKG_VERSION_MINOR").parse::<u32>().unwrap();
    info.version_patch = env!("CARGO_PKG_VERSION_PATCH").parse::<u32>().unwrap();

    return info;
}

pub fn handler_init(command: &mut DriverCommandInitialize) -> anyhow::Result<()> {
    command.driver_protocol_version = PROTOCOL_VERSION;
    if command.client_protocol_version != PROTOCOL_VERSION {
        return Ok(());
    }

    let feature_mouse = if unsafe { &*MOUSE_INPUT.get() }.is_some() {
        DriverFeature::InputMouse
    } else {
        DriverFeature::empty()
    };

    let feature_keyboard = if unsafe { &*KEYBOARD_INPUT.get() }.is_some() {
        DriverFeature::InputKeyboard
    } else {
        DriverFeature::empty()
    };

    let feature_metrics = if unsafe { &*METRICS_CLIENT.get() }.is_some() {
        DriverFeature::Metrics
    } else {
        DriverFeature::empty()
    };

    command.result = InitializeResult::Success;
    command.driver_version = driver_version();
    command.driver_features = DriverFeature::ProcessList |
        DriverFeature::ProcessModules |
        DriverFeature::MemoryRead |
        DriverFeature::MemoryWrite |
        feature_mouse |
        feature_keyboard |
        feature_metrics |
        DriverFeature::ProcessProtectionKernel;

    Ok(())
}
