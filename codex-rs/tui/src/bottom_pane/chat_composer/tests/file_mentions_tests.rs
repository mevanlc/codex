//! Fork-only file mention and completion behavior, using the upstream composer fixtures.

use super::*;
use pretty_assertions::assert_eq;

#[test]
fn filesystem_all_unified_mention_popup_snapshot() {
    snapshot_composer_state(
        "filesystem_all_unified_mention_popup",
        /*enhanced_keys_supported*/ false,
        |composer| {
            composer.set_mentions_v2_enabled(/*enabled*/ true);
            composer.set_text_content("@hidden-target".to_string(), Vec::new(), Vec::new());
            let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
            let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
            composer.on_file_search_result(
                "hidden-target".to_string(),
                FileSearchScope::All,
                vec![FileMatch {
                    score: 42,
                    path: PathBuf::from("ignored/.hidden-target"),
                    match_type: codex_file_search::MatchType::File,
                    root: PathBuf::from("/workspace/project"),
                    indices: None,
                }],
            );
        },
    );
}

#[test]
fn filesystem_all_explicit_directory_popup_snapshot() {
    snapshot_composer_state(
        "filesystem_all_explicit_directory_popup",
        /*enhanced_keys_supported*/ false,
        |composer| {
            composer.set_mentions_v2_enabled(/*enabled*/ true);
            composer.set_file_mentions_allow_explicit_paths(/*allow_explicit_paths*/ true);
            composer.set_text_content("@../".to_string(), Vec::new(), Vec::new());
            let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
            let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
            composer.on_file_search_result(
                "../".to_string(),
                FileSearchScope::All,
                vec![FileMatch {
                    score: 42,
                    path: PathBuf::from("../.hidden-ignored-target"),
                    match_type: codex_file_search::MatchType::File,
                    root: PathBuf::from("/workspace/project"),
                    indices: None,
                }],
            );
        },
    );
}

#[test]
fn directory_tab_continues_explicit_path_completion_snapshot() {
    snapshot_composer_state(
        "directory_tab_continues_explicit_path_completion",
        /*enhanced_keys_supported*/ false,
        |composer| {
            composer.set_mentions_v2_enabled(/*enabled*/ true);
            composer.set_file_mentions_allow_explicit_paths(/*allow_explicit_paths*/ true);
            composer.set_text_content("@/Appli".to_string(), Vec::new(), Vec::new());
            composer.on_file_search_result(
                "/Appli".to_string(),
                FileSearchScope::Standard,
                vec![FileMatch {
                    score: 42,
                    path: PathBuf::from("/Applications"),
                    match_type: MatchType::Directory,
                    root: PathBuf::from("/"),
                    indices: None,
                }],
            );
            let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        },
    );
}

#[test]
fn filesystem_all_home_directory_popup_snapshot() {
    snapshot_composer_state(
        "filesystem_all_home_directory_popup",
        /*enhanced_keys_supported*/ false,
        |composer| {
            composer.set_mentions_v2_enabled(/*enabled*/ true);
            composer.set_file_mentions_allow_explicit_paths(/*allow_explicit_paths*/ true);
            composer.set_text_content("@~/".to_string(), Vec::new(), Vec::new());
            let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
            let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
            composer.on_file_search_result(
                "~/".to_string(),
                FileSearchScope::All,
                vec![FileMatch {
                    score: 42,
                    path: PathBuf::from("~/.hidden-home-target"),
                    match_type: codex_file_search::MatchType::File,
                    root: PathBuf::from("/workspace/project"),
                    indices: None,
                }],
            );
        },
    );
}

#[test]
fn home_json_file_popup_prioritizes_immediate_children_snapshot() {
    snapshot_composer_state(
        "home_json_file_popup_prioritizes_immediate_children",
        /*enhanced_keys_supported*/ false,
        |composer| {
            composer.set_mentions_v2_enabled(/*enabled*/ true);
            composer.set_file_mentions_allow_explicit_paths(/*allow_explicit_paths*/ true);
            let query = "~/.json";
            composer.set_text_content(format!("@{query}"), Vec::new(), Vec::new());
            composer.on_file_search_result(
                query.to_string(),
                FileSearchScope::Standard,
                vec![
                    FileMatch {
                        score: 128,
                        path: PathBuf::from("~/.config/nested.json"),
                        match_type: MatchType::File,
                        root: PathBuf::from("/workspace/project"),
                        indices: None,
                    },
                    FileMatch {
                        score: 128,
                        path: PathBuf::from("~/z-direct.json"),
                        match_type: MatchType::File,
                        root: PathBuf::from("/workspace/project"),
                        indices: None,
                    },
                    FileMatch {
                        score: 128,
                        path: PathBuf::from("~/a-direct.json"),
                        match_type: MatchType::File,
                        root: PathBuf::from("/workspace/project"),
                        indices: None,
                    },
                ],
            );
        },
    );
}

