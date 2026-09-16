//! Fork-only pending-steer retraction coverage, using the upstream session fixtures.

use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn retract_steer_removes_matching_pending_input() {
    let (sess, tc, _rx) = make_session_and_context_with_rx().await;
    sess.spawn_task(
        Arc::clone(&tc),
        Vec::new(),
        NeverEndingTask {
            kind: TaskKind::Regular,
            listen_to_cancellation_token: true,
        },
    )
    .await;

    crate::session::turn_input::handle(
        &sess,
        TurnInputRequest::new(SubmittedTurnInput::UserInput {
            content: vec![UserInput::Text {
                text: "retract me".to_string(),
                text_elements: Vec::new(),
            }],
            client_id: Some("client-message-1".to_string()),
        }),
        TurnInputMode::Steer {
            expected_turn_id: tc.sub_id.clone(),
        },
        "test-submission".to_string(),
    )
    .await
    .expect("steer should be accepted");

    assert_eq!(
        sess.retract_steer(&tc.sub_id, "client-message-1").await,
        RetractSteerStatus::Retracted
    );
    assert_eq!(
        (sess.input_queue.get_pending_input(&sess.active_turn).await).0,
        Vec::<TurnInput>::new()
    );

    sess.abort_all_tasks(TurnAbortReason::Interrupted).await;
}

#[tokio::test]
async fn retract_steer_does_not_remove_input_after_drain_or_from_another_turn() {
    let (sess, tc, _rx) = make_session_and_context_with_rx().await;
    sess.spawn_task(
        Arc::clone(&tc),
        Vec::new(),
        NeverEndingTask {
            kind: TaskKind::Regular,
            listen_to_cancellation_token: true,
        },
    )
    .await;

    crate::session::turn_input::handle(
        &sess,
        TurnInputRequest::new(SubmittedTurnInput::UserInput {
            content: vec![UserInput::Text {
                text: "keep me".to_string(),
                text_elements: Vec::new(),
            }],
            client_id: Some("client-message-1".to_string()),
        }),
        TurnInputMode::Steer {
            expected_turn_id: tc.sub_id.clone(),
        },
        "test-submission".to_string(),
    )
    .await
    .expect("steer should be accepted");

    assert_eq!(
        sess.retract_steer("another-turn", "client-message-1").await,
        RetractSteerStatus::NotPending
    );
    assert_eq!(
        (sess.input_queue.get_pending_input(&sess.active_turn).await).0,
        vec![TurnInput::UserInput {
            content: vec![UserInput::Text {
                text: "keep me".to_string(),
                text_elements: Vec::new(),
            }],
            client_id: Some("client-message-1".to_string()),
            retractable: true,
            acceptance_order: None,
        }]
    );
    assert_eq!(
        sess.retract_steer(&tc.sub_id, "client-message-1").await,
        RetractSteerStatus::NotPending
    );

    sess.abort_all_tasks(TurnAbortReason::Interrupted).await;
}

#[tokio::test]
async fn retract_steer_rejects_steers_with_side_effects() {
    let (sess, tc, _rx) = make_session_and_context_with_rx().await;
    sess.spawn_task(
        Arc::clone(&tc),
        Vec::new(),
        NeverEndingTask {
            kind: TaskKind::Regular,
            listen_to_cancellation_token: true,
        },
    )
    .await;

    let mut request = TurnInputRequest::new(SubmittedTurnInput::UserInput {
        content: vec![UserInput::Text {
            text: "not retractable".to_string(),
            text_elements: Vec::new(),
        }],
        client_id: Some("client-message-1".to_string()),
    });
    request.responsesapi_client_metadata = Some(std::collections::HashMap::from([(
        "source".to_string(),
        "side-effect-test".to_string(),
    )]));
    crate::session::turn_input::handle(
        &sess,
        request,
        TurnInputMode::Steer {
            expected_turn_id: tc.sub_id.clone(),
        },
        "test-submission".to_string(),
    )
    .await
    .expect("steer should be accepted");

    assert_eq!(
        sess.retract_steer(&tc.sub_id, "client-message-1").await,
        RetractSteerStatus::NotRetractable
    );
    assert!(sess.input_queue.has_pending_input(&sess.active_turn).await);

    sess.abort_all_tasks(TurnAbortReason::Interrupted).await;
}
