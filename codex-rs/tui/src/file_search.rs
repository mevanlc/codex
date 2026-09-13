//! Session-based orchestration for `@` file searches.
//!
//! `ChatComposer` publishes every change of the `@token` as a
//! [`FileSearchRequest`]. This manager owns a single `codex-file-search`
//! session for the current search root, updates the query on every keystroke,
//! and drops the session when the query becomes empty. Opt-in explicit path
//! searches derive their root from `/`, `~/`, `./`, or `../` syntax while retaining
//! the user's lexical path prefix in results.

use codex_file_search as file_search;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;

mod explicit_paths;
use explicit_paths::PreparedFileSearch;
use explicit_paths::prepare_file_search;
use explicit_paths::prepare_match;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FileSearchRequest {
    pub(crate) query: String,
    pub(crate) allow_explicit_paths: bool,
    pub(crate) scope: FileSearchScope,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FileSearchScope {
    Standard,
    All,
}

impl FileSearchScope {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::All => "all",
        }
    }
}

pub(crate) struct FileSearchManager {
    state: Arc<Mutex<SearchState>>,
    search_dir: PathBuf,
    home_dir: Option<PathBuf>,
    app_tx: AppEventSender,
}

struct SearchState {
    latest_request: Option<FileSearchRequest>,
    active_search: Option<PreparedFileSearch>,
    session: Option<file_search::FileSearchSession>,
    session_token: usize,
}

impl FileSearchManager {
    pub fn new(search_dir: PathBuf, tx: AppEventSender) -> Self {
        Self::new_with_home_dir(search_dir, dirs::home_dir(), tx)
    }

    fn new_with_home_dir(
        search_dir: PathBuf,
        home_dir: Option<PathBuf>,
        tx: AppEventSender,
    ) -> Self {
        Self {
            state: Arc::new(Mutex::new(SearchState {
                latest_request: None,
                active_search: None,
                session: None,
                session_token: 0,
            })),
            search_dir,
            home_dir,
            app_tx: tx,
        }
    }

    /// Updates the directory used for file searches.
    /// This should be called when the session's CWD changes on resume.
    /// Drops the current session so it will be recreated with the new directory on next query.
    pub fn update_search_dir(&mut self, new_dir: PathBuf) {
        self.search_dir = new_dir;
        #[expect(clippy::unwrap_used)]
        let mut st = self.state.lock().unwrap();
        st.session.take();
        st.latest_request = None;
        st.active_search = None;
    }

    /// Call whenever the user edits the `@` token.
    pub fn on_user_query(&self, request: FileSearchRequest) {
        #[expect(clippy::unwrap_used)]
        let mut st = self.state.lock().unwrap();
        if st.latest_request.as_ref() == Some(&request) {
            return;
        }

        let prepared = prepare_file_search(&self.search_dir, self.home_dir.as_deref(), &request);
        st.latest_request = Some(request);

        if prepared.display_query.is_empty() {
            st.session.take();
            st.active_search = Some(prepared);
            return;
        }

        if st.active_search.as_ref().is_none_or(|active| {
            active.search_dir != prepared.search_dir || active.scope != prepared.scope
        }) {
            st.session.take();
        }
        st.active_search = Some(prepared.clone());

        if st.session.is_none() {
            self.start_session_locked(&mut st, prepared.search_dir, prepared.scope);
        }
        if let Some(session) = st.session.as_ref() {
            session.update_query(&prepared.search_query);
        }
    }

    fn start_session_locked(
        &self,
        st: &mut SearchState,
        search_dir: PathBuf,
        scope: FileSearchScope,
    ) {
        st.session_token = st.session_token.wrapping_add(1);
        let session_token = st.session_token;
        let reporter = Arc::new(TuiSessionReporter {
            state: self.state.clone(),
            app_tx: self.app_tx.clone(),
            session_token,
        });
        let session = file_search::create_session(
            vec![search_dir],
            file_search::FileSearchOptions {
                compute_indices: true,
                respect_gitignore: scope == FileSearchScope::Standard,
                ..Default::default()
            },
            reporter,
            /*cancel_flag*/ None,
        );
        match session {
            Ok(session) => st.session = Some(session),
            Err(err) => {
                tracing::warn!("file search session failed to start: {err}");
                st.session = None;
            }
        }
    }
}

struct TuiSessionReporter {
    state: Arc<Mutex<SearchState>>,
    app_tx: AppEventSender,
    session_token: usize,
}

impl TuiSessionReporter {
    fn send_snapshot(&self, snapshot: &file_search::FileSearchSnapshot) {
        #[expect(clippy::unwrap_used)]
        let st = self.state.lock().unwrap();
        let Some(active) = st.active_search.as_ref() else {
            return;
        };
        if st.session_token != self.session_token
            || active.display_query.is_empty()
            || snapshot.query != active.search_query
        {
            return;
        }
        let query = active.display_query.clone();
        let scope = active.scope;
        let matches = snapshot
            .matches
            .iter()
            .cloned()
            .map(|matched| prepare_match(active, matched))
            .collect();
        drop(st);
        self.app_tx.send(AppEvent::FileSearchResult {
            query,
            scope,
            matches,
        });
    }
}

impl file_search::SessionReporter for TuiSessionReporter {
    fn on_update(&self, snapshot: &file_search::FileSearchSnapshot) {
        self.send_snapshot(snapshot);
    }

    fn on_complete(&self) {}
}

#[cfg(test)]
#[path = "file_search_tests.rs"]
mod tests;
