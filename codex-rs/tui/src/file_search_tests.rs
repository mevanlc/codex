use super::*;
use codex_file_search::MatchType;
use pretty_assertions::assert_eq;
use std::fs;
use std::path::Path;
use std::time::Duration;
use tempfile::tempdir;
use tokio::sync::mpsc::unbounded_channel;
use tokio::time::timeout;

#[test]
fn explicit_path_queries_derive_search_roots_without_normalizing_the_prefix() {
    let base = PathBuf::from("/workspace/project");
    let queries = [
        "/absolute/path/to/a/file.txt",
        "../pd/ppd/file.txt",
        "./file.txt",
        "./../file.txt",
        "../././././dir/../.././../file.txt",
    ];

    let actual = queries.map(|query| {
        prepare_file_search(
            &base,
            /*home_dir*/ None,
            &FileSearchRequest {
                query: query.to_string(),
                allow_explicit_paths: true,
                scope: FileSearchScope::Standard,
            },
        )
    });

    assert_eq!(
        actual,
        [
            PreparedFileSearch {
                display_query: "/absolute/path/to/a/file.txt".to_string(),
                search_query: "file.txt".to_string(),
                search_dir: PathBuf::from("/absolute/path/to/a/"),
                path_prefix: "/absolute/path/to/a/".to_string(),
                result_root: base.clone(),
                scope: FileSearchScope::Standard,
            },
            PreparedFileSearch {
                display_query: "../pd/ppd/file.txt".to_string(),
                search_query: "file.txt".to_string(),
                search_dir: base.join("../pd/ppd/"),
                path_prefix: "../pd/ppd/".to_string(),
                result_root: base.clone(),
                scope: FileSearchScope::Standard,
            },
            PreparedFileSearch {
                display_query: "./file.txt".to_string(),
                search_query: "file.txt".to_string(),
                search_dir: base.join("./"),
                path_prefix: "./".to_string(),
                result_root: base.clone(),
                scope: FileSearchScope::Standard,
            },
            PreparedFileSearch {
                display_query: "./../file.txt".to_string(),
                search_query: "file.txt".to_string(),
                search_dir: base.join("./../"),
                path_prefix: "./../".to_string(),
                result_root: base.clone(),
                scope: FileSearchScope::Standard,
            },
            PreparedFileSearch {
                display_query: "../././././dir/../.././../file.txt".to_string(),
                search_query: "file.txt".to_string(),
                search_dir: base.join("../././././dir/../.././../"),
                path_prefix: "../././././dir/../.././../".to_string(),
                result_root: base,
                scope: FileSearchScope::Standard,
            },
        ]
    );
}

#[test]
fn ordinary_queries_keep_workspace_search_behavior_when_explicit_paths_are_enabled() {
    let base = PathBuf::from("/workspace/project");

    assert_eq!(
        prepare_file_search(
            &base,
            /*home_dir*/ None,
            &FileSearchRequest {
                query: "src/main.rs".to_string(),
                allow_explicit_paths: true,
                scope: FileSearchScope::Standard,
            },
        ),
        PreparedFileSearch {
            display_query: "src/main.rs".to_string(),
            search_query: "src/main.rs".to_string(),
            search_dir: base.clone(),
            path_prefix: String::new(),
            result_root: base,
            scope: FileSearchScope::Standard,
        }
    );
}

#[test]
fn tilde_path_query_expands_home_without_rewriting_the_prefix() {
    let cwd = PathBuf::from("/workspace/project");
    let home = PathBuf::from("/home/user");

    assert_eq!(
        prepare_file_search(
            &cwd,
            Some(&home),
            &FileSearchRequest {
                query: "~/notes/file.txt".to_string(),
                allow_explicit_paths: true,
                scope: FileSearchScope::Standard,
            },
        ),
        PreparedFileSearch {
            display_query: "~/notes/file.txt".to_string(),
            search_query: "file.txt".to_string(),
            search_dir: home.join("notes/"),
            path_prefix: "~/notes/".to_string(),
            result_root: cwd,
            scope: FileSearchScope::Standard,
        }
    );
}

#[test]
fn explicit_path_matches_retain_the_typed_prefix_and_shift_match_indices() {
    let active = PreparedFileSearch {
        display_query: "../pd/ppd/file.txt".to_string(),
        search_query: "file.txt".to_string(),
        search_dir: PathBuf::from("/workspace/project/../pd/ppd/"),
        path_prefix: "../pd/ppd/".to_string(),
        result_root: PathBuf::from("/workspace/project"),
        scope: FileSearchScope::Standard,
    };
    let matched = file_search::FileMatch {
        score: 42,
        path: PathBuf::from("file.txt"),
        match_type: MatchType::File,
        root: active.search_dir.clone(),
        indices: Some(vec![0, 3]),
    };

    assert_eq!(
        prepare_match(&active, matched),
        file_search::FileMatch {
            score: 42,
            path: PathBuf::from("../pd/ppd/file.txt"),
            match_type: MatchType::File,
            root: PathBuf::from("/workspace/project"),
            indices: Some(vec![10, 13]),
        }
    );
}

#[tokio::test]
async fn explicit_directory_query_emits_results_for_an_empty_basename() {
    let root = tempdir().unwrap();
    let cwd = root.path().join("additude");
    fs::create_dir(&cwd).unwrap();
    fs::write(root.path().join(".hidden-ignored-target"), "ignored").unwrap();

    let (tx, mut rx) = unbounded_channel();
    let manager = FileSearchManager::new(cwd, AppEventSender::new(tx));
    manager.on_user_query(FileSearchRequest {
        query: "../".to_string(),
        allow_explicit_paths: true,
        scope: FileSearchScope::All,
    });

    let matches = timeout(Duration::from_secs(5), async {
        loop {
            let event = rx.recv().await.expect("file search event");
            if let AppEvent::FileSearchResult {
                query,
                scope,
                matches,
            } = event
                && !matches.is_empty()
            {
                assert_eq!(query, "../");
                assert_eq!(scope, FileSearchScope::All);
                break matches;
            }
        }
    })
    .await
    .expect("explicit directory search result");

    assert!(
        matches
            .iter()
            .any(|matched| matched.path.as_path() == Path::new("../.hidden-ignored-target"))
    );
}

#[tokio::test]
async fn tilde_directory_query_emits_home_directory_results() {
    let root = tempdir().unwrap();
    let cwd = root.path().join("workspace");
    let home = root.path().join("home");
    fs::create_dir(&cwd).unwrap();
    fs::create_dir(&home).unwrap();
    fs::write(home.join(".hidden-home-target"), "ignored").unwrap();

    let (tx, mut rx) = unbounded_channel();
    let manager = FileSearchManager::new_with_home_dir(cwd, Some(home), AppEventSender::new(tx));
    manager.on_user_query(FileSearchRequest {
        query: "~/".to_string(),
        allow_explicit_paths: true,
        scope: FileSearchScope::All,
    });

    let matches = timeout(Duration::from_secs(5), async {
        loop {
            let event = rx.recv().await.expect("file search event");
            if let AppEvent::FileSearchResult {
                query,
                scope,
                matches,
            } = event
                && !matches.is_empty()
            {
                assert_eq!(query, "~/");
                assert_eq!(scope, FileSearchScope::All);
                break matches;
            }
        }
    })
    .await
    .expect("tilde directory search result");

    assert!(
        matches
            .iter()
            .any(|matched| matched.path.as_path() == Path::new("~/.hidden-home-target"))
    );
}
