use super::*;
use pretty_assertions::assert_eq;
use std::fs;
use std::path::Path;
use std::time::Duration;
use tempfile::tempdir;
use tokio::sync::mpsc::unbounded_channel;
use tokio::time::timeout;

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
