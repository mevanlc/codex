use super::*;
use codex_file_search::MatchType;
use pretty_assertions::assert_eq;

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
