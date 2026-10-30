use std::path::Path;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use rusqlite::{Connection, OptionalExtension, Row, params};
use rust_decimal::Decimal;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use trading_agent_kis_client::journal::{
    BrokerCommandAck, BrokerCommandJournal, BrokerCommandRecord, BrokerCommandStatus,
    BrokerCommandType,
};
use trading_agent_kis_client::{
    AccountKey, BrokerEnvironment, BrokerError, ClientOrderId, OrderSide, OrderType, Quantity,
    RequestId, Symbol,
};

use super::{open_migrated_connection, timestamp};

#[derive(Clone)]
pub struct SqliteBrokerCommandJournal {
    connection: Arc<Mutex<Connection>>,
}

impl SqliteBrokerCommandJournal {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, BrokerError> {
        let connection = open_migrated_connection(path).map_err(|error| {
            BrokerError::persistence(format!("failed to open broker command journal: {error}"))
        })?;
        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
        })
    }

    async fn with_connection<T>(
        &self,
        operation: impl FnOnce(&mut Connection) -> Result<T, BrokerError> + Send + 'static,
    ) -> Result<T, BrokerError>
    where
        T: Send + 'static,
    {
        let connection = Arc::clone(&self.connection);
        tokio::task::spawn_blocking(move || {
            let mut guard = connection.lock().map_err(|error| {
                BrokerError::persistence(format!(
                    "broker command journal lock was poisoned: {error}"
                ))
            })?;
            operation(&mut guard)
        })
        .await
        .map_err(|error| {
            BrokerError::persistence(format!("broker command journal task failed: {error}"))
        })?
    }

    async fn transition(
        &self,
        client_order_id: &ClientOrderId,
        expected: impl FnOnce(BrokerCommandStatus) -> bool + Send + 'static,
        transition: &'static str,
        event_type: &'static str,
        update: impl FnOnce(&mut BrokerCommandRecord) -> Result<(), BrokerError> + Send + 'static,
    ) -> Result<(), BrokerError> {
        let client_order_id = client_order_id.clone();
        self.with_connection(move |connection| {
            let transaction = connection.transaction().map_err(sqlite_error)?;
            let mut command = load_command(&transaction, &client_order_id)?
                .ok_or_else(|| BrokerError::invariant("journal command was not found"))?;
            if !expected(command.last_known_state) {
                return Err(BrokerError::invariant(format!(
                    "invalid journal state transition {transition} for command: {client_order_id}"
                )));
            }
            update(&mut command)?;
            update_command(&transaction, &command)?;
            insert_command_event(&transaction, &command, event_type)?;
            transaction.commit().map_err(sqlite_error)
        })
        .await
    }
}

#[async_trait]
impl BrokerCommandJournal for SqliteBrokerCommandJournal {
    async fn get_command(
        &self,
        client_order_id: &ClientOrderId,
    ) -> Result<Option<BrokerCommandRecord>, BrokerError> {
        let client_order_id = client_order_id.clone();
        self.with_connection(move |connection| load_command(connection, &client_order_id))
            .await
    }

    async fn reserve_command(&self, command: BrokerCommandRecord) -> Result<(), BrokerError> {
        self.with_connection(move |connection| {
            let transaction = connection.transaction().map_err(sqlite_error)?;
            if command_exists(&transaction, &command.client_order_id)? {
                return Err(BrokerError::DuplicateIntent {
                    client_order_id: command.client_order_id,
                });
            }
            if let Some(parent_client_order_id) = &command.parent_client_order_id
                && !command_exists(&transaction, parent_client_order_id)?
            {
                return Err(BrokerError::invariant(
                    "parent command not found for child broker command",
                ));
            }
            insert_command(&transaction, &command)?;
            transaction.commit().map_err(sqlite_error)
        })
        .await
    }

