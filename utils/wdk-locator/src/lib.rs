use std::path::{
    Path,
    PathBuf,
};

use winreg::{
    enums::*,
    RegKey,
};

#[derive(Debug)]
pub struct WindowsKit {
    pub version: String,
    pub dir_root: PathBuf,

    pub dir_include: PathBuf,
    pub dir_libs: PathBuf,
}

fn get_windows_kits_dir() -> anyhow::Result<PathBuf> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let key = r"SOFTWARE\Microsoft\Windows Kits\Installed Roots";
    let dir: String = hklm.open_subkey(key)?.get_value("KitsRoot10")?;

    Ok(dir.into())
}

fn find_latest_version(windows_kits_dir: &PathBuf) -> anyhow::Result<String> {
    let max_libdir = Path::new(windows_kits_dir)
        .join("lib")
        .read_dir()?
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

    Ok(max_libdir
        .file_name()
        .expect("expected to have a file name")
        .to_string_lossy()
        .to_string())
}

pub fn locate_wdk() -> anyhow::Result<WindowsKit> {
    let windows_kits_dir = get_windows_kits_dir()?;
    let version = find_latest_version(&windows_kits_dir)?;

    Ok(WindowsKit {
        dir_root: windows_kits_dir.clone(),
        dir_include: windows_kits_dir.join("Include").join(&version).join("km"),
        dir_libs: windows_kits_dir.join("lib").join(&version).join("km"),

        version,
    })
}

#[cfg(test)]
mod test {
    use crate::{
        find_latest_version,
        get_windows_kits_dir,
    };

    #[test]
    fn print_status() {
        let windows_kits_dir = get_windows_kits_dir().unwrap();
        let version = find_latest_version(&windows_kits_dir).unwrap();
        println!("WDK installation details");
        println!("Directory: {}", windows_kits_dir.display());
        println!("Version  : {}", version);
    }
}
