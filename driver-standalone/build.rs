use std::env;

use wdk_locator::locate_wdk;

fn main() {
    let windows_kit = locate_wdk().unwrap();

    let target = env::var("TARGET").unwrap();
    let arch = if target.contains("x86_64") {
        "x64"
    } else if target.contains("i686") {
        "x86"
    } else {
        panic!("Only support x86_64 and i686!");
    };

    let lib_dir = windows_kit.dir_libs.join(arch);
    println!(
        "cargo:rustc-link-search=native={}",
        lib_dir.to_str().unwrap()
    );

    println!("cargo:rustc-link-arg=/DEF:driver-standalone/.cargo/link.def");
}
