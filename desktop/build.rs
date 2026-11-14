mod cooperation_marker;

fn main() {
    println!("cargo:rerun-if-changed=update-public-key.txt");
    if std::env::var("PROFILE").as_deref() == Ok("release") {
        let key = std::fs::read_to_string("update-public-key.txt").unwrap_or_default();
        assert!(
            !key.trim().is_empty(),
            "Öffentlicher Inspect-Updateschlüssel fehlt für den Release-Build"
        );
    }
    let mut attributes = tauri_build::Attributes::new();
    let target = std::env::var("TARGET").expect("Cargo target missing");
    let out_dir = std::path::PathBuf::from(
        std::env::var_os("OUT_DIR").expect("Cargo output directory missing"),
    );
    if let Some(resource) = cooperation_marker::resource_for_target(&target, &out_dir)
        .expect("Cooperation marker generation failed")
    {
        attributes = attributes
            .windows_attributes(tauri_build::WindowsAttributes::new().append_rc_content(resource));
    }
    tauri_build::try_build(attributes).expect("Tauri build configuration failed");
}
