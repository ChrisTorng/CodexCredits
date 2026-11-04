fn main() {
    // Embed the exact shared Android artwork as the native Windows EXE/taskbar
    // and default window icon; do not fall back to Tauri's placeholder.
    let attributes = tauri_build::Attributes::new().windows_attributes(
        tauri_build::WindowsAttributes::new().window_icon_path("icons/icon.ico"),
    );
    tauri_build::try_build(attributes).expect("failed to build Tauri app");
}
