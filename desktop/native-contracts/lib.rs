//! Pure release contracts also used by the GUI, without GTK or a Windows desktop.
#[path = "../src/cli_export.rs"]
pub mod cli_export;
#[path = "../src/update_policy.rs"]
pub mod update_policy;

pub mod application_updates {
    pub const PUBLIC_KEY: &str = include_str!("../update-public-key.txt");
}

#[cfg(test)]
mod export_tests {
    use super::cli_export;
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "inspect-license-fixture-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir(&root).unwrap();
            Self(root)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn portable_export_retains_public_license_text_and_never_overwrites_existing_files() {
        let fixture = Fixture::new();
        let source = fixture.0.join("source");
        fs::create_dir(&source).unwrap();
        fs::create_dir(source.join("library")).unwrap();
        fs::write(source.join("library/LICENSE"), "Synthetic public license").unwrap();
        let target = fixture.0.join("export");
        cli_export::licenses(&source, &target).unwrap();
        assert_eq!(
            fs::read_to_string(target.join("library/LICENSE")).unwrap(),
            "Synthetic public license"
        );
        assert!(cli_export::licenses(&source, &target).is_err());
        assert_eq!(
            fs::read_to_string(target.join("library/LICENSE")).unwrap(),
            "Synthetic public license"
        );
    }
    #[test]
    fn oversized_license_material_is_rejected_before_copying() {
        let fixture = Fixture::new();
        let source = fixture.0.join("source");
        fs::create_dir(&source).unwrap();
        fs::File::create(source.join("large"))
            .unwrap()
            .set_len(33 * 1024 * 1024)
            .unwrap();
        assert!(cli_export::licenses(&source, &fixture.0.join("export")).is_err());
        assert!(!fixture.0.join("export/large").exists());
    }
    #[cfg(unix)]
    #[test]
    fn bundled_license_links_are_not_followed() {
        let fixture = Fixture::new();
        let source = fixture.0.join("source");
        fs::create_dir(&source).unwrap();
        fs::write(fixture.0.join("unrelated"), "Not exported").unwrap();
        std::os::unix::fs::symlink(fixture.0.join("unrelated"), source.join("link")).unwrap();
        assert!(cli_export::licenses(&source, &fixture.0.join("export")).is_err());
        assert!(!fixture.0.join("export/link").exists());
    }
}

#[path = "../src/app_coordination.rs"]
pub mod app_coordination;