    async fn mark_submitting(
        &self,
        client_order_id: &ClientOrderId,
        request_id: &RequestId,
    ) -> Result<(), BrokerError> {
        let request_id = request_id.to_string();
        self.transition(
            client_order_id,
            |status| status == BrokerCommandStatus::CommandRecorded,
            "command_recorded -> submitting",
            "submitting",
            move |command| {
                command.request_id = Some(request_id);
                command.transport_send_started_at = Some(OffsetDateTime::now_utc());
                command.last_known_state = BrokerCommandStatus::Submitting;
                Ok(())
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
            "accepted",
            |command| {
                command.broker_order_id = ack.broker_order_id;
                command.broker_order_orgno = ack.broker_order_orgno;
                command.raw_ack_code = ack.raw_ack_code;
                command.last_known_state = BrokerCommandStatus::Accepted;
                command.reconcile_required = false;
                command.last_reconcile_reason = None;
                Ok(())
            },
        )
        .await
    }

    async fn mark_reconcile_needed(
        &self,
        client_order_id: &ClientOrderId,
        reason: &str,
    ) -> Result<(), BrokerError> {
        let reason = reason.to_owned();
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
            "submitted_ack_unknown",
            move |command| {
                command.last_known_state = BrokerCommandStatus::SubmittedAckUnknown;
                command.reconcile_required = true;
                command.last_reconcile_reason = Some(reason);
                Ok(())
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
                    BrokerCommandStatus::Submitting
                        | BrokerCommandStatus::SubmittedAckUnknown
                        | BrokerCommandStatus::ReconcileFailed
                )
            },
            "unresolved -> reconciled",
            "reconciled",
            move |command| {
                command.last_known_state = status;
                command.reconcile_required = false;
                command.last_reconciled_at = Some(OffsetDateTime::now_utc());
                command.last_reconcile_reason = None;
                Ok(())
            },
        )
        .await
    }

    async fn mark_reconcile_failed(
        &self,
        client_order_id: &ClientOrderId,
        reason: &str,
    ) -> Result<(), BrokerError> {
        let reason = reason.to_owned();
        self.transition(
            client_order_id,
            |current| {
                matches!(
                    current,
                    BrokerCommandStatus::Submitting
                        | BrokerCommandStatus::SubmittedAckUnknown
                        | BrokerCommandStatus::ReconcileFailed
                )
            },
            "unresolved -> reconcile_failed",
            "reconcile_failed",
            move |command| {
                command.last_known_state = BrokerCommandStatus::ReconcileFailed;
                command.reconcile_required = true;
                command.last_reconciled_at = Some(OffsetDateTime::now_utc());
                command.last_reconcile_reason = Some(reason);
                Ok(())
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
            "terminal",
            move |command| {
                command.last_known_state = status;
                command.reconcile_required = false;
                command.last_reconciled_at = Some(OffsetDateTime::now_utc());
                command.last_reconcile_reason = None;
                Ok(())
            },
        )
        .await
    }

    async fn mark_safety_halted(
        &self,
        client_order_id: &ClientOrderId,
        reason: &str,
    ) -> Result<(), BrokerError> {
        let reason = reason.to_owned();
        self.transition(
            client_order_id,
            |current| {
                matches!(
                    current,
                    BrokerCommandStatus::CommandRecorded | BrokerCommandStatus::ReadyToSend
                )
            },
            "pre-submit -> safety_halted",
            "safety_halted",
            move |command| {
                command.last_known_state = BrokerCommandStatus::SafetyHalted;
                command.reconcile_required = false;
                command.last_reconciled_at = Some(OffsetDateTime::now_utc());
                command.last_reconcile_reason = Some(reason);
                Ok(())
            },
        )
        .await
    }

    async fn list_unreconciled(&self) -> Result<Vec<BrokerCommandRecord>, BrokerError> {
        self.with_connection(move |connection| {
            let mut statement = connection
                .prepare(
                    "SELECT *
                     FROM broker_command_journal
                     WHERE reconcile_required = 1
                        OR last_known_state IN ('submitting', 'submitted_ack_unknown', 'reconcile_failed')
                     ORDER BY requested_at, client_order_id",
                )
                .map_err(sqlite_error)?;
            statement
                .query_map([], StoredCommand::from_row)
                .map_err(sqlite_error)?
                .map(|row| {
                    row.map_err(sqlite_error)
                        .and_then(StoredCommand::into_record)
                })
                .collect()
        })
        .await
    }
}

