use async_trait::async_trait;
use std::collections::BTreeMap;
use tokio::sync::Mutex;
use trading_agent_kis_client::journal::{
    BrokerCommandAck, BrokerCommandJournal, BrokerCommandRecord, BrokerCommandStatus,
};
use trading_agent_kis_client::{BrokerError, ClientOrderId, RequestId};

#[derive(Debug, Default)]
pub struct MemoryCommandJournal {
    commands: Mutex<BTreeMap<String, BrokerCommandRecord>>,
}

impl MemoryCommandJournal {
    pub fn new() -> Self {
        Self::default()
    }

    async fn transition(
        &self,
        client_order_id: &ClientOrderId,
        expected: impl FnOnce(BrokerCommandStatus) -> bool,
        transition: &str,
        update: impl FnOnce(&mut BrokerCommandRecord),
    ) -> Result<(), BrokerError> {
        let mut commands = self.commands.lock().await;
        let command = commands
            .get_mut(client_order_id.as_str())
            .ok_or_else(|| BrokerError::invariant("journal command was not found"))?;
        if !expected(command.last_known_state) {
            return Err(BrokerError::invariant(format!(
                "invalid journal state transition {transition} for command: {client_order_id}"
            )));
        }
        update(command);
        Ok(())
    }
}

#[async_trait]
impl BrokerCommandJournal for MemoryCommandJournal {
    async fn get_command(
        &self,
        client_order_id: &ClientOrderId,
    ) -> Result<Option<BrokerCommandRecord>, BrokerError> {
        Ok(self
            .commands
            .lock()
            .await
            .get(client_order_id.as_str())
            .cloned())
    }

    async fn reserve_command(&self, command: BrokerCommandRecord) -> Result<(), BrokerError> {
        let mut commands = self.commands.lock().await;
        if commands.contains_key(command.client_order_id.as_str()) {
            return Err(BrokerError::DuplicateIntent {
                client_order_id: command.client_order_id,
            });
        }
        if let Some(parent_client_order_id) = &command.parent_client_order_id
            && !commands.contains_key(parent_client_order_id.as_str())
        {
            return Err(BrokerError::invariant(
                "parent command not found for child broker command",
            ));
        }
        commands.insert(command.client_order_id.to_string(), command);
        Ok(())
    }

    async fn mark_submitting(
        &self,
        client_order_id: &ClientOrderId,
        request_id: &RequestId,
    ) -> Result<(), BrokerError> {
        self.transition(
            client_order_id,
            |status| status == BrokerCommandStatus::CommandRecorded,
            "command_recorded -> submitting",
            |command| {
                command.request_id = Some(request_id.to_string());
                command.transport_send_started_at = Some(time::OffsetDateTime::now_utc());
                command.last_known_state = BrokerCommandStatus::Submitting;
            },
        )
        .await
    }

    async fn mark_ack(
        &self,
        client_order_id: &ClientOrderId,
        ack: BrokerCommandAck,
    ) -> Result<(), BrokerError> {
        self.transition(
            client_order_id,
            |status| status == BrokerCommandStatus::Submitting,
            "submitting -> accepted",
            |command| {
                command.broker_order_id = ack.broker_order_id;
                command.broker_order_orgno = ack.broker_order_orgno;
                command.raw_ack_code = ack.raw_ack_code;
                command.last_known_state = BrokerCommandStatus::Accepted;
                command.reconcile_required = false;
                command.last_reconcile_reason = None;
            },
        )
        .await
    }

    async fn mark_reconcile_needed(
        &self,
        client_order_id: &ClientOrderId,
        reason: &str,
    ) -> Result<(), BrokerError> {
        self.transition(
            client_order_id,
            |status| {
                matches!(
                    status,
                    BrokerCommandStatus::CommandRecorded
                        | BrokerCommandStatus::ReadyToSend
                        | BrokerCommandStatus::Submitting
                )
            },
            "possible submit -> submitted_ack_unknown",
            |command| {
                command.last_known_state = BrokerCommandStatus::SubmittedAckUnknown;
                command.reconcile_required = true;
                command.last_reconcile_reason = Some(reason.to_owned());
            },
        )
        .await
    }

