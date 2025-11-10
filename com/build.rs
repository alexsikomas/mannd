use std::{env, path::PathBuf};

use which::which;

fn main() {
    println!("cargo::rustc-check-cfg=cfg(iwd_installed)");
    if which("iw").is_ok() {
        println!("cargo:rustc-cfg=iwd_installed");
    }

    // wpa
    if which("wpa_supplicant").is_ok() {
        cc::Build::new()
            .file("./wpa_supplicant/src/common/wpa_ctrl.c")
            .file("./wpa_supplicant/src/utils/os_unix.c")
            .include("./wpa_supplicant/src/utils")
            .include("./wpa_supplicant/src/common")
            .define("CONFIG_CTRL_IFACE", None)
            .define("CONFIG_CTRL_IFACE_UNIX", None)
            .compile("wpa_ctrl");

        let bindings = bindgen::Builder::default()
            .header("./wpa_supplicant/header.h")
            .clang_args([
                "-I./wpa_supplicant/src/utils/",
                "-I./wpa_supplicant/src/common/",
            ])
            .clang_arg("-DCONFIG_CTRL_IFACE")
            .clang_arg("-DCONFIG_CTRL_IFACE_UNIX")
            .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
            .generate()
            .expect("unable to generate bindings for wpa");

        let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
        bindings
            .write_to_file(out_path.join("bindings.rs"))
            .expect("Couldn't write bindings for wpa");
    }
}
