use std::fs;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use rusqlite::Connection;
use rust_decimal::Decimal;
use tempfile::tempdir;
use trading_agent_kis_client::journal::{
    BrokerCommandAck, BrokerCommandJournal, BrokerCommandRecord, BrokerCommandStatus,
};
use trading_agent_kis_client::{
    AccountKey, BrokerEnvironment, BrokerError, BrokerHealth, BrokerOrderId, BrokerOrderOrgNo,
    ClientOrderId, OrderReference, OrderSide, OrderSubmission, ReconciliationResult, RequestId,
};

use trade_engine::config::{
    AdvisorConfig, AdvisorProviderKind, RiskConfig, RuntimeConfig, StrategyConfig,
};
use trade_engine::gateway::testing::MockTradingGateway;
use trade_engine::gateway::traits::TradingGateway;
use trade_engine::gateway::types::{
    AccountSnapshot, AmendOrderRequest, AmendResult, BuyingPower, CancelOrderRequest, CancelResult,
    CashBalance, CreateOrderRequest, Fill, MarketSession, OrderStatus, Position, Quantity, Quote,
    Symbol, TradingDay,
};
use trade_engine::reconciliation::ReconciliationGateway;
use trade_engine::runner::DbRuntimeExecutionDriver;
use trade_engine::runtime::{
    DryRunRunContext, DryRunRuntimeSettings, RuntimeLoopControl, RuntimeMarketHours, RuntimeMode,
    run_runtime_loop_with_execution,
};
use trade_engine::storage::{BlockingMainDb, MainDb, SqliteBrokerCommandJournal};

#[tokio::test]
async fn submit_orders_loop_submits_current_run_execution_request_after_tick() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let request_path = tempdir.path().join("advisor-request.json");
    let script_path = tempdir.path().join("advisor.sh");
    fs::write(
        &script_path,
        format!(
            r#"cat > "{}"
printf '%s\n' '{{"summary":"buy proposal","observations":["quote present"],"risks":[],"next_actions":["submit through policy"],"rationale":"test buy","evidence":[{{"source":"test","detail":"buy"}}],"proposals":[{{"decision_type":"buy","symbol":"005930","quantity":1,"order_type":"limit","limit_price":"73000","confidence":"0.80","time_horizon":"intraday","reason":"test buy","risk_notes":[]}}]}}'
"#,
            request_path.display()
        ),
    )
    .expect("write advisor script");

    let store = BlockingMainDb::open(&db_path).expect("open store");
    let journal = SqliteBrokerCommandJournal::open(&db_path).expect("open journal");
    let base_gateway = MockTradingGateway::new()
        .with_cash(Decimal::new(1_000_000, 0))
        .with_buying_power(Decimal::new(1_000_000, 0))
        .with_quote(Quote {
            symbol: Symbol::new("005930").expect("symbol"),
            full_name: None,
            last_price: Decimal::new(73_000, 0),
        })
        .with_order_submission(OrderSubmission {
            reference: accepted_reference("placeholder-client-order-id"),
            accepted: true,
        });
    let gateway = JournalAwareGateway::new(base_gateway.clone(), &journal);
    let account_key = AccountKey::new("12345678-01").expect("account key");
    let run_context = DryRunRunContext::new("5678", BrokerEnvironment::Paper)
        .with_account_key(account_key.clone())
        .with_mode(RuntimeMode::SubmitOrders);
    let settings = DryRunRuntimeSettings::with_risk(
        RuntimeConfig {
            market_tick_seconds: 1,
            ..RuntimeConfig::default()
        },
        StrategyConfig::default(),
        Default::default(),
        RiskConfig {
            enable_live_trading: true,
            max_order_notional: Decimal::new(100_000, 0),
            max_position_notional: Decimal::new(1_000_000, 0),
            max_daily_loss: Decimal::new(100_000, 0),
            max_daily_order_count: 3,
        },
        false,
    )
    .with_advisor(AdvisorConfig {
        provider: AdvisorProviderKind::Command,
        command: Some(PathBuf::from("sh")),
        args: vec![script_path.to_string_lossy().into_owned()],
        timeout_seconds: 5,
        max_stderr_bytes: 4096,
        max_stdout_bytes: 262_144,
        ..AdvisorConfig::default()
    });
    let execution_driver = DbRuntimeExecutionDriver::new(
        db_path.clone(),
        account_key,
        BrokerEnvironment::Paper,
        false,
        settings.risk.clone(),
    );

    let output = run_runtime_loop_with_execution(
        &gateway,
        &store,
        &run_context,
        &[Symbol::new("005930").expect("symbol")],
        settings,
        RuntimeLoopControl {
            market_hours: RuntimeMarketHours::Regular,
            max_market_ticks: Some(1),
        },
        &execution_driver,
    )
    .await
    .expect("submit orders loop");

    assert!(output.contains("mode: submit_orders"));
    assert!(output.contains("Execution submissions #1\n  status: submitted\n  count: 1"));
    assert_eq!(base_gateway.call_counts().place_order, 1);
    assert_eq!(runtime_run_modes(&db_path), vec!["submit_orders"]);
    assert_eq!(execution_statuses(&db_path), vec!["accepted"]);
}

