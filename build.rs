fn main() {
    // Add custom cfg to prevent duplicate macro expansion
    println!("cargo:rustc-cfg=totp_manager_main");
    tauri_build::build();
}
