fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "get_server_url",
            "open_workspace",
            "open_external",
        ]),
    ))
    .expect("failed to run Tauri build script");
}