#[tokio::test]
async fn submit_orders_loop_reconciles_unresolved_journal_before_next_tick() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let request_path = tempdir.path().join("advisor-request.json");
    let script_path = tempdir.path().join("advisor.sh");
    fs::write(
        &script_path,
        format!(
            r#"cat > "{}"
printf '%s\n' '{{"summary":"buy proposal","observations":["quote present"],"risks":[],"next_actions":["submit through policy"],"rationale":"test buy","evidence":[{{"source":"test","detail":"buy"}}],"proposals":[{{"decision_type":"buy","symbol":"005930","quantity":1,"order_type":"limit","limit_price":"73000","confidence":"0.80","time_horizon":"intraday","reason":"test buy","risk_notes":[]}}]}}'
"#,
            request_path.display()
        ),
    )
    .expect("write advisor script");

    let store = BlockingMainDb::open(&db_path).expect("open store");
    let journal = SqliteBrokerCommandJournal::open(&db_path).expect("open journal");
    seed_unresolved_journal_command(&journal).await;
    assert_eq!(
        journal.list_unreconciled().await.expect("unresolved").len(),
        1
    );
    let base_gateway = MockTradingGateway::new()
        .with_cash(Decimal::new(1_000_000, 0))
        .with_buying_power(Decimal::new(1_000_000, 0))
        .with_quote(Quote {
            symbol: Symbol::new("005930").expect("symbol"),
            full_name: None,
            last_price: Decimal::new(73_000, 0),
        })
        .with_order_submission(OrderSubmission {
            reference: accepted_reference("placeholder-client-order-id"),
            accepted: true,
        });
    let gateway = JournalAwareGateway::new(base_gateway.clone(), &journal);
    let account_key = AccountKey::new("12345678-01").expect("account key");
    let run_context = DryRunRunContext::new("5678", BrokerEnvironment::Paper)
        .with_account_key(account_key.clone())
        .with_mode(RuntimeMode::SubmitOrders);
    let settings = DryRunRuntimeSettings::with_risk(
        RuntimeConfig {
            market_tick_seconds: 1,
            ..RuntimeConfig::default()
        },
        StrategyConfig::default(),
        Default::default(),
        RiskConfig {
            enable_live_trading: true,
            max_order_notional: Decimal::new(100_000, 0),
            max_position_notional: Decimal::new(1_000_000, 0),
            max_daily_loss: Decimal::new(100_000, 0),
            max_daily_order_count: 3,
        },
        false,
    )
    .with_advisor(AdvisorConfig {
        provider: AdvisorProviderKind::Command,
        command: Some(PathBuf::from("sh")),
        args: vec![script_path.to_string_lossy().into_owned()],
        timeout_seconds: 5,
        max_stderr_bytes: 4096,
        max_stdout_bytes: 262_144,
        ..AdvisorConfig::default()
    });
    let execution_driver = DbRuntimeExecutionDriver::new(
        db_path.clone(),
        account_key,
        BrokerEnvironment::Paper,
        false,
        settings.risk.clone(),
    );

    let output = run_runtime_loop_with_execution(
        &gateway,
        &store,
        &run_context,
        &[Symbol::new("005930").expect("symbol")],
        settings,
        RuntimeLoopControl {
            market_hours: RuntimeMarketHours::Regular,
            max_market_ticks: Some(1),
        },
        &execution_driver,
    )
    .await
    .expect("submit orders loop");

    let reconciliation_index = output
        .find("Reconciliation #1\n  status: reconciled\n  count: 1")
        .expect("reconciliation output");
    let tick_index = output
        .find("Market tick #1\n  status: started")
        .expect("tick output");
    assert!(reconciliation_index < tick_index);
    assert!(output.contains("Execution submissions #1\n  status: submitted\n  count: 1"));
    assert_eq!(base_gateway.call_counts().place_order, 1);
    assert!(
        journal
            .list_unreconciled()
            .await
            .expect("unresolved")
            .is_empty()
    );
    assert_eq!(execution_statuses(&db_path), vec!["accepted"]);
}