struct StoredCommand {
    client_order_id: String,
    parent_client_order_id: Option<String>,
    decision_id: Option<String>,
    command_type: String,
    environment: String,
    account_key: String,
    symbol: String,
    side: String,
    quantity: i64,
    limit_price: Option<String>,
    order_type: String,
    requested_at: String,
    request_id: Option<String>,
    transport_send_started_at: Option<String>,
    broker_order_id: Option<String>,
    broker_order_orgno: Option<String>,
    target_broker_order_id: Option<String>,
    target_broker_order_orgno: Option<String>,
    cancel_or_amend_quantity: Option<i64>,
    amend_price: Option<String>,
    exchange_code: Option<String>,
    last_known_state: String,
    raw_ack_code: Option<String>,
    reconcile_required: i64,
    last_reconciled_at: Option<String>,
    last_reconcile_reason: Option<String>,
    trading_day: Option<String>,
    broker_observation_window_start: Option<String>,
    broker_observation_window_end: Option<String>,
    sanitized_last_request_metadata_id: Option<String>,
    sanitized_last_response_metadata_id: Option<String>,
}

impl StoredCommand {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            client_order_id: row.get("client_order_id")?,
            parent_client_order_id: row.get("parent_client_order_id")?,
            decision_id: row.get("decision_id")?,
            command_type: row.get("command_type")?,
            environment: row.get("environment")?,
            account_key: row.get("account_key")?,
            symbol: row.get("symbol")?,
            side: row.get("side")?,
            quantity: row.get("quantity")?,
            limit_price: row.get("limit_price")?,
            order_type: row.get("order_type")?,
            requested_at: row.get("requested_at")?,
            request_id: row.get("request_id")?,
            transport_send_started_at: row.get("transport_send_started_at")?,
            broker_order_id: row.get("broker_order_id")?,
            broker_order_orgno: row.get("broker_order_orgno")?,
            target_broker_order_id: row.get("target_broker_order_id")?,
            target_broker_order_orgno: row.get("target_broker_order_orgno")?,
            cancel_or_amend_quantity: row.get("cancel_or_amend_quantity")?,
            amend_price: row.get("amend_price")?,
            exchange_code: row.get("exchange_code")?,
            last_known_state: row.get("last_known_state")?,
            raw_ack_code: row.get("raw_ack_code")?,
            reconcile_required: row.get("reconcile_required")?,
            last_reconciled_at: row.get("last_reconciled_at")?,
            last_reconcile_reason: row.get("last_reconcile_reason")?,
            trading_day: row.get("trading_day")?,
            broker_observation_window_start: row.get("broker_observation_window_start")?,
            broker_observation_window_end: row.get("broker_observation_window_end")?,
            sanitized_last_request_metadata_id: row.get("sanitized_last_request_metadata_id")?,
            sanitized_last_response_metadata_id: row.get("sanitized_last_response_metadata_id")?,
        })
    }

    fn into_record(self) -> Result<BrokerCommandRecord, BrokerError> {
        Ok(BrokerCommandRecord {
            client_order_id: ClientOrderId::new(self.client_order_id)?,
            parent_client_order_id: self
                .parent_client_order_id
                .map(ClientOrderId::new)
                .transpose()?,
            decision_id: self.decision_id,
            command_type: BrokerCommandType::parse(&self.command_type)?,
            environment: BrokerEnvironment::parse(&self.environment)?,
            account_key: AccountKey::new(self.account_key)?,
            symbol: Symbol::new(self.symbol)?,
            side: OrderSide::parse(&self.side)?,
            quantity: parse_quantity("quantity", self.quantity)?,
            limit_price: parse_optional_decimal("limit_price", self.limit_price)?,
            order_type: OrderType::parse(&self.order_type)?,
            requested_at: parse_timestamp("requested_at", &self.requested_at)?,
            request_id: self.request_id,
            transport_send_started_at: parse_optional_timestamp(
                "transport_send_started_at",
                self.transport_send_started_at,
            )?,
            broker_order_id: self.broker_order_id,
            broker_order_orgno: self.broker_order_orgno,
            target_broker_order_id: self.target_broker_order_id,
            target_broker_order_orgno: self.target_broker_order_orgno,
            cancel_or_amend_quantity: self
                .cancel_or_amend_quantity
                .map(|quantity| parse_quantity("cancel_or_amend_quantity", quantity))
                .transpose()?,
            amend_price: parse_optional_decimal("amend_price", self.amend_price)?,
            exchange_code: self.exchange_code,
            last_known_state: BrokerCommandStatus::parse(&self.last_known_state)?,
            raw_ack_code: self.raw_ack_code,
            reconcile_required: parse_bool("reconcile_required", self.reconcile_required)?,
            last_reconciled_at: parse_optional_timestamp(
                "last_reconciled_at",
                self.last_reconciled_at,
            )?,
            last_reconcile_reason: self.last_reconcile_reason,
            trading_day: self.trading_day,
            broker_observation_window_start: parse_optional_timestamp(
                "broker_observation_window_start",
                self.broker_observation_window_start,
            )?,
            broker_observation_window_end: parse_optional_timestamp(
                "broker_observation_window_end",
                self.broker_observation_window_end,
            )?,
            sanitized_last_request_metadata_id: self.sanitized_last_request_metadata_id,
            sanitized_last_response_metadata_id: self.sanitized_last_response_metadata_id,
        })
    }
}

