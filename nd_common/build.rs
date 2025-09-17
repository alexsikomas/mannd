use which::which;

fn main() {
    println!("cargo::rustc-check-cfg=cfg(iwd_installed)");
    if which("iw").is_ok() {
        println!("cargo:rustc-cfg=iwd_installed");
    }
}
