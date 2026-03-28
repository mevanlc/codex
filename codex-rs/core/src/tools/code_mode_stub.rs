//! Stub replacement for the `code_mode` module when the `code-mode` Cargo
//! feature is disabled.  Provides the same public surface so the rest of
//! `codex-core` compiles without the `v8`/`codex-code-mode` dependency.

use std::sync::Arc;

use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use crate::tools::ToolRouter;
use crate::tools::context::SharedTurnDiffTracker;

/// No-op service when code-mode is compiled out.
pub(crate) struct CodeModeService;

impl CodeModeService {
    pub(crate) fn new() -> Self {
        Self
    }

    pub(crate) async fn shutdown(&self) -> Result<(), String> {
        Ok(())
    }

    pub(crate) fn start_turn_worker(
        &self,
        _session: &Arc<Session>,
        _turn: &Arc<TurnContext>,
        _router: Arc<ToolRouter>,
        _tracker: SharedTurnDiffTracker,
    ) -> Option<CodeModeTurnWorkerStub> {
        None
    }
}

/// Placeholder type so callers that bind the worker to a variable compile.
pub(crate) struct CodeModeTurnWorkerStub;
