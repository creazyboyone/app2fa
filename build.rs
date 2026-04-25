fn main() {
    println!("cargo:rustc-cfg=totp_manager_main");
    tauri_build::build();
}
