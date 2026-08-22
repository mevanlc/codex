use super::*;
use pretty_assertions::assert_eq;
use std::path::PathBuf;

fn file_match(score: u32, path: &str) -> FileMatch {
    FileMatch {
        score,
        path: PathBuf::from(path),
        match_type: MatchType::File,
        root: PathBuf::from("/workspace"),
        indices: None,
    }
}

#[test]
fn filesystem_rows_use_depth_to_break_equal_score_ties() {
    let matches = [
        file_match(128, "~/.config/nested.json"),
        file_match(128, "~/z-direct.json"),
        file_match(128, "~/a-direct.json"),
    ];

    let rows = filtered_candidates(
        &[],
        &matches,
        ".json",
        SearchMode::Filesystem,
        /*show_file_matches*/ true,
    );

    assert_eq!(
        rows.into_iter()
            .map(|row| row.display_name)
            .collect::<Vec<_>>(),
        vec![
            "~/a-direct.json".to_string(),
            "~/z-direct.json".to_string(),
            "~/.config/nested.json".to_string(),
        ]
    );
}

#[test]
fn filesystem_rows_keep_score_ahead_of_depth() {
    let matches = [file_match(71, "direct"), file_match(72, "sub/nested")];

    let rows = filtered_candidates(
        &[],
        &matches,
        "query",
        SearchMode::Filesystem,
        /*show_file_matches*/ true,
    );

    assert_eq!(
        rows.into_iter()
            .map(|row| row.display_name)
            .collect::<Vec<_>>(),
        vec!["sub/nested".to_string(), "direct".to_string()]
    );
}
