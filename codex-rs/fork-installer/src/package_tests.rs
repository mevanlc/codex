use super::*;
use crate::test_support::FakeProbe;
use crate::test_support::Fixture;
use crate::test_support::PackageSpec;
use crate::test_support::write_archive;
use pretty_assertions::assert_eq;

#[test]
fn member_paths_cannot_escape_the_destination() {
    let destination = Path::new("dest");
    for name in ["../x", "/etc/passwd", "a/../../b", "a\\b", "C:x", "a/.."] {
        assert!(member_path(destination, name).is_err(), "{name}");
    }
    assert_eq!(
        member_path(destination, "./bin//codex").unwrap(),
        destination.join("bin").join("codex")
    );
}

#[test]
fn links_and_unknown_archive_types_are_rejected() {
    let fixture = Fixture::new();
    let cancel = Cancel::default();
    let zip = fixture.dir.path().join("links.zip");
    write_archive(
        &zip,
        &[("bin/codex".to_string(), b"target".to_vec(), 0o120777)],
    );
    let error = extract(&zip, &fixture.dir.path().join("zip"), &cancel).unwrap_err();
    assert!(
        error.to_string().contains("links are not supported"),
        "{error:#}"
    );

    let tar_path = fixture.dir.path().join("links.tar.zst");
    let mut builder = tar::Builder::new(
        zstd::stream::write::Encoder::new(std::fs::File::create(&tar_path).unwrap(), 3).unwrap(),
    );
    let mut header = tar::Header::new_gnu();
    header.set_entry_type(tar::EntryType::Symlink);
    header.set_size(0);
    builder
        .append_link(&mut header, "bin/codex", "/etc/passwd")
        .unwrap();
    builder.into_inner().unwrap().finish().unwrap();
    let error = extract(&tar_path, &fixture.dir.path().join("tar"), &cancel).unwrap_err();
    assert!(
        error.to_string().contains("unsupported package member"),
        "{error:#}"
    );

    let loose = fixture.dir.path().join("codex.zst");
    std::fs::write(&loose, b"binary").unwrap();
    let error = extract(&loose, &fixture.dir.path().join("loose"), &cancel).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("loose binaries are no longer supported")
    );
}

#[test]
fn package_versions_allow_fork_snapshots() {
    for version in [
        "0.164.0-2ff75a6",
        "0.164.0-alpha.6-a41d338",
        "1.2.3-0123456",
        "1.2.3+build.1",
    ] {
        assert!(is_package_version(version), "{version}");
    }
    for version in [
        "",
        "1.2",
        "v1.2.3",
        "1.2.3-",
        "1.2.3-a/b",
        "../../escape",
        "1.2.3+a+b",
    ] {
        assert!(!is_package_version(version), "{version}");
    }
}

#[test]
fn each_target_requires_its_platform_executables() {
    let fixture = Fixture::new();
    for triple in [
        "x86_64-unknown-linux-musl",
        "aarch64-apple-darwin",
        "aarch64-linux-android",
        "x86_64-pc-windows-msvc",
    ] {
        let target = Target::new(triple);
        let archive = fixture.package(&PackageSpec {
            target: target.clone(),
            ..PackageSpec::new("1.2.3")
        });
        let complete = fixture.dir.path().join(format!("{triple}-complete"));
        extract(&archive, &complete, &Cancel::default()).unwrap();
        assert_eq!(
            validate(&target, &complete, Some("1.2.3"), Some(&FakeProbe)).unwrap(),
            "1.2.3"
        );
        assert!(validate(&target, &complete, Some("1.2.4"), None).is_err());
        let platform_file = target.required_executables().pop().unwrap();
        let archive = fixture.package(&PackageSpec {
            target: target.clone(),
            missing: Some(platform_file.clone()),
            ..PackageSpec::new("1.2.3")
        });
        let incomplete = fixture.dir.path().join(format!("{triple}-incomplete"));
        extract(&archive, &incomplete, &Cancel::default()).unwrap();
        let error = validate(&target, &incomplete, None, None).unwrap_err();
        assert!(error.to_string().contains(&platform_file), "{error:#}");
    }
}