fn command_exists(
    connection: &Connection,
    client_order_id: &ClientOrderId,
) -> Result<bool, BrokerError> {
    connection
        .query_row(
            "SELECT 1 FROM broker_command_journal WHERE client_order_id = ?1",
            params![client_order_id.as_str()],
            |_| Ok(()),
        )
        .optional()
        .map(|value| value.is_some())
        .map_err(sqlite_error)
}

fn load_command(
    connection: &Connection,
    client_order_id: &ClientOrderId,
) -> Result<Option<BrokerCommandRecord>, BrokerError> {
    connection
        .query_row(
            "SELECT * FROM broker_command_journal WHERE client_order_id = ?1",
            params![client_order_id.as_str()],
            StoredCommand::from_row,
        )
        .optional()
        .map_err(sqlite_error)?
        .map(StoredCommand::into_record)
        .transpose()
}

fn insert_command(
    connection: &Connection,
    command: &BrokerCommandRecord,
) -> Result<(), BrokerError> {
    let now = timestamp().map_err(|error| BrokerError::persistence(error.to_string()))?;
    connection
        .execute(
            "INSERT INTO broker_command_journal (
                client_order_id,
                parent_client_order_id,
                decision_id,
                command_type,
                environment,
                account_key,
                symbol,
                side,
                quantity,
                limit_price,
                order_type,
                requested_at,
                request_id,
                transport_send_started_at,
                broker_order_id,
                broker_order_orgno,
                target_broker_order_id,
                target_broker_order_orgno,
                cancel_or_amend_quantity,
                amend_price,
                exchange_code,
                last_known_state,
                raw_ack_code,
                reconcile_required,
                last_reconciled_at,
                last_reconcile_reason,
                trading_day,
                broker_observation_window_start,
                broker_observation_window_end,
                sanitized_last_request_metadata_id,
                sanitized_last_response_metadata_id,
                created_at,
                updated_at
            )
            VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17,
                ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32, ?33
            )",
            params![
                command.client_order_id.as_str(),
                command
                    .parent_client_order_id
                    .as_ref()
                    .map(ClientOrderId::as_str),
                command.decision_id.as_deref(),
                command.command_type.as_str(),
                command.environment.as_str(),
                command.account_key.as_str(),
                command.symbol.as_str(),
                command.side.as_str(),
                quantity_to_i64(command.quantity)?,
                command.limit_price.map(|price| price.to_string()),
                command.order_type.as_str(),
                format_timestamp(command.requested_at)?,
                command.request_id.as_deref(),
                format_optional_timestamp(command.transport_send_started_at)?,
                command.broker_order_id.as_deref(),
                command.broker_order_orgno.as_deref(),
                command.target_broker_order_id.as_deref(),
                command.target_broker_order_orgno.as_deref(),
                command
                    .cancel_or_amend_quantity
                    .map(quantity_to_i64)
                    .transpose()?,
                command.amend_price.map(|price| price.to_string()),
                command.exchange_code.as_deref(),
                command.last_known_state.as_str(),
                command.raw_ack_code.as_deref(),
                bool_to_i64(command.reconcile_required),
                format_optional_timestamp(command.last_reconciled_at)?,
                command.last_reconcile_reason.as_deref(),
                command.trading_day.as_deref(),
                format_optional_timestamp(command.broker_observation_window_start)?,
                format_optional_timestamp(command.broker_observation_window_end)?,
                command.sanitized_last_request_metadata_id.as_deref(),
                command.sanitized_last_response_metadata_id.as_deref(),
                now,
                now,
            ],
        )
        .map_err(sqlite_error)?;
    insert_command_event(connection, command, "command_recorded")
}

