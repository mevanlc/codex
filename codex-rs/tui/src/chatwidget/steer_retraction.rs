//! Pending-steer identity and retraction behavior for `ChatWidget`.

use super::*;

impl ChatWidget {
    pub(super) fn request_latest_pending_steer_retraction(&mut self) {
        if self
            .input_queue
            .pending_steer_retraction_in_flight
            .is_some()
        {
            return;
        }
        let Some(pending) = self.input_queue.pending_steers.back() else {
            return;
        };
        let Some(expected_turn_id) = pending.turn_id.clone() else {
            return;
        };
        let client_user_message_id = pending.client_id.clone();
        self.input_queue.pending_steer_retraction_in_flight = Some(client_user_message_id.clone());
        if !self.submit_op(AppCommand::retract_steer(
            expected_turn_id,
            client_user_message_id,
        )) {
            self.input_queue.pending_steer_retraction_in_flight = None;
        }
    }

    pub(crate) fn mark_pending_steer_accepted(&mut self, client_id: &str, turn_id: String) {
        let Some(pending) = self
            .input_queue
            .pending_steers
            .iter_mut()
            .find(|pending| pending.client_id == client_id)
        else {
            tracing::warn!(
                client_id,
                "accepted steer was not present in the pending queue"
            );
            return;
        };
        pending.turn_id = Some(turn_id);
    }

    pub(crate) fn on_pending_steer_retraction_result(
        &mut self,
        client_id: &str,
        status: codex_app_server_protocol::TurnRetractStatus,
    ) {
        if self
            .input_queue
            .pending_steer_retraction_in_flight
            .as_deref()
            == Some(client_id)
        {
            self.input_queue.pending_steer_retraction_in_flight = None;
        }

        match status {
            codex_app_server_protocol::TurnRetractStatus::Retracted => {
                let Some(position) = self
                    .input_queue
                    .pending_steers
                    .iter()
                    .position(|pending| pending.client_id == client_id)
                else {
                    tracing::warn!(
                        client_id,
                        "retracted steer was not present in the pending queue"
                    );
                    return;
                };
                let Some(pending) = self.input_queue.pending_steers.remove(position) else {
                    return;
                };
                self.restore_user_message_to_composer(user_message_for_restore(
                    pending.user_message,
                    &pending.history_record,
                ));
                self.refresh_pending_input_preview();
                self.request_redraw();
            }
            codex_app_server_protocol::TurnRetractStatus::NotPending => {
                self.add_warning_message(
                    "Message was already submitted and can no longer be edited.".to_string(),
                );
            }
            codex_app_server_protocol::TurnRetractStatus::NotRetractable => {
                self.add_warning_message("This steer message cannot be retracted.".to_string());
            }
        }
    }

    pub(crate) fn on_pending_steer_retraction_failed(&mut self, client_id: &str, error: String) {
        if self
            .input_queue
            .pending_steer_retraction_in_flight
            .as_deref()
            == Some(client_id)
        {
            self.input_queue.pending_steer_retraction_in_flight = None;
        }
        self.add_error_message(error);
    }
}