#[tokio::test]
async fn submit_orders_driver_ignores_stale_requested_execution_from_previous_run() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let account_key = AccountKey::new("12345678-01").expect("account key");
    seed_stale_requested_execution(&db_path);
    let store = BlockingMainDb::open(&db_path).expect("open store");
    let journal = SqliteBrokerCommandJournal::open(&db_path).expect("open journal");
    let base_gateway = MockTradingGateway::new()
        .with_cash(Decimal::new(1_000_000, 0))
        .with_quote(Quote {
            symbol: Symbol::new("005930").expect("symbol"),
            full_name: None,
            last_price: Decimal::new(73_000, 0),
        });
    let gateway = JournalAwareGateway::new(base_gateway.clone(), &journal);
    let settings = DryRunRuntimeSettings::with_risk(
        RuntimeConfig::default(),
        StrategyConfig {
            id: "hold-only".to_owned(),
            short_window: None,
            long_window: None,
        },
        Default::default(),
        RiskConfig {
            enable_live_trading: true,
            ..RiskConfig::default()
        },
        false,
    );
    let execution_driver = DbRuntimeExecutionDriver::new(
        db_path.clone(),
        account_key.clone(),
        BrokerEnvironment::Paper,
        false,
        settings.risk.clone(),
    );

    let output = run_runtime_loop_with_execution(
        &gateway,
        &store,
        &DryRunRunContext::new("5678", BrokerEnvironment::Paper)
            .with_account_key(account_key)
            .with_mode(RuntimeMode::SubmitOrders),
        &[Symbol::new("005930").expect("symbol")],
        settings,
        RuntimeLoopControl {
            market_hours: RuntimeMarketHours::Regular,
            max_market_ticks: Some(1),
        },
        &execution_driver,
    )
    .await
    .expect("submit orders loop");

    assert!(output.contains("Execution submissions #1\n  status: none\n  count: 0"));
    assert_eq!(base_gateway.call_counts().place_order, 0);
    assert_eq!(execution_statuses(&db_path), vec!["requested"]);
}

fn runtime_run_modes(path: &Path) -> Vec<String> {
    let connection = Connection::open(path).expect("open db");
    let mut statement = connection
        .prepare("SELECT mode FROM runtime_runs ORDER BY id")
        .expect("prepare runtime run modes");
    statement
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query runtime run modes")
        .collect::<Result<Vec<_>, _>>()
        .expect("runtime run modes")
}

fn execution_statuses(path: &Path) -> Vec<String> {
    let connection = Connection::open(path).expect("open db");
    let mut statement = connection
        .prepare("SELECT status FROM execution_requests ORDER BY id")
        .expect("prepare execution statuses");
    statement
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query execution statuses")
        .collect::<Result<Vec<_>, _>>()
        .expect("execution statuses")
}

