use std::fs;
use std::time::Duration;

use rust_decimal::Decimal;
use tempfile::tempdir;
use trade_engine::advisor::{
    AdvisorAccountSummary, AdvisorCurrentState, AdvisorKisApiSummary, AdvisorMarketContext,
    AdvisorMarketTickObservation, AdvisorProvider, AdvisorProviderError, AdvisorQuoteSummary,
    AdvisorRequest, AdvisorRuntimeContext, AdvisorUniverseDiscoveryContext,
    AdvisorUniverseDiscoverySource, CommandAdvisorProvider,
};
use trade_engine::gateway::types::{CurrencyCode, ExchangeCode, MarketScope, Quantity, Symbol};
use trading_agent_kis_client::BrokerEnvironment;

#[test]
fn codex_advisor_wrapper_prompts_for_organized_universe_context() {
    let script_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("scripts")
        .join("codex-advisor");
    let script = fs::read_to_string(script_path).expect("read codex advisor script");

    assert!(script.contains("runtime.universe_discovery"));
    assert!(script.contains("discovered_candidates with bucket, signals, evidence"));
    assert!(script.contains("Discovery candidates are inspection context"));
    assert!(script.contains("Chart-pattern, breakout, support/resistance"));
    assert!(script.contains("actively look for a small, risk-bounded entry"));
    assert!(script.contains("target_price_gap"));
    assert!(script.contains("current-price-to-target gap"));
    assert!(script.contains("not merely cheap versus a stale target"));
    assert!(script.contains(
        "Treat stale, single-broker, or unsourced targets as weak support unless directly observed"
    ));
    assert!(script.contains("default to hold merely because the mode is live"));
}

#[tokio::test]
async fn command_advisor_provider_sends_redacted_request_and_parses_stdout() {
    let tempdir = tempdir().expect("tempdir");
    let request_path = tempdir.path().join("request.json");
    let script_path = tempdir.path().join("advisor.sh");
    fs::write(
        &script_path,
        format!(
            r#"while IFS= read -r line || [ -n "$line" ]; do printf '%s\n' "$line"; done > "{}"
printf '%s\n' '{{"summary":"reviewed dry-run context","observations":["cash stable"],"risks":["read-only"],"next_actions":["keep observing"],"rationale":"dry-run context has no write intent","evidence":[{{"source":"runtime","detail":"mode is dry_run"}}],"proposals":[{{"decision_type":"hold","symbol":"005930","quantity":null,"order_type":null,"limit_price":null,"confidence":"0.50","time_horizon":null,"reason":"advisor hold","risk_notes":[]}}]}}'
"#,
            request_path.display()
        ),
    )
    .expect("write advisor script");

    let provider = CommandAdvisorProvider::new(
        "sh",
        vec![script_path.to_string_lossy().into_owned()],
        Duration::from_secs(5),
        4096,
        4096,
    );
    let request = advisor_request(Symbol::new("005930").expect("symbol"));

    let response = provider.advise(&request).await.expect("advisor output");

    assert_eq!(response.output.output().summary, "reviewed dry-run context");
    assert_eq!(
        response.output.output().rationale,
        "dry-run context has no write intent"
    );
    assert_eq!(response.output.output().evidence[0].source, "runtime");
    assert_eq!(
        response.output.output().proposals[0].quantity,
        None::<Quantity>
    );
    let request_json = fs::read_to_string(request_path).expect("request json");
    let request_value: serde_json::Value =
        serde_json::from_str(&request_json).expect("request json value");
    assert_eq!(request_value["schema_version"], 8);
    assert!(request_value["runtime"].get("watchlist").is_none());
    assert!(
        request_value["runtime"]["universe_discovery"]
            .get("user_watchlist")
            .is_none()
    );
    assert!(
        request_value["runtime"]["universe_discovery"]
            .get("discovered_symbols")
            .is_none()
    );
    assert!(
        request_value["runtime"]["universe_discovery"]
            .get("effective_candidates")
            .is_none()
    );
    assert!(request_json.contains("\"runtime\""));
    assert!(request_json.contains("\"market\":{\"kind\":\"domestic_stock\"}"));
    assert!(request_json.contains("\"universe_discovery\""));
    assert!(request_json.contains("\"discovered_candidates\""));
    assert!(
        request_json.contains("\"source\":{\"provider\":\"disabled\",\"status\":\"disabled\"}")
    );
    assert!(request_json.contains("\"current_state\""));
    assert!(request_json.contains("\"current_tick\""));
    assert!(request_json.contains("\"account_suffix\":\"5678\""));
    assert!(!request_json.contains("12345678-01"));
    assert!(!request_json.contains("KIS_APP_SECRET"));
}

