//! Local authors' utility. Never invokes a signer, shell, installer or network.
use fbforge_suite_core::{
    catalog::parse_catalog,
    production::{bundled_authenticode_policies, bundled_metadata_trust_store},
    trust::{
        self,
        authoring::{self, MetadataHeader, PreparedMessage},
        TrustError, TrustStore,
    },
};
use semver::Version;
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

const INPUT_LIMIT: u64 = 2_097_152;
const HELP: &str="FBForge Suite: lokale Metadatenvorbereitung v2\n\
prepare-catalog --body DATEI --sequence N --issued-at UNIX --expires-at UNIX --out DATEI\n\
prepare-release --body DATEI --catalog-body DATEI --tool ID --version SEMVER --sequence N --issued-at UNIX --expires-at UNIX --out DATEI\n\
assemble-catalog --message DATEI --signature DATEI --out DATEI\n\
assemble-release --message DATEI --signature DATEI --catalog DATEI --tool ID --version SEMVER --out DATEI\n\
check-package --catalog DATEI --release DATEI --tool ID --version SEMVER --package DATEI\n\
Vorbereitete Nachrichten sind unvertrauenswürdig. Es werden keine Schlüssel gelesen, keine Signierer ausgeführt und keine Dateien überschrieben.\n";

#[derive(Debug)]
enum CliError {
    Arguments,
    Input,
    TooLarge,
    OutputExists,
    Output,
    Clock,
    Trust(TrustError),
    #[cfg(not(windows))]
    WindowsUnavailable,
    #[cfg(windows)]
    Windows(fbforge_suite_core::authenticode::VerificationFailure),
}
impl From<TrustError> for CliError {
    fn from(error: TrustError) -> Self {
        Self::Trust(error)
    }
}
impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Arguments=>f.write_str("Die Argumente sind ungültig oder unvollständig. Verwenden Sie --help."),
            Self::Input=>f.write_str("Eine lokale Eingabedatei konnte nicht sicher gelesen werden."),
            Self::TooLarge=>f.write_str("Eine Eingabedatei überschreitet die zulässige Grenze."),
            Self::OutputExists=>f.write_str("Die Ausgabedatei existiert bereits und wurde nicht überschrieben."),
            Self::Output=>f.write_str("Die neue Ausgabedatei konnte nicht vollständig gespeichert werden. Verwerfen Sie eine mögliche Teildatei."),
            Self::Clock=>f.write_str("Die native Uhr konnte nicht verwendet werden."),
            Self::Trust(error)=>write!(f,"Die Metadaten- oder Paketprüfung ist fehlgeschlagen: {error}"),
            #[cfg(not(windows))] Self::WindowsUnavailable=>f.write_str("Der Paket-Hash passt. Authenticode wurde auf dieser Plattform nicht geprüft. Es liegt kein erfolgreicher Paketgesamtnachweis vor."),
            #[cfg(windows)] Self::Windows(error)=>write!(f,"{error}"),
        }
    }
}
impl CliError {
    fn exit_code(&self) -> i32 {
        #[cfg(not(windows))]
        if matches!(self, Self::WindowsUnavailable) {
            return 2;
        }
        1
    }
}

struct Arguments {
    command: String,
    values: BTreeMap<String, OsString>,
}
impl Arguments {
    fn parse(mut args: impl Iterator<Item = OsString>) -> Result<Self, CliError> {
        let command = args
            .next()
            .and_then(|s| s.into_string().ok())
            .ok_or(CliError::Arguments)?;
        let expected: &[&str] = match command.as_str() {
            "prepare-catalog" => &[
                "--body",
                "--sequence",
                "--issued-at",
                "--expires-at",
                "--out",
            ],
            "prepare-release" => &[
                "--body",
                "--catalog-body",
                "--tool",
                "--version",
                "--sequence",
                "--issued-at",
                "--expires-at",
                "--out",
            ],
            "assemble-catalog" => &["--message", "--signature", "--out"],
            "assemble-release" => &[
                "--message",
                "--signature",
                "--catalog",
                "--tool",
                "--version",
                "--out",
            ],
            "check-package" => &["--catalog", "--release", "--tool", "--version", "--package"],
            _ => return Err(CliError::Arguments),
        };
        let mut values = BTreeMap::new();
        while let Some(flag) = args.next() {
            let flag = flag.into_string().map_err(|_| CliError::Arguments)?;
            if !expected.contains(&flag.as_str()) || values.contains_key(&flag) {
                return Err(CliError::Arguments);
            }
            let value = args
                .next()
                .filter(|s| !s.is_empty() && s.len() <= 32768)
                .ok_or(CliError::Arguments)?;
            values.insert(flag, value);
        }
        if values.len() != expected.len() {
            return Err(CliError::Arguments);
        }
        Ok(Self { command, values })
    }
    fn path(&self, key: &str) -> &Path {
        Path::new(&self.values[key])
    }
    fn text(&self, key: &str) -> Result<&str, CliError> {
        self.values[key].to_str().ok_or(CliError::Arguments)
    }
    fn version(&self) -> Result<Version, CliError> {
        Version::parse(self.text("--version")?).map_err(|_| CliError::Arguments)
    }
    fn header(&self) -> Result<MetadataHeader, CliError> {
        let number = |key| self.text(key)?.parse().map_err(|_| CliError::Arguments);
        Ok(MetadataHeader {
            sequence: number("--sequence")?,
            issued_at: number("--issued-at")?,
            expires_at: number("--expires-at")?,
        })
    }
    fn read(&self, key: &str) -> Result<String, CliError> {
        read_text(self.path(key), INPUT_LIMIT)
    }
}

