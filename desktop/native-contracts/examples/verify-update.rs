use fbforge_inspect_native_contracts::update_policy;
use std::{fs, io::Read, path::Path};

fn read(path: &Path, limit: usize) -> Result<Vec<u8>, &'static str> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|_| "update_file_missing")?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "update_file_invalid")?;
    if bytes.len() > limit {
        return Err("update_file_too_large");
    }
    Ok(bytes)
}
fn verify(directory: &Path, key: &Path) -> Result<(), &'static str> {
    let raw = read(&directory.join("latest.json"), update_policy::MAX_MANIFEST)?;
    let manifest: serde_json::Value =
        serde_json::from_slice(&raw).map_err(|_| "update_manifest_invalid")?;
    update_policy::manifest(&manifest, "0.0.0")?;
    let version = manifest["version"]
        .as_str()
        .ok_or("update_manifest_invalid")?;
    let name = format!("FBForge-Inspect-{version}-windows-x64-setup.exe");
    let path = directory.join(name);
    let bytes = read(&path, update_policy::MAX_PACKAGE)?;
    let signature = String::from_utf8(read(&path.with_extension("exe.sig"), 4096)?)
        .map_err(|_| "update_signature_invalid")?;
    if manifest["platforms"]["windows-x86_64"]["signature"].as_str() != Some(signature.trim()) {
        return Err("update_signature_invalid");
    }
    let key = String::from_utf8(read(key, 4096)?).map_err(|_| "update_not_configured")?;
    update_policy::verify(&bytes, &signature, &key, version)
}
fn main() {
    let args: Vec<_> = std::env::args_os().collect();
    if args.len() != 3 || verify(Path::new(&args[1]), Path::new(&args[2])).is_err() {
        eprintln!("update_artifact_verification_failed");
        std::process::exit(1);
    }
    println!(
        "Inspect package signature, signed product, signed version and download identity verified."
    );
}
