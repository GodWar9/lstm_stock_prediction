//! Fingerprint working source files, including staged, modified and untracked code.
use crate::contract::SourceSnapshot;
use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::Path, process::Command};

fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let output = Command::new("git").current_dir(root).args(args).output()?;
    if !output.status.success() {
        bail!(
            "Git source inventory failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(output.stdout)
}

pub fn snapshot(root: &Path) -> Result<SourceSnapshot> {
    let root = root.canonicalize()?;
    let paths = git(
        &root,
        &[
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
            "--",
            "rust",
            "python",
            "web",
            "configs",
            "scripts",
            ".github",
        ],
    )?;
    let mut files = BTreeMap::<String, Option<String>>::new();
    for bytes in paths.split(|b| *b == 0).filter(|b| !b.is_empty()) {
        let name = std::str::from_utf8(bytes).context("Source filenames must be UTF-8")?;
        let path = Path::new(name);
        let extension = path.extension().and_then(|s| s.to_str()).unwrap_or("");
        if !matches!(
            extension,
            "rs" | "py"
                | "ts"
                | "tsx"
                | "js"
                | "mjs"
                | "css"
                | "html"
                | "toml"
                | "lock"
                | "yaml"
                | "yml"
                | "ps1"
        ) && !matches!(
            path.file_name().and_then(|s| s.to_str()),
            Some("package.json" | "package-lock.json" | "tsconfig.json" | "tsconfig.node.json")
        ) {
            continue;
        }
        if name.starts_with("web/tests/fixtures/") || name.starts_with("python/research/") {
            continue;
        }
        let full = root.join(path);
        let digest = if full.exists() {
            let resolved = full.canonicalize()?;
            if !resolved.starts_with(&root) || fs::symlink_metadata(&full)?.file_type().is_symlink()
            {
                bail!("Source path escapes repository or is a symlink: {name}");
            }
            Some(format!("{:x}", Sha256::digest(fs::read(full)?)))
        } else {
            None
        };
        files.insert(name.to_owned(), digest);
    }
    if files.is_empty() {
        bail!("No source files available for provenance");
    }
    let git_commit = git(&root, &["rev-parse", "HEAD"])
        .map(|v| String::from_utf8_lossy(&v).trim().to_owned())
        .unwrap_or_else(|_| "unknown".into());
    let dirty = !git(
        &root,
        &["status", "--porcelain", "--untracked-files=normal"],
    )?
    .is_empty();
    Ok(SourceSnapshot {
        git_commit,
        working_tree_dirty: dirty,
        content_sha256: format!("{:x}", Sha256::digest(serde_json::to_vec(&files)?)),
        file_count: files.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fingerprints_bytes_untracked_sources_and_deletions() {
        let dir = tempfile::tempdir().unwrap();
        git(dir.path(), &["init"]).unwrap();
        fs::create_dir(dir.path().join("rust")).unwrap();
        let source = dir.path().join("rust/example.rs");
        fs::write(&source, "original").unwrap();
        git(dir.path(), &["add", "rust/example.rs"]).unwrap();
        let before = snapshot(dir.path()).unwrap();
        assert_eq!(
            before.content_sha256,
            snapshot(dir.path()).unwrap().content_sha256
        );
        fs::write(&source, "modified").unwrap();
        let after = snapshot(dir.path()).unwrap();
        assert_ne!(before.content_sha256, after.content_sha256);
        assert!(after.working_tree_dirty);
        fs::write(dir.path().join("rust/new.rs"), "untracked").unwrap();
        assert_eq!(snapshot(dir.path()).unwrap().file_count, 2);
        fs::remove_file(&source).unwrap();
        assert_ne!(
            after.content_sha256,
            snapshot(dir.path()).unwrap().content_sha256
        );
    }
}
