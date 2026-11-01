use std::env;
use std::path::PathBuf;

const INSTALLER_BIN: &str = "codex-fork-installer";
const INSTALLER_MANIFEST: &str = "codex-fork-installer.manifest";

fn main() -> Result<(), String> {
    println!("cargo:rerun-if-changed={INSTALLER_MANIFEST}");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return Ok(());
    }

    let manifest_dir = env::var_os("CARGO_MANIFEST_DIR")
        .ok_or_else(|| "CARGO_MANIFEST_DIR should be set for build scripts".to_string())?;
    let manifest_path = PathBuf::from(manifest_dir).join(INSTALLER_MANIFEST);
    let manifest_path = manifest_path.display();

    match (
        env::var("CARGO_CFG_TARGET_ENV").as_deref(),
        env::var("CARGO_CFG_TARGET_ABI").as_deref(),
    ) {
        (Ok("msvc"), _) => {
            println!("cargo:rustc-link-arg-bin={INSTALLER_BIN}=/MANIFEST:EMBED");
            println!("cargo:rustc-link-arg-bin={INSTALLER_BIN}=/MANIFESTINPUT:{manifest_path}");
        }
        (Ok("gnu"), Ok("llvm")) => {
            println!("cargo:rustc-link-arg-bin={INSTALLER_BIN}=-Wl,-Xlink=/manifest:embed");
            println!(
                "cargo:rustc-link-arg-bin={INSTALLER_BIN}=-Wl,-Xlink=/manifestinput:{manifest_path}"
            );
        }
        _ => {}
    }

    Ok(())
}
