//! Fork-specific explicit-path query preparation and result projection.

use super::FileSearchRequest;
use super::FileSearchScope;
use codex_file_search as file_search;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PreparedFileSearch {
    pub(super) display_query: String,
    pub(super) search_query: String,
    pub(super) search_dir: PathBuf,
    path_prefix: String,
    result_root: PathBuf,
    pub(super) scope: FileSearchScope,
}

pub(super) fn prepare_file_search(
    search_dir: &std::path::Path,
    home_dir: Option<&std::path::Path>,
    request: &FileSearchRequest,
) -> PreparedFileSearch {
    let explicit_path = request.allow_explicit_paths
        && (request.query.starts_with('/')
            || request.query.starts_with("~/") && home_dir.is_some()
            || request.query.starts_with("./")
            || request.query.starts_with("../"));
    let Some((directory, search_query)) = explicit_path
        .then(|| request.query.rsplit_once('/'))
        .flatten()
    else {
        return PreparedFileSearch {
            display_query: request.query.clone(),
            search_query: request.query.clone(),
            search_dir: search_dir.to_path_buf(),
            path_prefix: String::new(),
            result_root: search_dir.to_path_buf(),
            scope: request.scope,
        };
    };

    let path_prefix = format!("{directory}/");
    let explicit_search_dir = if let Some(home_relative_path) = path_prefix.strip_prefix("~/")
        && let Some(home_dir) = home_dir
    {
        home_dir.join(home_relative_path)
    } else if path_prefix.starts_with('/') {
        PathBuf::from(&path_prefix)
    } else {
        search_dir.join(&path_prefix)
    };
    PreparedFileSearch {
        display_query: request.query.clone(),
        search_query: search_query.to_string(),
        search_dir: explicit_search_dir,
        path_prefix,
        result_root: search_dir.to_path_buf(),
        scope: request.scope,
    }
}

pub(super) fn prepare_match(
    active: &PreparedFileSearch,
    mut matched: file_search::FileMatch,
) -> file_search::FileMatch {
    if active.path_prefix.is_empty() {
        return matched;
    }

    matched.path = PathBuf::from(&active.path_prefix).join(matched.path);
    matched.root = active.result_root.clone();
    let index_offset = u32::try_from(active.path_prefix.chars().count()).unwrap_or(u32::MAX);
    if let Some(indices) = matched.indices.as_mut() {
        for index in indices {
            *index = index.saturating_add(index_offset);
        }
    }
    matched
}

#[cfg(test)]
#[path = "explicit_paths_tests.rs"]
mod tests;
