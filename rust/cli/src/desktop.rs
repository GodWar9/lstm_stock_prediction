//! Portable recorded-research app. No runtime toolchain or provider credentials.
use anyhow::{ensure, Context};
use clap::Parser;
use flate2::read::GzDecoder;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, io::Read, path::Path, process::Command};

const ARCHIVE: &[u8] = include_bytes!("../../../deploy/artifacts.tar.gz");
const MANIFEST: &[u8] = include_bytes!("../../../deploy/artifacts.tar.json");
const MAX_BYTES: u64 = 128 * 1024 * 1024;

#[derive(Parser)]
#[command(
    version,
    about = "LSTM research inspector with bundled synthetic data and real ONNX inference"
)]
struct Args {
    /// Choose a local port; zero selects an available port.
    #[arg(long, default_value_t = 0)]
    port: u16,
    /// Print the local URL without opening the default browser.
    #[arg(long)]
    no_browser: bool,
    /// Verify all embedded artifacts and exit without starting a server.
    #[arg(long)]
    check: bool,
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn extract(archive: &[u8], manifest: &[u8], root: &Path) -> anyhow::Result<Value> {
    let expected: Value = serde_json::from_slice(manifest)?;
    ensure!(
        expected["schema_version"] == 1,
        "Unsupported bundle version"
    );
    ensure!(
        expected["source"] == "synthetic",
        "Portable sample must be labelled synthetic"
    );
    ensure!(
        expected["bundle_sha256"].as_str() == Some(&digest(archive)),
        "Bundle checksum mismatch"
    );
    let files = expected["files"]
        .as_object()
        .context("Missing bundle file manifest")?;
    ensure!(
        !files.is_empty() && files.len() <= 1000,
        "Invalid bundle file count"
    );
    ensure!(
        root.is_dir() && root.read_dir()?.next().is_none(),
        "Extraction target must be empty"
    );
    let mut seen = BTreeSet::new();
    let mut case_names = BTreeSet::new();
    let mut total = 0_u64;
    // Bound decompressed bytes, including tar headers, before reading entries.
    let decoded = GzDecoder::new(archive).take(MAX_BYTES + 2 * 1024 * 1024);
    for entry in tar::Archive::new(decoded).entries()? {
        let mut entry = entry?;
        let name = std::str::from_utf8(&entry.path_bytes())?.to_owned();
        ensure!(
            entry.header().entry_type().is_file()
                && !name.contains(['\\', ':'])
                && name
                    .split('/')
                    .all(|part| !part.is_empty() && part != "." && part != "..")
                && seen.insert(name.clone())
                && case_names.insert(name.to_ascii_lowercase()),
            "Unsafe or duplicate bundle path"
        );
        ensure!(
            name == "bundle.json" || files.contains_key(&name),
            "Unexpected bundle file"
        );
        total = total
            .checked_add(entry.size())
            .context("Bundle size overflow")?;
        ensure!(
            total <= MAX_BYTES && seen.len() <= 1001,
            "Bundle exceeds extraction limits"
        );
        let mut data = Vec::new();
        entry.read_to_end(&mut data)?;
        if name == "bundle.json" {
            let mut inner_expected = expected.clone();
            inner_expected
                .as_object_mut()
                .unwrap()
                .remove("bundle_sha256");
            ensure!(
                serde_json::from_slice::<Value>(&data)? == inner_expected,
                "Inner manifest mismatch"
            );
        } else {
            ensure!(
                files[&name]["size_bytes"].as_u64() == Some(data.len() as u64)
                    && files[&name]["sha256"].as_str() == Some(&digest(&data)),
                "Artifact checksum mismatch"
            );
        }
        let destination = root.join(&name);
        std::fs::create_dir_all(destination.parent().context("Missing parent")?)?;
        std::fs::write(destination, data)?;
    }
    ensure!(
        seen.len() == files.len() + 1
            && seen.contains("bundle.json")
            && files.keys().all(|name| seen.contains(name)),
        "Incomplete bundle"
    );
    Ok(expected)
}

fn open_browser(url: &str) -> anyhow::Result<()> {
    #[cfg(target_os = "windows")]
    let mut command = {
        use std::os::windows::process::CommandExt;
        let system = std::env::var_os("SystemRoot").context("Windows directory unavailable")?;
        let mut command = Command::new(Path::new(&system).join("explorer.exe"));
        command.creation_flags(0x08000000);
        command
    };
    #[cfg(target_os = "macos")]
    let mut command = Command::new("open");
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let mut command = Command::new("xdg-open");
    command
        .arg(url)
        .spawn()
        .context("Could not open the browser")?;
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    // This happens before creating any runtime threads. The portable sample must
    // never use credentials or enable a feed inherited from a developer shell.
    for key in [
        "QUANTCTL_LIVE_SYMBOLS",
        "APCA_API_KEY_ID",
        "APCA_API_SECRET_KEY",
        "TWELVE_DATA_API_KEY",
        "QUANTCTL_BACKEND_TOKEN",
    ] {
        std::env::remove_var(key);
    }
    std::env::set_var("QUANTCTL_ALLOW_NETWORK", "0");
    let workspace = tempfile::Builder::new()
        .prefix("lstm-inspector-")
        .tempdir()?;
    let manifest = extract(ARCHIVE, MANIFEST, workspace.path())?;
    if args.check {
        println!(
            "{}",
            serde_json::json!({"status":"verified", "source":manifest["source"],
            "model":manifest["model"], "dataset":manifest["dataset"], "symbol":manifest["symbol"],
            "bundle_sha256":manifest["bundle_sha256"],
            "files":manifest["files"].as_object().unwrap().len()})
        );
        workspace.close()?;
        return Ok(());
    }
    println!("LSTM research inspector\nBundled synthetic historical data; real ONNX inference.\nKeep this window open. Press Ctrl+C here to stop the app and clean up.");
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let result = runtime.block_on(quant_api::serve_with_ready(
        workspace.path().into(),
        args.port,
        quant_api::ServerOptions::default(),
        |address| {
            let url = format!("http://{address}/forecast");
            println!("Open the app: {url}");
            if !args.no_browser {
                if let Err(error) = open_browser(&url) {
                    eprintln!("{error}. Open the URL above manually.");
                }
            }
        },
    ));
    workspace.close()?;
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_bundle_extracts_all_verified_artifacts() {
        let root = tempfile::tempdir().unwrap();
        let manifest = extract(ARCHIVE, MANIFEST, root.path()).unwrap();
        assert_eq!(manifest["source"], "synthetic");
        for name in manifest["files"].as_object().unwrap().keys() {
            assert!(root.path().join(name).is_file(), "{name}");
        }
    }

    #[test]
    fn corrupted_archive_and_file_manifest_are_rejected() {
        let root = tempfile::tempdir().unwrap();
        let mut archive = ARCHIVE.to_vec();
        archive[10] ^= 1;
        assert!(extract(&archive, MANIFEST, root.path())
            .unwrap_err()
            .to_string()
            .contains("checksum"));
        let mut manifest: Value = serde_json::from_slice(MANIFEST).unwrap();
        let first = manifest["files"]
            .as_object_mut()
            .unwrap()
            .values_mut()
            .next()
            .unwrap();
        first["sha256"] = Value::String("0".repeat(64));
        assert!(extract(
            ARCHIVE,
            &serde_json::to_vec(&manifest).unwrap(),
            root.path()
        )
        .is_err());
    }

    #[test]
    fn never_overwrites_an_existing_workspace() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("keep"), "original").unwrap();
        assert!(extract(ARCHIVE, MANIFEST, root.path()).is_err());
        assert_eq!(
            std::fs::read_to_string(root.path().join("keep")).unwrap(),
            "original"
        );
    }

    #[test]
    fn rejects_path_traversal_even_with_a_matching_archive_hash() {
        let mut builder = tar::Builder::new(Vec::new());
        let mut header = tar::Header::new_gnu();
        header.set_size(1);
        header.set_mode(0o644);
        // Construct malicious raw tar metadata; the safe builder rejects this path.
        header.as_mut_bytes()[..9].copy_from_slice(b"../escape");
        header.set_cksum();
        builder.append(&header, &b"x"[..]).unwrap();
        let raw = builder.into_inner().unwrap();
        let mut compressed =
            flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        std::io::Write::write_all(&mut compressed, &raw).unwrap();
        let archive = compressed.finish().unwrap();
        let manifest = serde_json::json!({"schema_version":1,"source":"synthetic",
            "bundle_sha256":digest(&archive),"files":{"../escape":{"sha256":digest(b"x"),"size_bytes":1}}});
        let root = tempfile::tempdir().unwrap();
        assert!(extract(
            &archive,
            &serde_json::to_vec(&manifest).unwrap(),
            root.path()
        )
        .unwrap_err()
        .to_string()
        .contains("Unsafe"));
        assert_eq!(root.path().read_dir().unwrap().count(), 0);
    }
}