fn seed_stale_requested_execution(path: &Path) {
    let mut db = MainDb::open(path).expect("open db");
    let context = DryRunRunContext::new("5678", BrokerEnvironment::Paper);
    let run_id = db.start_dry_run_run(&context).expect("start run");
    let tick_id = db
        .start_dry_run_tick(run_id, 0, std::time::Duration::from_secs(60))
        .expect("start tick");
    let connection = Connection::open(path).expect("open raw db");
    connection
        .execute(
            "INSERT INTO decision_outputs (
                run_id,
                tick_id,
                observed_at,
                provider,
                raw_output_json,
                status
            )
            VALUES (?1, ?2, ?3, 'stale-test', '{}', 'parsed')",
            rusqlite::params![run_id.as_i64(), tick_id.as_i64(), "2026-04-26T00:00:00Z"],
        )
        .expect("insert stale decision output");
    let decision_output_id = connection.last_insert_rowid();
    connection
        .execute(
            "INSERT INTO decision_proposals (
                run_id,
                tick_id,
                decision_output_id,
                observed_at,
                decision_type,
                symbol,
                quantity,
                order_type,
                limit_price,
                confidence,
                time_horizon,
                reason,
                risk_notes_json,
                status
            )
            VALUES (?1, ?2, ?3, ?4, 'buy', '005930', 1, 'limit', '73000', '0.80', 'intraday', 'stale', '[]', 'valid')",
            rusqlite::params![
                run_id.as_i64(),
                tick_id.as_i64(),
                decision_output_id,
                "2026-04-26T00:00:00Z"
            ],
        )
        .expect("insert stale decision proposal");
    let decision_proposal_id = connection.last_insert_rowid();
    connection
        .execute(
            "INSERT INTO execution_requests (
                run_id,
                tick_id,
                decision_proposal_id,
                created_at,
                status,
                symbol,
                side,
                quantity,
                order_type,
                limit_price,
                execution_request_id
            )
            VALUES (?1, ?2, ?3, ?4, 'requested', '005930', 'buy', 1, 'limit', '73000', 'stale-exec')",
            rusqlite::params![
                run_id.as_i64(),
                tick_id.as_i64(),
                decision_proposal_id,
                "2026-04-26T00:00:00Z"
            ],
        )
        .expect("insert stale execution");
    connection
        .execute(
            "UPDATE runtime_ticks
             SET status = 'completed',
                 completed_at = '2026-04-26T00:00:01Z',
                 lease_expires_at = '2026-04-26T00:00:01Z'
             WHERE id = ?1",
            [tick_id.as_i64()],
        )
        .expect("complete stale tick");
    connection
        .execute(
            "UPDATE runtime_runs
             SET status = 'completed',
                 completed_at = '2026-04-26T00:00:01Z'
             WHERE id = ?1",
            [run_id.as_i64()],
        )
        .expect("complete stale run");
}

async fn seed_unresolved_journal_command(journal: &SqliteBrokerCommandJournal) {
    let client_order_id = ClientOrderId::new("exec-stale-reconcile").expect("client order id");
    let command = BrokerCommandRecord::new_place_order(
        BrokerEnvironment::Paper,
        CreateOrderRequest::cash_limit(
            client_order_id.clone(),
            AccountKey::new("12345678-01").expect("account key"),
            Symbol::new("000660").expect("symbol"),
            OrderSide::Buy,
            Quantity::new(1),
            Decimal::new(100_000, 0),
        )
        .expect("order request"),
    )
    .expect("place command");
    journal
        .reserve_command(command)
        .await
        .expect("reserve stale command");
    let request_id = RequestId::new("stale-reconcile-request").expect("request id");
    journal
        .mark_submitting(&client_order_id, &request_id)
        .await
        .expect("mark submitting");
    journal
        .mark_reconcile_needed(&client_order_id, "stale ambiguous command")
        .await
        .expect("mark reconcile needed");
}

fn accepted_reference(client_order_id: &str) -> OrderReference {
    OrderReference {
        client_order_id: ClientOrderId::new(client_order_id).expect("client order id"),
        broker_order_id: Some(BrokerOrderId::new("broker-order-1").expect("broker order id")),
        broker_order_orgno: Some(BrokerOrderOrgNo::new("001").expect("broker order orgno")),
        exchange_code: None,
        account_key: AccountKey::new("12345678-01").expect("account key"),
        environment: BrokerEnvironment::Paper,
    }
}

struct JournalAwareGateway<'a> {
    inner: MockTradingGateway,
    journal: &'a SqliteBrokerCommandJournal,
    reconciliation_result: ReconciliationResult,
}

impl<'a> JournalAwareGateway<'a> {
    fn new(inner: MockTradingGateway, journal: &'a SqliteBrokerCommandJournal) -> Self {
        Self {
            inner,
            journal,
            reconciliation_result: ReconciliationResult::ConfirmedRejected,
        }
    }
}

