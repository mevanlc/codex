use super::*;
use pretty_assertions::assert_eq;

fn write(root: &Path, relative: &str, contents: &str) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, contents).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    }
}

/// An independent encoding of the documented format, so any change to it is deliberate.
#[test]
fn digest_matches_the_documented_format() {
    let dir = tempfile::tempdir().unwrap();
    // Bytewise order puts "a-c" before "a/b", unlike per-component sorting.
    write(dir.path(), "a/b", "first");
    write(dir.path(), "a-c", "second");
    write(dir.path(), "B", "third");
    write(dir.path(), RECEIPT, "ignored");

    let mut expected = blake3::Hasher::new();
    expected.update(b"codex-package-tree-v2\0");
    let mode: u32 = if cfg!(unix) { 0o644 } else { 0 };
    for (relative, contents) in [("B", "third"), ("a-c", "second"), ("a/b", "first")] {
        expected.update(&(relative.len() as u64).to_le_bytes());
        expected.update(relative.as_bytes());
        expected.update(&mode.to_le_bytes());
        expected.update(blake3::hash(contents.as_bytes()).as_bytes());
    }
    assert_eq!(
        tree_digest(dir.path()).unwrap(),
        expected.finalize().to_hex().to_string()
    );
}

#[test]
fn digest_tracks_names_contents_and_location_but_not_the_receipt() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "bin/codex", "codex");
    let original = tree_digest(dir.path()).unwrap();

    write(dir.path(), RECEIPT, "{}");
    assert_eq!(tree_digest(dir.path()).unwrap(), original);
    // A receipt anywhere but the root is package content.
    write(dir.path(), &format!("bin/{RECEIPT}"), "{}");
    assert_ne!(tree_digest(dir.path()).unwrap(), original);
    std::fs::remove_file(dir.path().join("bin").join(RECEIPT)).unwrap();

    write(dir.path(), "bin/codex", "changed");
    assert_ne!(tree_digest(dir.path()).unwrap(), original);
    write(dir.path(), "bin/codex", "codex");
    assert_eq!(tree_digest(dir.path()).unwrap(), original);

    std::fs::rename(dir.path().join("bin"), dir.path().join("lib")).unwrap();
    assert_ne!(tree_digest(dir.path()).unwrap(), original);
}

#[cfg(unix)]
#[test]
fn digest_tracks_permissions_and_rejects_links() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "bin/codex", "codex");
    let original = tree_digest(dir.path()).unwrap();
    std::fs::set_permissions(
        dir.path().join("bin/codex"),
        std::fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    assert_ne!(tree_digest(dir.path()).unwrap(), original);

    // The daemon's root alias is tolerated; any other link is not.
    std::os::unix::fs::symlink("bin/codex", dir.path().join("codex")).unwrap();
    let with_alias = tree_digest(dir.path()).unwrap();
    std::fs::remove_file(dir.path().join("codex")).unwrap();
    assert_eq!(tree_digest(dir.path()).unwrap(), with_alias);
    std::os::unix::fs::symlink("codex", dir.path().join("bin/alias")).unwrap();
    assert!(
        tree_digest(dir.path())
            .unwrap_err()
            .to_string()
            .contains("package contains a link")
    );
}
