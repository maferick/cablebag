fn main() {
    // The Windows manifest (Common Controls 6, which dialogs need) is linked into every program of this crate,
    // test programs too: through tauri-build only the app got it, and a test program that pulls in the dialog
    // code then fails to start with STATUS_ENTRYPOINT_NOT_FOUND.
    let mut attrs = tauri_build::Attributes::new();
    #[cfg(windows)]
    {
        attrs = attrs.windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest());
        let manifest = std::env::current_dir().unwrap().join("windows-app-manifest.xml");
        println!("cargo:rerun-if-changed={}", manifest.display());
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
        println!("cargo:rustc-link-arg=/WX");
    }
    tauri_build::try_build(attrs).expect("tauri build");
}
