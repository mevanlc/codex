use anyhow::Result;
use app_test_support::TestAppServer;
use app_test_support::to_response;
use app_test_support::write_mock_responses_config_toml_with_chatgpt_base_url;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnRetractParams;
use codex_app_server_protocol::TurnRetractResponse;
use codex_app_server_protocol::TurnRetractStatus;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::TurnSteerParams;
use codex_app_server_protocol::UserInput as V2UserInput;
use core_test_support::responses;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::sync::oneshot;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test]
async fn turn_retract_removes_a_pending_steer() -> Result<()> {
    let (release_tx, release_rx) = oneshot::channel();
    let (server, mut completions) = start_streaming_sse_server(vec![vec![
        StreamingSseChunk {
            gate: None,
            body: responses::sse(vec![responses::ev_response_created("resp-1")]),
        },
        StreamingSseChunk {
            gate: Some(release_rx),
            body: responses::sse(vec![responses::ev_completed("resp-1")]),
        },
    ]])
    .await;

    let codex_home = TempDir::new()?;
    write_mock_responses_config_toml_with_chatgpt_base_url(
        codex_home.path(),
        server.uri(),
        server.uri(),
    )?;
    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_managed_config()
        .build()
        .await?;
    timeout(DEFAULT_READ_TIMEOUT, app_server.initialize()).await??;

    let thread_request_id = app_server
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let thread_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(thread_request_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response(thread_response)?;

    let turn_request_id = app_server
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "first prompt".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let turn_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(turn_request_id)),
    )
    .await??;
    let TurnStartResponse { turn } = to_response(turn_response)?;
    server.wait_for_request_count(1).await;

    let client_user_message_id = "client-steer-message-1".to_string();
    let steer_request_id = app_server
        .send_turn_steer_request(TurnSteerParams {
            thread_id: thread.id.clone(),
            client_user_message_id: Some(client_user_message_id.clone()),
            input: vec![V2UserInput::Text {
                text: "retracted prompt".to_string(),
                text_elements: Vec::new(),
            }],
            responsesapi_client_metadata: None,
            additional_context: None,
            expected_turn_id: turn.id.clone(),
        })
        .await?;
    let _: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(steer_request_id)),
    )
    .await??;

    let retract_params = TurnRetractParams {
        thread_id: thread.id.clone(),
        expected_turn_id: turn.id.clone(),
        client_user_message_id,
    };
    let retract_request_id = app_server
        .send_turn_retract_request(retract_params.clone())
        .await?;
    let retract_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(retract_request_id)),
    )
    .await??;
    assert_eq!(
        to_response::<TurnRetractResponse>(retract_response)?,
        TurnRetractResponse {
            status: TurnRetractStatus::Retracted,
        }
    );

    let second_retract_request_id = app_server.send_turn_retract_request(retract_params).await?;
    let second_retract_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        app_server
            .read_stream_until_response_message(RequestId::Integer(second_retract_request_id)),
    )
    .await??;
    assert_eq!(
        to_response::<TurnRetractResponse>(second_retract_response)?,
        TurnRetractResponse {
            status: TurnRetractStatus::NotPending,
        }
    );

    release_tx.send(()).expect("release response stream");
    timeout(DEFAULT_READ_TIMEOUT, completions.remove(0)).await??;
    let _turn_completed = timeout(
        DEFAULT_READ_TIMEOUT,
        app_server.read_stream_until_notification_message("turn/completed"),
    )
    .await??;
    assert_eq!(server.requests().await.len(), 1);
    server.shutdown().await;

    Ok(())
}
