use super::*;
use pretty_assertions::assert_eq;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::sync::Mutex;

#[test]
fn shallow_entries_are_emitted_before_recursive_entries_without_duplicates() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    fs::write(root.join("direct.txt"), "direct").unwrap();
    fs::create_dir_all(root.join("direct-dir/nested-dir")).unwrap();
    fs::write(root.join("direct-dir/nested.txt"), "nested").unwrap();
    fs::write(root.join("direct-dir/nested-dir/deep.txt"), "deep").unwrap();

    let visited = Arc::new(Mutex::new(Vec::new()));
    run_phased_walk(
        WalkConfig {
            search_directories: &[root.to_path_buf()],
            threads: 2,
            respect_gitignore: true,
            override_matcher: None,
        },
        || false,
        || {
            let visited = visited.clone();
            Box::new(move |entry| {
                if let Ok(entry) = entry
                    && let Ok(relative_path) = entry.path().strip_prefix(root)
                {
                    visited.lock().unwrap().push(relative_path.to_path_buf());
                }
                WalkState::Continue
            })
        },
    );

    let visited = visited.lock().unwrap();
    let first_recursive = visited
        .iter()
        .position(|path| path.components().count() >= 2)
        .unwrap();
    assert!(
        visited[..first_recursive]
            .iter()
            .all(|path| path.components().count() <= 1)
    );
    assert!(
        visited[first_recursive..]
            .iter()
            .all(|path| path.components().count() >= 2)
    );

    let unique = visited.iter().cloned().collect::<BTreeSet<_>>();
    assert_eq!(visited.len(), unique.len());
    assert_eq!(
        unique,
        BTreeSet::from([
            Path::new("").to_path_buf(),
            Path::new("direct-dir").to_path_buf(),
            Path::new("direct-dir/nested-dir").to_path_buf(),
            Path::new("direct-dir/nested-dir/deep.txt").to_path_buf(),
            Path::new("direct-dir/nested.txt").to_path_buf(),
            Path::new("direct.txt").to_path_buf(),
        ])
    );
}
