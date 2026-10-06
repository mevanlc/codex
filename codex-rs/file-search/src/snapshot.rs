//! Converts matcher snapshots without associating old results with a new query.

use crate::FileMatch;
use crate::FileSearchSnapshot;
use crate::IndexedEntry;
use crate::SessionInner;
use crate::ranked_snapshot_matches;
use nucleo::Matcher;
use nucleo::Nucleo;
use std::path::PathBuf;

pub(super) fn for_query(
    inner: &SessionInner,
    nucleo: &Nucleo<IndexedEntry>,
    query: &str,
    indices_matcher: &mut Option<Matcher>,
    walk_complete: bool,
) -> Option<FileSearchSnapshot> {
    let snapshot = nucleo.snapshot();
    let pattern = snapshot.pattern().column_pattern(0);
    // A tick can collect the previous query's results while the new query is
    // still running. Wait for a matching snapshot before using the new label.
    if pattern.atoms != nucleo.pattern.column_pattern(0).atoms {
        return None;
    }
    let limit = inner.limit.min(snapshot.matched_item_count() as usize);
    let matches = ranked_snapshot_matches(snapshot, &inner.search_directories, limit)
        .into_iter()
        .filter_map(|ranked_match| {
            let item = snapshot.get_item(ranked_match.match_.idx)?;
            let indices = if let Some(indices_matcher) = indices_matcher.as_mut() {
                let mut indices = Vec::<u32>::new();
                let haystack = item.matcher_columns[0].slice(..);
                let _ = pattern.indices(haystack, indices_matcher, &mut indices);
                indices.sort_unstable();
                indices.dedup();
                Some(indices)
            } else {
                None
            };
            Some(FileMatch {
                score: ranked_match.match_.score,
                path: PathBuf::from(ranked_match.relative_path),
                match_type: item.data.match_type,
                root: inner.search_directories[ranked_match.root_idx].clone(),
                indices,
            })
        })
        .collect();
    Some(FileSearchSnapshot {
        query: query.to_string(),
        matches,
        total_match_count: snapshot.matched_item_count() as usize,
        scanned_file_count: snapshot.item_count() as usize,
        walk_complete,
    })
}
