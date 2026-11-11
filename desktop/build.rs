fn main() {
    println!("cargo:rerun-if-changed=update-public-key.txt");
    if std::env::var("PROFILE").as_deref() == Ok("release") {
        let key = std::fs::read_to_string("update-public-key.txt").unwrap_or_default();
        assert!(
            !key.trim().is_empty(),
            "Öffentlicher Inspect-Updateschlüssel fehlt für den Release-Build"
        );
    }
    tauri_build::build()
}