#[test]
fn filesystem_all_results_do_not_leak_into_standard_search_modes() {
    let (mut composer, mut rx) = new_test_composer();
    composer.set_mentions_v2_enabled(/*enabled*/ true);
    composer.set_text_content("@needle".to_string(), Vec::new(), Vec::new());

    assert_eq!(
        std::iter::from_fn(|| rx.try_recv().ok())
            .filter_map(|event| match event {
                AppEvent::StartFileSearch(request) => Some(request),
                _ => None,
            })
            .collect::<Vec<_>>(),
        vec![
            FileSearchRequest {
                query: String::new(),
                allow_explicit_paths: false,
                scope: FileSearchScope::Standard,
            },
            FileSearchRequest {
                query: "needle".to_string(),
                allow_explicit_paths: false,
                scope: FileSearchScope::Standard,
            },
        ]
    );

    let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
    assert!(rx.try_recv().is_err());
    let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
    let AppEvent::StartFileSearch(expanded_request) =
        rx.try_recv().expect("expanded search request")
    else {
        panic!("expected expanded file search request");
    };
    assert_eq!(
        expanded_request,
        FileSearchRequest {
            query: "needle".to_string(),
            allow_explicit_paths: false,
            scope: FileSearchScope::All,
        }
    );

    composer.on_file_search_result(
        "needle".to_string(),
        FileSearchScope::Standard,
        vec![FileMatch {
            score: 10,
            path: PathBuf::from("standard-needle"),
            match_type: codex_file_search::MatchType::File,
            root: PathBuf::from("/workspace/project"),
            indices: None,
        }],
    );
    let ActivePopup::MentionV2(popup) = &composer.popups.active else {
        panic!("expected unified mention popup");
    };
    assert!(popup.selected().is_none());

    composer.on_file_search_result(
        "needle".to_string(),
        FileSearchScope::All,
        vec![FileMatch {
            score: 20,
            path: PathBuf::from("ignored/.hidden-needle"),
            match_type: codex_file_search::MatchType::File,
            root: PathBuf::from("/workspace/project"),
            indices: None,
        }],
    );
    let ActivePopup::MentionV2(popup) = &composer.popups.active else {
        panic!("expected unified mention popup");
    };
    assert!(matches!(
        popup.selected(),
        Some(MentionV2Selection::File { path, .. })
            if path.as_path() == Path::new("ignored/.hidden-needle")
    ));

    let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
    let AppEvent::StartFileSearch(standard_request) =
        rx.try_recv().expect("standard search request")
    else {
        panic!("expected standard file search request");
    };
    assert_eq!(
        standard_request,
        FileSearchRequest {
            query: "needle".to_string(),
            allow_explicit_paths: false,
            scope: FileSearchScope::Standard,
        }
    );
    let _ = composer.handle_key_event(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
    assert!(rx.try_recv().is_err());

    composer.on_file_search_result(
        "needle".to_string(),
        FileSearchScope::All,
        vec![FileMatch {
            score: 20,
            path: PathBuf::from("ignored/.hidden-needle"),
            match_type: codex_file_search::MatchType::File,
            root: PathBuf::from("/workspace/project"),
            indices: None,
        }],
    );
    let ActivePopup::MentionV2(popup) = &composer.popups.active else {
        panic!("expected unified mention popup");
    };
    assert!(popup.selected().is_none());

    composer.on_file_search_result(
        "needle".to_string(),
        FileSearchScope::Standard,
        vec![FileMatch {
            score: 10,
            path: PathBuf::from("standard-needle"),
            match_type: codex_file_search::MatchType::File,
            root: PathBuf::from("/workspace/project"),
            indices: None,
        }],
    );
    let ActivePopup::MentionV2(popup) = &composer.popups.active else {
        panic!("expected unified mention popup");
    };
    assert!(matches!(
        popup.selected(),
        Some(MentionV2Selection::File { path, .. })
            if path.as_path() == Path::new("standard-needle")
    ));
}

#[test]
fn explicit_path_file_popup_snapshot() {
    snapshot_composer_state(
        "explicit_path_file_popup",
        /*enhanced_keys_supported*/ false,
        |composer| {
            composer.set_mentions_v2_enabled(/*enabled*/ true);
            composer.set_file_mentions_allow_explicit_paths(/*allow_explicit_paths*/ true);
            let query = "../pd/ppd/file.txt";
            composer.set_text_content(format!("@{query}"), Vec::new(), Vec::new());
            composer.on_file_search_result(
                query.to_string(),
                FileSearchScope::Standard,
                vec![FileMatch {
                    score: 42,
                    path: PathBuf::from(query),
                    match_type: codex_file_search::MatchType::File,
                    root: PathBuf::from("/workspace/project"),
                    indices: Some(vec![10, 11, 12, 13, 14, 15, 16, 17]),
                }],
            );
        },
    );
}

#[test]
fn explicit_path_setting_is_forwarded_to_file_search() {
    let (mut composer, mut rx) = new_test_composer();
    composer.set_mentions_v2_enabled(/*enabled*/ true);
    composer.set_file_mentions_allow_explicit_paths(/*allow_explicit_paths*/ true);

    let query = "../././././dir/../.././../file.txt";
    composer.set_text_content(format!("@{query}"), Vec::new(), Vec::new());

    let requests = std::iter::from_fn(|| rx.try_recv().ok())
        .filter_map(|event| match event {
            AppEvent::StartFileSearch(request) => Some(request),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        requests,
        vec![
            FileSearchRequest {
                query: String::new(),
                allow_explicit_paths: true,
                scope: FileSearchScope::Standard,
            },
            FileSearchRequest {
                query: query.to_string(),
                allow_explicit_paths: true,
                scope: FileSearchScope::Standard,
            },
        ]
    );
}

#[test]
fn file_completion_can_preserve_at_in_submitted_text() {
    let (mut composer, _rx) = new_test_composer();
    composer.set_file_mentions_preserve_at(/*preserve_at*/ true);
    complete_file(
        &mut composer,
        "@prog",
        /*cursor*/ "@prog".len(),
        "prog",
        PathBuf::from("program"),
    );
    assert_eq!(composer.current_text(), "@program ");

    let (result, _needs_redraw) =
        composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(
        result,
        InputResult::Submitted {
            text: "@program".to_string(),
            text_elements: Vec::new(),
        }
    );
}

#[test]
fn tab_on_directory_continues_unified_explicit_path_completion() {
    let (mut composer, mut rx) = new_test_composer();
    composer.set_mentions_v2_enabled(/*enabled*/ true);
    composer.set_file_mentions_allow_explicit_paths(/*allow_explicit_paths*/ true);
    show_file_completion(
        &mut composer,
        "@/Appli",
        /*cursor*/ "@/Appli".len(),
        "/Appli",
        PathBuf::from("/Applications"),
        MatchType::Directory,
    );

    let _ = std::iter::from_fn(|| rx.try_recv().ok()).count();
    let (result, _needs_redraw) =
        composer.handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));

    assert_eq!(result, InputResult::None);
    assert_eq!(composer.current_text(), "@/Applications/");
    assert!(matches!(composer.popups.active, ActivePopup::MentionV2(_)));
    let requests = std::iter::from_fn(|| rx.try_recv().ok())
        .filter_map(|event| match event {
            AppEvent::StartFileSearch(request) => Some(request),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        requests.last(),
        Some(&FileSearchRequest {
            query: "/Applications/".to_string(),
            allow_explicit_paths: true,
            scope: FileSearchScope::Standard,
        })
    );
}

#[test]
fn tab_on_directory_continues_legacy_explicit_path_completion() {
    let (mut composer, _rx) = new_test_composer();
    composer.set_mentions_v2_enabled(/*enabled*/ false);
    composer.set_file_mentions_allow_explicit_paths(/*allow_explicit_paths*/ true);
    show_file_completion(
        &mut composer,
        "@/Appli",
        /*cursor*/ "@/Appli".len(),
        "/Appli",
        PathBuf::from("/Applications"),
        MatchType::Directory,
    );

    let (result, _needs_redraw) =
        composer.handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));

    assert_eq!(result, InputResult::None);
    assert_eq!(composer.current_text(), "@/Applications/");
    assert!(matches!(composer.popups.active, ActivePopup::File(_)));
}