    async fn mark_reconciled(
        &self,
        client_order_id: &ClientOrderId,
        status: BrokerCommandStatus,
    ) -> Result<(), BrokerError> {
        if !is_reconciled_status(status) {
            return Err(BrokerError::validation(
                "reconciled journal status must be accepted, filled, cancelled, or rejected",
            ));
        }
        self.transition(
            client_order_id,
            |current| {
                matches!(
                    current,
                    BrokerCommandStatus::SubmittedAckUnknown | BrokerCommandStatus::ReconcileFailed
                )
            },
            "ack_unknown -> reconciled",
            |command| {
                command.last_known_state = status;
                command.reconcile_required = false;
                command.last_reconciled_at = Some(time::OffsetDateTime::now_utc());
                command.last_reconcile_reason = None;
            },
        )
        .await
    }

    async fn mark_reconcile_failed(
        &self,
        client_order_id: &ClientOrderId,
        reason: &str,
    ) -> Result<(), BrokerError> {
        self.transition(
            client_order_id,
            |current| {
                matches!(
                    current,
                    BrokerCommandStatus::SubmittedAckUnknown | BrokerCommandStatus::ReconcileFailed
                )
            },
            "ack_unknown -> reconcile_failed",
            |command| {
                command.last_known_state = BrokerCommandStatus::ReconcileFailed;
                command.reconcile_required = true;
                command.last_reconciled_at = Some(time::OffsetDateTime::now_utc());
                command.last_reconcile_reason = Some(reason.to_owned());
            },
        )
        .await
    }

    async fn mark_terminal(
        &self,
        client_order_id: &ClientOrderId,
        status: BrokerCommandStatus,
    ) -> Result<(), BrokerError> {
        if !is_terminal_submit_status(status) {
            return Err(BrokerError::validation(
                "terminal submit status must be accepted, cancelled, rejected, filled, or failed",
            ));
        }
        self.transition(
            client_order_id,
            |current| current == BrokerCommandStatus::Submitting,
            "submitting -> terminal",
            |command| {
                command.last_known_state = status;
                command.reconcile_required = false;
                command.last_reconciled_at = Some(time::OffsetDateTime::now_utc());
                command.last_reconcile_reason = None;
            },
        )
        .await
    }

    async fn mark_safety_halted(
        &self,
        client_order_id: &ClientOrderId,
        reason: &str,
    ) -> Result<(), BrokerError> {
        self.transition(
            client_order_id,
            |current| {
                matches!(
                    current,
                    BrokerCommandStatus::CommandRecorded | BrokerCommandStatus::ReadyToSend
                )
            },
            "pre-submit -> safety_halted",
            |command| {
                command.last_known_state = BrokerCommandStatus::SafetyHalted;
                command.reconcile_required = false;
                command.last_reconciled_at = Some(time::OffsetDateTime::now_utc());
                command.last_reconcile_reason = Some(reason.to_owned());
            },
        )
        .await
    }

    async fn list_unreconciled(&self) -> Result<Vec<BrokerCommandRecord>, BrokerError> {
        Ok(self
            .commands
            .lock()
            .await
            .values()
            .filter(|command| {
                command.reconcile_required
                    || matches!(
                        command.last_known_state,
                        BrokerCommandStatus::SubmittedAckUnknown
                            | BrokerCommandStatus::ReconcileFailed
                    )
            })
            .cloned()
            .collect())
    }
}

fn is_reconciled_status(status: BrokerCommandStatus) -> bool {
    matches!(
        status,
        BrokerCommandStatus::Accepted
            | BrokerCommandStatus::Filled
            | BrokerCommandStatus::Cancelled
            | BrokerCommandStatus::Rejected
    )
}

fn is_terminal_submit_status(status: BrokerCommandStatus) -> bool {
    matches!(
        status,
        BrokerCommandStatus::Accepted
            | BrokerCommandStatus::PartiallyFilled
            | BrokerCommandStatus::Filled
            | BrokerCommandStatus::Cancelled
            | BrokerCommandStatus::Rejected
            | BrokerCommandStatus::Failed
            | BrokerCommandStatus::SafetyHalted
    )
}
