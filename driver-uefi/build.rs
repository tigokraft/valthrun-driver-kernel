use std::{
    env::var,
    path::{
        Path,
        PathBuf,
    },
};

use winreg::{
    enums::*,
    RegKey,
};

fn get_windows_kits_dir() -> anyhow::Result<PathBuf> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

    let key = r"SOFTWARE\Microsoft\Windows Kits\Installed Roots";

    let dir: String = hklm.open_subkey(key)?.get_value("KitsRoot10")?;

    Ok(dir.into())
}

fn get_km_dirs(windows_kits_dir: &PathBuf) -> anyhow::Result<(PathBuf, PathBuf)> {
    let readdir = Path::new(windows_kits_dir).join("lib").read_dir()?;

    let max_libdir = readdir
        .filter_map(|dir| dir.ok())
        .map(|dir| dir.path())
        .filter(|dir| {
            dir.components()
                .last()
                .and_then(|c| c.as_os_str().to_str())
                .map(|c| c.starts_with("10.") && dir.join("km").is_dir())
                .unwrap_or(false)
        })
        .max()
        .ok_or_else(|| {
            anyhow::anyhow!("Can not find a valid km dir in `{:?}`", windows_kits_dir)
        })?;

    let version = max_libdir
        .file_name()
        .expect("expected to have a file name");

    Ok((
        max_libdir.join("km"),
        Path::new(windows_kits_dir)
            .join("Include")
            .join(version)
            .join("km"),
    ))
}

fn main() -> anyhow::Result<()> {
    let windows_kits_dir = get_windows_kits_dir().unwrap();
    let (km_lib, _km_include) = get_km_dirs(&windows_kits_dir).unwrap();

    let target = var("TARGET").unwrap();
    let arch = if target.contains("x86_64") {
        "x64"
    } else if target.contains("i686") {
        "x86"
    } else {
        panic!("Only support x86_64 and i686!");
    };

    let lib_dir = km_lib.join(arch);
    println!(
        "cargo:rustc-link-search=native={}",
        lib_dir.to_str().unwrap()
    );

    Ok(())
}