#[test]
fn enter_on_directory_finishes_with_trailing_slash_and_honors_preserve_at() {
    for (preserve_at, expected) in [(false, "/Applications/ "), (true, "@/Applications/ ")] {
        let (mut composer, _rx) = new_test_composer();
        composer.set_mentions_v2_enabled(/*enabled*/ true);
        composer.set_file_mentions_preserve_at(preserve_at);
        composer.set_file_mentions_allow_explicit_paths(/*allow_explicit_paths*/ true);
        show_file_completion(
            &mut composer,
            "@/Appli",
            /*cursor*/ "@/Appli".len(),
            "/Appli",
            PathBuf::from("/Applications"),
            MatchType::Directory,
        );

        let (result, _needs_redraw) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_eq!(result, InputResult::None);
        assert_eq!(composer.current_text(), expected);
        assert!(matches!(composer.popups.active, ActivePopup::None));
    }
}

#[test]
fn explicit_path_forms_can_be_completed_without_losing_their_lexical_prefix() {
    let (mut composer, _rx) = new_test_composer();
    composer.set_mentions_v2_enabled(/*enabled*/ true);
    composer.set_file_mentions_preserve_at(/*preserve_at*/ true);
    composer.set_file_mentions_allow_explicit_paths(/*allow_explicit_paths*/ true);

    for path in [
        "/absolute/path/to/a/file.txt",
        "~/file.txt",
        "../pd/ppd/file.txt",
        "./file.txt",
        "./../file.txt",
        "../././././dir/../.././../file.txt",
    ] {
        let text = format!("@{path}");
        complete_file(
            &mut composer,
            &text,
            /*cursor*/ text.len(),
            path,
            PathBuf::from(path),
        );
        assert_eq!(composer.current_text(), format!("@{path} "));
    }
}
