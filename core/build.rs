fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rustc-check-cfg=cfg(iwd_installed)");
    println!("cargo::rustc-check-cfg=cfg(wpa_installed)");
    if std::process::Command::new("iwctl")
        .arg("--help")
        .output()
        .is_ok()
    {
        println!("cargo:rustc-cfg=iwd_installed");
    }

    if std::process::Command::new("wpa_supplicant")
        .arg("-v")
        .output()
        .is_ok()
    {
        println!("cargo:rustc-cfg=wpa_installed");
    }
}
