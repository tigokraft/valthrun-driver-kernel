use obfstr::obfstr;
use valthrun_driver_shared::requests::{RequestInitialize, ResponseInitialize, INIT_STATUS_SUCCESS, INIT_STATUS_DRIVER_OUTDATED, INIT_STATUS_CONTROLLER_OUTDATED, DriverInfo, ControllerInfo};

fn driver_version() -> u32 {
    let major = env!("CARGO_PKG_VERSION_MAJOR").parse::<u32>().unwrap();
    let minor = env!("CARGO_PKG_VERSION_MINOR").parse::<u32>().unwrap();
    let patch = env!("CARGO_PKG_VERSION_PATCH").parse::<u32>().unwrap();
    return (major << 24) | (minor << 16) | (patch << 8); 
}

pub fn handler_init(req: &RequestInitialize, res: &mut ResponseInitialize) -> anyhow::Result<()> {
    res.status_code = INIT_STATUS_SUCCESS;
    res.driver_version = driver_version();

    if res.driver_version < req.target_version {
        /* driver is outdated */
        res.status_code = INIT_STATUS_DRIVER_OUTDATED;
        return Ok(());
    }
    if req.target_version > res.driver_version {
        /* Newer version requested. Assuming currently everything is a breaking change. */
        res.status_code = INIT_STATUS_CONTROLLER_OUTDATED;
        return Ok(());
    }

    let _controller_info = unsafe {
        if !seh::probe_read(req.controller_info as u64, req.controller_info_length, 0x01) {
            anyhow::bail!("{}", obfstr!("faild to read controller info"));
        }

        if req.controller_info_length < core::mem::size_of::<ControllerInfo>() {
            anyhow::bail!("{}", obfstr!("unexpected driver info size"));
        }

        &*req.controller_info
    };
    
    let _driver_info = unsafe {
        if !seh::probe_write(req.driver_info as u64, req.driver_info_length, 0x01) {
            anyhow::bail!("{}", obfstr!("faild to write driver info"));
        }

        if req.driver_info_length < core::mem::size_of::<DriverInfo>() {
            anyhow::bail!("{}", obfstr!("unexpected driver info size"));
        }

        &mut *req.driver_info
    };

    Ok(())
}