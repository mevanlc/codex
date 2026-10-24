#[cfg(any(not(debug_assertions), test))]
use codex_install_context::InstallContext;
#[cfg(any(not(debug_assertions), test))]
use codex_install_context::InstallMethod;

/// Update action the CLI should perform after restoring the terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateAction {
    Daemon(DaemonUpdateSource),
    ForkPackage,
}

impl UpdateAction {
    #[cfg(any(not(debug_assertions), test))]
    pub(crate) fn from_install_context(context: &InstallContext) -> Option<Self> {
        // Even a fork launched via an old npm/Brew shim must never install OAI.
        (context.method != InstallMethod::Other || context.package_layout.is_some())
            .then_some(Self::ForkPackage)
    }

    pub fn command_args(self) -> (&'static str, &'static [&'static str]) {
        match self {
            Self::Daemon(source) => ("codex", source.command_args()),
            Self::ForkPackage => ("codex", &["update"]),
        }
    }

    pub fn command_str(self) -> String {
        let (command, args) = self.command_args();
        shlex::try_join(std::iter::once(command).chain(args.iter().copied()))
            .unwrap_or_else(|_| format!("{command} {}", args.join(" ")))
    }
}

#[cfg(not(debug_assertions))]
pub fn get_update_action() -> Option<UpdateAction> {
    UpdateAction::from_install_context(InstallContext::current())
}

/// Package source explicitly selected by the user in the daemon menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaemonUpdateSource {
    PublicStable,
    ThisCli,
}

impl DaemonUpdateSource {
    pub fn command_args(self) -> &'static [&'static str] {
        match self {
            Self::PublicStable => &["app-server", "daemon", "update"],
            Self::ThisCli => &["app-server", "daemon", "update", "--from-cli", "--yes"],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_packaged_channel_updates_from_the_fork() {
        for method in [
            InstallMethod::Npm,
            InstallMethod::Bun,
            InstallMethod::Pnpm,
            InstallMethod::VitePlus,
            InstallMethod::Brew,
        ] {
            assert_eq!(
                UpdateAction::from_install_context(&InstallContext {
                    method,
                    package_layout: None
                }),
                Some(UpdateAction::ForkPackage)
            );
        }
        assert_eq!(
            UpdateAction::from_install_context(&InstallContext {
                method: InstallMethod::Other,
                package_layout: None
            }),
            None
        );
    }
}