fn update_command(
    connection: &Connection,
    command: &BrokerCommandRecord,
) -> Result<(), BrokerError> {
    let updated_at = timestamp().map_err(|error| BrokerError::persistence(error.to_string()))?;
    let affected = connection
        .execute(
            "UPDATE broker_command_journal
             SET request_id = ?2,
                 transport_send_started_at = ?3,
                 broker_order_id = ?4,
                 broker_order_orgno = ?5,
                 last_known_state = ?6,
                 raw_ack_code = ?7,
                 reconcile_required = ?8,
                 last_reconciled_at = ?9,
                 last_reconcile_reason = ?10,
                 updated_at = ?11
             WHERE client_order_id = ?1",
            params![
                command.client_order_id.as_str(),
                command.request_id.as_deref(),
                format_optional_timestamp(command.transport_send_started_at)?,
                command.broker_order_id.as_deref(),
                command.broker_order_orgno.as_deref(),
                command.last_known_state.as_str(),
                command.raw_ack_code.as_deref(),
                bool_to_i64(command.reconcile_required),
                format_optional_timestamp(command.last_reconciled_at)?,
                command.last_reconcile_reason.as_deref(),
                updated_at,
            ],
        )
        .map_err(sqlite_error)?;
    if affected == 0 {
        return Err(BrokerError::invariant("journal command was not found"));
    }
    Ok(())
}

fn insert_command_event(
    connection: &Connection,
    command: &BrokerCommandRecord,
    event_type: &str,
) -> Result<(), BrokerError> {
    let observed_at = timestamp().map_err(|error| BrokerError::persistence(error.to_string()))?;
    let metadata_json = serde_json::json!({
        "command_type": command.command_type.as_str(),
        "last_known_state": command.last_known_state.as_str(),
        "reconcile_required": command.reconcile_required,
        "broker_order_id": command.broker_order_id,
        "broker_order_orgno": command.broker_order_orgno,
        "raw_ack_code": command.raw_ack_code,
        "last_reconcile_reason": command.last_reconcile_reason
    })
    .to_string();
    connection
        .execute(
            "INSERT INTO broker_command_events (
                client_order_id,
                event_type,
                observed_at,
                request_id,
                metadata_json
            )
            VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                command.client_order_id.as_str(),
                event_type,
                observed_at,
                command.request_id.as_deref(),
                metadata_json,
            ],
        )
        .map(|_| ())
        .map_err(sqlite_error)
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

fn quantity_to_i64(quantity: Quantity) -> Result<i64, BrokerError> {
    i64::try_from(quantity.get())
        .map_err(|error| BrokerError::persistence(format!("quantity overflow: {error}")))
}

fn parse_quantity(field: &str, quantity: i64) -> Result<Quantity, BrokerError> {
    let quantity = u64::try_from(quantity)
        .map_err(|error| BrokerError::persistence(format!("invalid {field}: {error}")))?;
    if quantity == 0 {
        return Err(BrokerError::persistence(format!(
            "{field} must be positive"
        )));
    }
    Ok(Quantity::new(quantity))
}

fn parse_bool(field: &str, value: i64) -> Result<bool, BrokerError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        other => Err(BrokerError::persistence(format!(
            "invalid {field} boolean value: {other}"
        ))),
    }
}

fn bool_to_i64(value: bool) -> i64 {
    if value { 1 } else { 0 }
}

fn parse_optional_decimal(
    field: &str,
    value: Option<String>,
) -> Result<Option<Decimal>, BrokerError> {
    value
        .map(|value| {
            Decimal::from_str(&value)
                .map_err(|error| BrokerError::persistence(format!("invalid {field}: {error}")))
        })
        .transpose()
}

fn parse_timestamp(field: &str, value: &str) -> Result<OffsetDateTime, BrokerError> {
    OffsetDateTime::parse(value, &Rfc3339)
        .map_err(|error| BrokerError::persistence(format!("invalid {field}: {error}")))
}

fn parse_optional_timestamp(
    field: &str,
    value: Option<String>,
) -> Result<Option<OffsetDateTime>, BrokerError> {
    value
        .as_deref()
        .map(|value| parse_timestamp(field, value))
        .transpose()
}

fn format_timestamp(value: OffsetDateTime) -> Result<String, BrokerError> {
    value
        .format(&Rfc3339)
        .map_err(|error| BrokerError::persistence(format!("failed to format timestamp: {error}")))
}

fn format_optional_timestamp(value: Option<OffsetDateTime>) -> Result<Option<String>, BrokerError> {
    value.map(format_timestamp).transpose()
}

fn sqlite_error(error: rusqlite::Error) -> BrokerError {
    BrokerError::persistence(error.to_string())
}
