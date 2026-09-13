/// The current Codex CLI version as embedded at compile time.
#[cfg(not(test))]
pub const CODEX_CLI_VERSION: &str = env!("CARGO_PKG_VERSION");

// Stabilize layout before rendering snapshots; replacing the version in rendered
// text leaves padding dependent on the package version used for the test build.
#[cfg(test)]
pub const CODEX_CLI_VERSION: &str = "0.0.0";