#[tokio::test]
async fn command_advisor_provider_rejects_stdout_above_cap() {
    let tempdir = tempdir().expect("tempdir");
    let script_path = tempdir.path().join("advisor.sh");
    fs::write(
        &script_path,
        r#"while IFS= read -r line || [ -n "$line" ]; do :; done
printf '%s' '{"summary":"reviewed dry-run context","observations":["cash stable"],"risks":["read-only"],"next_actions":["keep observing"],"rationale":"dry-run context has no write intent","evidence":[],"proposals":[]}'
"#,
    )
    .expect("write advisor script");

    let provider = CommandAdvisorProvider::new(
        "sh",
        vec![script_path.to_string_lossy().into_owned()],
        Duration::from_secs(5),
        4096,
        32,
    );
    let request = advisor_request(Symbol::new("005930").expect("symbol"));

    let error = provider
        .advise(&request)
        .await
        .expect_err("over-cap stdout must be rejected");

    match error {
        AdvisorProviderError::InvalidOutput {
            raw_output_json,
            reason,
            ..
        } => {
            assert!(reason.contains("stdout exceeded max_stdout_bytes"));
            assert!(raw_output_json.contains("<truncated>"));
            assert!(raw_output_json.len() <= 64);
        }
        other => panic!("unexpected error: {other}"),
    }
}

fn advisor_request(symbol: Symbol) -> AdvisorRequest {
    AdvisorRequest {
        schema_version: trade_engine::advisor::ADVISOR_REQUEST_SCHEMA_VERSION,
        runtime: AdvisorRuntimeContext {
            mode: "dry_run".to_owned(),
            environment: BrokerEnvironment::Live.as_str().to_owned(),
            account_suffix: "5678".to_owned(),
            market: AdvisorMarketContext {
                kind: "domestic_stock".to_owned(),
                exchange_code: None,
                currency: None,
            },
            market_hours: "regular".to_owned(),
            tick_index: 1,
            market_tick_count: 1,
            user_watchlist: vec![symbol.clone()],
            discovered_universe: Vec::new(),
            effective_candidates: vec![symbol.clone()],
            universe_discovery: AdvisorUniverseDiscoveryContext {
                discovered_candidates: Vec::new(),
                source: AdvisorUniverseDiscoverySource::disabled(),
            },
        },
        current_state: AdvisorCurrentState {
            current_tick: AdvisorMarketTickObservation {
                stage: "TickObservation".to_owned(),
                market_session_open: true,
                technical_analysis: Vec::new(),
                domestic_market_alignment: None,
                kis_api: AdvisorKisApiSummary {
                    account: AdvisorAccountSummary {
                        cash: Decimal::new(1_000_000, 0),
                        position_count: 0,
                        positions: Vec::new(),
                    },
                    open_order_count: 0,
                    open_orders: Vec::new(),
                    today_fill_count: 0,
                    today_fills: Vec::new(),
                    quote_count: 1,
                    quotes: vec![AdvisorQuoteSummary {
                        symbol,
                        full_name: None,
                        last_price: Decimal::new(73_000, 0),
                        recent_prices: vec!["73000".to_owned()],
                    }],
                },
            },
        },
    }
}

#[test]
fn advisor_market_context_serializes_overseas_scope() {
    let context = AdvisorMarketContext::from_market_scope(&MarketScope::OverseasStock {
        exchange: ExchangeCode::new("NASD").expect("exchange"),
        currency: CurrencyCode::new("USD").expect("currency"),
    });

    let value = serde_json::to_value(context).expect("market context json");

    assert_eq!(
        value,
        serde_json::json!({
            "kind": "overseas_stock",
            "exchange_code": "NASD",
            "currency": "USD"
        })
    );
}