fn open_input(path: &Path) -> Result<File, CliError> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options
            .share_mode(windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ)
            .custom_flags(windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let file = options.open(path).map_err(|_| CliError::Input)?;
    let metadata = file.metadata().map_err(|_| CliError::Input)?;
    if !metadata.is_file() {
        return Err(CliError::Input);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes()
            & windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT
            != 0
        {
            return Err(CliError::Input);
        }
    }
    Ok(file)
}
fn read_text(path: &Path, limit: u64) -> Result<String, CliError> {
    let file = open_input(path)?;
    if file.metadata().map_err(|_| CliError::Input)?.len() > limit {
        return Err(CliError::TooLarge);
    }
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| CliError::Input)?;
    if bytes.len() as u64 > limit {
        return Err(CliError::TooLarge);
    }
    String::from_utf8(bytes).map_err(|_| CliError::Input)
}
fn write_new(path: &Path, bytes: &[u8]) -> Result<(), CliError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            CliError::OutputExists
        } else {
            CliError::Output
        }
    })?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| CliError::Output)
}

fn run(arguments: Arguments) -> Result<(), CliError> {
    match arguments.command.as_str() {
        "prepare-catalog" => {
            let message =
                authoring::prepare_catalog(&arguments.read("--body")?, arguments.header()?)?;
            write_new(arguments.path("--out"), message.as_bytes())?;
            println!(
                "Die unvertrauenswürdige Nachricht wurde vorbereitet. Sie ist noch nicht signiert."
            );
        }
        "prepare-release" => {
            let catalog = parse_catalog(&arguments.read("--catalog-body")?)
                .map_err(|_| TrustError::InvalidCatalog)?;
            let message = authoring::prepare_release(
                &arguments.read("--body")?,
                &catalog,
                arguments.text("--tool")?,
                &arguments.version()?,
                arguments.header()?,
            )?;
            write_new(arguments.path("--out"), message.as_bytes())?;
            println!("Die unvertrauenswürdige Release-Nachricht wurde vorbereitet. Sie ist noch nicht signiert.");
        }
        "assemble-catalog" | "assemble-release" => {
            let message = PreparedMessage::from_bytes(arguments.read("--message")?.as_bytes())?;
            let signature = read_text(arguments.path("--signature"), 16_384)?;
            let envelope = if arguments.command == "assemble-catalog" {
                authoring::assemble_catalog(&message, &signature)?
            } else {
                authoring::assemble_release(
                    &message,
                    &signature,
                    &arguments.read("--catalog")?,
                    arguments.text("--tool")?,
                    &arguments.version()?,
                )?
            };
            write_new(arguments.path("--out"), envelope.as_bytes())?;
            println!("Die vollständige Metadatenhülle wurde gegen die lokale Produktionspolicy geprüft und gespeichert. Sie erteilt keine Ausführungsfreigabe.");
        }
        "check-package" => {
            let store = bundled_metadata_trust_store()?;
            let policies = bundled_authenticode_policies().map_err(|_| CliError::Arguments)?;
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| CliError::Clock)?
                .as_secs();
            check_package_at(
                &arguments.read("--catalog")?,
                &arguments.read("--release")?,
                (arguments.text("--tool")?, &arguments.version()?),
                arguments.path("--package"),
                &store,
                &policies,
                now,
            )?;
            println!("Metadaten, Paketbytes und Windows-Publisher wurden geprüft. Dieser Nachweis erteilt keine Ausführungsfreigabe.");
        }
        _ => return Err(CliError::Arguments),
    }
    Ok(())
}

fn check_package_at(
    catalog: &str,
    release: &str,
    expected: (&str, &Version),
    path: &Path,
    store: &TrustStore,
    policies: &fbforge_suite_core::authenticode::AuthenticodePolicies,
    now: u64,
) -> Result<(), CliError> {
    let catalog = trust::verify_catalog(catalog, store, now, None)?;
    let release =
        trust::verify_release(release, store, now, None, &catalog, expected.0, expected.1)?;
    #[cfg(windows)]
    {
        fbforge_suite_core::authenticode::verify_package(path, &release, policies, now)
            .map_err(CliError::Windows)?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = policies;
        release.verify_package(open_input(path)?)?;
        Err(CliError::WindowsUnavailable)
    }
}

fn main() {
    let mut arguments = std::env::args_os().skip(1).peekable();
    if arguments.peek().is_some_and(|s| s == "--help") {
        if arguments.count() == 1 {
            print!("{HELP}");
            return;
        }
        eprintln!("{}", CliError::Arguments);
        std::process::exit(1);
    }
    if let Err(error) = Arguments::parse(arguments).and_then(run) {
        eprintln!("{error}");
        std::process::exit(error.exit_code());
    }
}

#[cfg(test)]
#[path = "fbforge-metadata/tests.rs"]
mod tests;
