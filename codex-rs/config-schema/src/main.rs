//! Generates the canonical config schema fixture for development and releases.

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

/// Generate shared and fork-overlay JSON Schemas in the output directory.
#[derive(Parser)]
#[command(name = "codex-write-config-schema")]
struct Args {
    #[arg(short, long, value_name = "PATH")]
    out: Option<PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let out_path = args.out.unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../core/config.schema.json")
    });
    codex_config::schema::write_config_schema(&out_path)?;
    codex_config::schema::write_config_overlay_schema(
        &out_path.with_file_name("config-overlay.schema.json"),
    )?;
    Ok(())
}
