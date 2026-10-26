use std::path::PathBuf;
use std::process::ExitCode;

use clap::ArgGroup;
use clap::Parser;
use clap::builder::NonEmptyStringValueParser;
use codex_fork_installer::Cancel;
use codex_fork_installer::Cancelled;
use codex_fork_installer::Environment;
use codex_fork_installer::Mode;
use codex_fork_installer::Request;

/// Install complete mevanlc/codex packages into ~/.local/share/mevanlc-codex;
/// commands go in ~/.local/bin, which must be on PATH.
#[derive(Debug, Parser)]
#[command(name = "codex-fork-installer", group(ArgGroup::new("source").args(["release", "artifact", "archive"])))]
struct Cli {
    /// Newest published non-prerelease carrying this platform's package (default).
    #[arg(long)]
    release: bool,

    /// Newest non-expired Actions artifact for this platform; needs GH_TOKEN or `gh auth login`.
    #[arg(long)]
    artifact: bool,

    /// Install a local package archive (.tar.zst or .zip); like --version, this pins it.
    #[arg(long, value_name = "PATH")]
    archive: Option<PathBuf>,

    /// Install even when the selected package is already as new or newer.
    #[arg(long)]
    force: bool,

    /// Exact tag or bare fork version: a deliberate pin that permits downgrades.
    #[arg(
        long,
        value_name = "TAG_OR_VERSION",
        value_parser = NonEmptyStringValueParser::new(),
        conflicts_with_all = ["artifact", "archive", "list_available"]
    )]
    version: Option<String>,

    /// List releases carrying this platform's package and mark the default.
    #[arg(long, conflicts_with_all = ["artifact", "archive"])]
    list_available: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let request = Request {
        mode: match cli.archive {
            Some(path) => Mode::Archive(path),
            None if cli.artifact => Mode::Artifact,
            None => Mode::Release,
        },
        version: cli.version,
        list_available: cli.list_available,
        force: cli.force,
    };
    let cancel = Cancel::default();
    let result = cancel
        .install_handlers()
        .map_err(anyhow::Error::from)
        .and_then(|()| Environment::from_process())
        .and_then(|environment| codex_fork_installer::run(&request, &environment, &cancel));
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            if let Some(cancelled) = error.downcast_ref::<Cancelled>() {
                eprintln!("error: {error:#}");
                return ExitCode::from(cancelled.exit_code);
            }
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        }
    }
}
