use super::IndexedEntry;
use super::MatchType;
use super::SessionInner;
use super::WorkSignal;
use super::get_file_path;
use ignore::DirEntry;
use ignore::Error;
use ignore::WalkBuilder;
use ignore::WalkParallel;
use ignore::WalkState;
use ignore::overrides::Override;
use nucleo::Injector;
use nucleo::Utf32String;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;

#[cfg(test)]
#[path = "walker_tests.rs"]
mod tests;

#[derive(Clone, Copy)]
enum WalkPhase {
    Shallow,
    Recursive,
}

impl WalkPhase {
    fn configure(self, builder: &mut WalkBuilder) {
        match self {
            Self::Shallow => {
                builder.max_depth(Some(1));
            }
            Self::Recursive => {
                builder.min_depth(Some(2));
            }
        }
    }
}

#[derive(Clone, Copy)]
struct WalkConfig<'a> {
    search_directories: &'a [PathBuf],
    threads: usize,
    respect_gitignore: bool,
    override_matcher: Option<&'a Override>,
}

type WalkVisitor<'a> = Box<dyn FnMut(Result<DirEntry, Error>) -> WalkState + Send + 'a>;

/// Walks the search directories in disjoint shallow and recursive phases.
///
/// The shallow phase emits the roots and their direct children before the
/// recursive phase emits entries at depth two and below. Both phases use the
/// same ignore configuration, and their depth ranges do not overlap.
fn run_phased_walk<'a>(
    config: WalkConfig<'_>,
    should_stop: impl Fn() -> bool,
    mut visitor_builder: impl FnMut() -> WalkVisitor<'a>,
) {
    for phase in [WalkPhase::Shallow, WalkPhase::Recursive] {
        if should_stop() {
            break;
        }
        let Some(walker) = build_walker(config, phase) else {
            break;
        };
        walker.run(&mut visitor_builder);
    }
}

/// Builds one phase of the filesystem walk.
///
/// `require_git(true)` matches git's own ignore semantics: git never reads
/// `.gitignore` files from directories above the repository root. Without this
/// flag, a broad parent ignore (for example, `~/.gitignore` containing `*`)
/// could silently suppress every file in the walk.
fn build_walker(config: WalkConfig<'_>, phase: WalkPhase) -> Option<WalkParallel> {
    let (first_root, additional_roots) = config.search_directories.split_first()?;
    let mut builder = WalkBuilder::new(first_root);
    for root in additional_roots {
        builder.add(root);
    }
    builder
        .threads(config.threads)
        // Allow hidden entries.
        .hidden(false)
        // Follow symlinks to search their contents.
        .follow_links(true)
        // Only apply gitignore rules when a git context exists.
        .require_git(true);
    if !config.respect_gitignore {
        builder
            .git_ignore(false)
            .git_global(false)
            .git_exclude(false)
            .ignore(false)
            .parents(false);
    }
    if let Some(override_matcher) = config.override_matcher {
        builder.overrides(override_matcher.clone());
    }
    phase.configure(&mut builder);
    Some(builder.build_parallel())
}

pub(super) fn worker(
    inner: Arc<SessionInner>,
    override_matcher: Option<Override>,
    injector: Injector<IndexedEntry>,
) {
    let config = WalkConfig {
        search_directories: &inner.search_directories,
        threads: inner.threads,
        respect_gitignore: inner.respect_gitignore,
        override_matcher: override_matcher.as_ref(),
    };
    let cancelled = inner.cancelled.clone();
    let shutdown = inner.shutdown.clone();

    run_phased_walk(
        config,
        || cancelled.load(Ordering::Relaxed) || shutdown.load(Ordering::Relaxed),
        || {
            const CHECK_INTERVAL: usize = 1024;
            let mut n = 0;
            let search_directories = inner.search_directories.clone();
            let injector = injector.clone();
            let cancelled = inner.cancelled.clone();
            let shutdown = inner.shutdown.clone();

            Box::new(move |entry| {
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(_) => return WalkState::Continue,
                };
                let path = entry.path();
                let Some(full_path) = path.to_str() else {
                    return WalkState::Continue;
                };
                if let Some((_, relative_path)) = get_file_path(path, &search_directories) {
                    let match_type = match entry.file_type() {
                        Some(file_type) if file_type.is_dir() => MatchType::Directory,
                        _ => MatchType::File,
                    };
                    injector.push(
                        IndexedEntry {
                            full_path: Arc::from(full_path),
                            match_type,
                        },
                        |_, cols| {
                            cols[0] = Utf32String::from(relative_path);
                        },
                    );
                }
                n += 1;
                if n >= CHECK_INTERVAL {
                    if cancelled.load(Ordering::Relaxed) || shutdown.load(Ordering::Relaxed) {
                        return WalkState::Quit;
                    }
                    n = 0;
                }
                WalkState::Continue
            })
        },
    );
    let _ = inner.work_tx.send(WorkSignal::WalkComplete);
}