#[async_trait]
impl TradingGateway for JournalAwareGateway<'_> {
    async fn ping(&self) -> Result<BrokerHealth, BrokerError> {
        self.inner.ping().await
    }

    async fn get_account_snapshot(&self) -> Result<AccountSnapshot, BrokerError> {
        self.inner.get_account_snapshot().await
    }

    async fn get_cash_balance(&self) -> Result<CashBalance, BrokerError> {
        self.inner.get_cash_balance().await
    }

    async fn get_positions(&self) -> Result<Vec<Position>, BrokerError> {
        self.inner.get_positions().await
    }

    async fn get_quote(&self, symbol: &Symbol) -> Result<Quote, BrokerError> {
        self.inner.get_quote(symbol).await
    }

    async fn get_market_session(&self, trading_day: &str) -> Result<MarketSession, BrokerError> {
        self.inner.get_market_session(trading_day).await
    }

    async fn get_buying_power(&self, symbol: Option<&Symbol>) -> Result<BuyingPower, BrokerError> {
        self.inner.get_buying_power(symbol).await
    }

    async fn list_open_orders(&self) -> Result<Vec<OrderStatus>, BrokerError> {
        self.inner.list_open_orders().await
    }

    async fn list_fills(&self, day: TradingDay) -> Result<Vec<Fill>, BrokerError> {
        self.inner.list_fills(day).await
    }

    async fn place_order(&self, order: CreateOrderRequest) -> Result<OrderSubmission, BrokerError> {
        let client_order_id = order.client_order_id.clone();
        let request_id = RequestId::new(format!("runtime-write-{client_order_id}"))?;
        self.journal
            .mark_submitting(&client_order_id, &request_id)
            .await?;
        let mut submission = self.inner.place_order(order).await?;
        submission.reference.client_order_id = client_order_id.clone();
        self.journal
            .mark_ack(
                &client_order_id,
                BrokerCommandAck {
                    broker_order_id: submission
                        .reference
                        .broker_order_id
                        .as_ref()
                        .map(ToString::to_string),
                    broker_order_orgno: submission
                        .reference
                        .broker_order_orgno
                        .as_ref()
                        .map(ToString::to_string),
                    raw_ack_code: Some("runtime-write-test".to_owned()),
                },
            )
            .await?;
        Ok(submission)
    }

    async fn cancel_order(&self, request: CancelOrderRequest) -> Result<CancelResult, BrokerError> {
        self.inner.cancel_order(request).await
    }

    async fn amend_order(&self, request: AmendOrderRequest) -> Result<AmendResult, BrokerError> {
        self.inner.amend_order(request).await
    }
}

#[async_trait]
impl ReconciliationGateway for JournalAwareGateway<'_> {
    async fn reconcile_order(
        &self,
        client_order_id: &ClientOrderId,
        _lookback_days: u16,
    ) -> Result<ReconciliationResult, BrokerError> {
        match self.reconciliation_result {
            ReconciliationResult::ConfirmedSubmitted | ReconciliationResult::ConfirmedAmended => {
                self.journal
                    .mark_reconciled(client_order_id, BrokerCommandStatus::Accepted)
                    .await?;
            }
            ReconciliationResult::ConfirmedRejected => {
                self.journal
                    .mark_reconciled(client_order_id, BrokerCommandStatus::Rejected)
                    .await?;
            }
            ReconciliationResult::ConfirmedCancelled => {
                self.journal
                    .mark_reconciled(client_order_id, BrokerCommandStatus::Cancelled)
                    .await?;
            }
            ReconciliationResult::ConfirmedFilled => {
                self.journal
                    .mark_reconciled(client_order_id, BrokerCommandStatus::Filled)
                    .await?;
            }
            ReconciliationResult::StillUnknown => {
                self.journal
                    .mark_reconcile_failed(client_order_id, "test reconciliation still unknown")
                    .await?;
            }
            ReconciliationResult::ManualInterventionRequired => {
                self.journal
                    .mark_reconcile_failed(client_order_id, "test reconciliation ambiguous")
                    .await?;
            }
        }
        Ok(self.reconciliation_result.clone())
    }
}
