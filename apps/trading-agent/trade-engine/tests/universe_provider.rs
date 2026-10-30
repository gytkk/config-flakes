use std::fs;
use std::time::Duration;

use rust_decimal::Decimal;
use tempfile::tempdir;
use trade_engine::gateway::types::Symbol;
use trade_engine::universe::{
    CommandUniverseDiscoveryProvider, UniverseDiscoveryBucket, UniverseDiscoveryMarketContext,
    UniverseDiscoveryOutput, UniverseDiscoveryProvider, UniverseDiscoveryProviderError,
    UniverseDiscoveryRequest, UniverseDiscoveryRiskFlag, UniverseDiscoveryRuntimeContext,
    UniverseDiscoverySignal, UniverseDiscoveryTargetFreshness,
};
use trading_agent_kis_client::BrokerEnvironment;

#[test]
fn codex_universe_wrapper_prompts_for_proactive_broad_discovery() {
    let script_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("scripts")
        .join("codex-universe");
    let script = fs::read_to_string(script_path).expect("read codex universe script");

    assert!(script.contains("proactive, broad cross-sector discovery scout"));
    assert!(
        script.contains("Return a useful expanded universe even when the user_watchlist is narrow")
    );
    assert!(script.contains("Use multiple independent buckets"));
    assert!(script.contains("Spend the extra refresh budget on deeper cross-checks"));
    assert!(script.contains("rank candidates by evidence quality"));
    assert!(
        script.contains(
            "Explicitly hunt for current-price versus broker/consensus target-price gaps"
        )
    );
    assert!(script.contains("candidate.target_support"));
    assert!(script.contains("stale_target_price, single_broker_target, or unverified_source"));
    assert!(script.contains("Demote or exclude those gaps unless fresh price/volume action or a"));
    assert!(script.contains("Prefer fewer high-conviction candidates over a broad but weak list"));
    assert!(script.contains("Treat breakout, support/resistance, and indicator labels"));
    assert!(script.contains("liquidity"));
    assert!(script.contains("valuation"));
    assert!(script.contains("short_term_momentum"));
    assert!(script.contains("target_price_gap"));
    assert!(script.contains("When runtime.market.kind is domestic_stock"));
    assert!(script.contains("When runtime.market.kind is overseas_stock"));
    assert!(script.contains("primary-listed on the configured"));
    assert!(script.contains("For NASD return NASDAQ-primary symbols only"));
    assert!(script.contains("NYSE return NYSE-primary symbols only"));
    assert!(script.contains("AMEX return AMEX-primary symbols only"));
    assert!(script.contains("KRX six-digit"));
    assert!(
        script.contains(
            "Never force an empty candidates array just because runtime context is sparse"
        )
    );
    assert!(script.contains("--model gpt-5.4"));
    assert!(script.contains("--disable fast_mode"));
    assert!(!script.contains("--enable fast_mode"));
    assert!(!script.contains("service_tier=\"fast\""));
}

#[test]
fn universe_output_schema_requires_bucketed_audit_evidence() {
    let schema_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("schemas")
        .join("universe-output.schema.json");
    let schema_json = fs::read_to_string(schema_path).expect("read universe schema");
    let schema: serde_json::Value = serde_json::from_str(&schema_json).expect("schema json");
    let required = schema["properties"]["candidates"]["items"]["required"]
        .as_array()
        .expect("candidate required fields");

    assert!(schema_json.contains("target_price_gap"));
    assert!(schema_json.contains("target_support"));
    assert!(schema_json.contains("source_count"));
    assert!(schema_json.contains("consensus"));
    assert!(schema_json.contains("freshness"));
    assert!(schema_json.contains("analyst_target_upside"));
    assert!(schema_json.contains("consensus_target_upside"));
    assert!(schema_json.contains("target_price_revision"));
    assert!(schema_json.contains("stale_target_price"));
    assert!(schema_json.contains("single_broker_target"));

    for field in ["bucket", "signals", "evidence", "risk_flags"] {
        assert!(
            required.iter().any(|item| item.as_str() == Some(field)),
            "missing required candidate field {field}"
        );
    }
}

#[test]
fn legacy_universe_output_defaults_candidate_enrichment() {
    let output: UniverseDiscoveryOutput = serde_json::from_str(
        r#"{"summary":"legacy snapshot","candidates":[{"symbol":"000660","reason":"legacy candidate","confidence":"0.62"}]}"#,
    )
    .expect("legacy output json");
    let output = output.validate().expect("legacy output validates");
    let candidate = &output.output().candidates[0];

    assert_eq!(candidate.bucket, None);
    assert_eq!(candidate.target_support, None);
    assert!(candidate.signals.is_empty());
    assert!(candidate.evidence.is_empty());
    assert!(candidate.risk_flags.is_empty());
}

#[tokio::test]
async fn command_universe_provider_sends_request_and_parses_stdout() {
    let tempdir = tempdir().expect("tempdir");
    let request_path = tempdir.path().join("request.json");
    let script_path = tempdir.path().join("universe.sh");
    fs::write(
        &script_path,
        format!(
            r#"while IFS= read -r line || [ -n "$line" ]; do printf '%s\n' "$line"; done > "{}"
printf '%s\n' '{{"summary":"found adjacent semiconductor candidate","candidates":[{{"symbol":"000660","bucket":"short_term_momentum","target_support":{{"source_count":2,"consensus":true,"freshness":"recent"}},"signals":["volume_breakout","relative_strength"],"evidence":[{{"metric":"20d volume percentile","value":"top quartile","as_of":"latest available tick","source":"runtime discovery context"}}],"risk_flags":["unverified_source"],"reason":"same sector momentum candidate","confidence":"0.62"}}]}}'
"#,
            request_path.display()
        ),
    )
    .expect("write universe script");

    let provider = CommandUniverseDiscoveryProvider::new(
        "sh",
        vec![script_path.to_string_lossy().into_owned()],
        Duration::from_secs(5),
        4096,
        4096,
    );
    let request = universe_request();

    let response = provider.discover(&request).await.expect("universe output");

    assert_eq!(
        response.output.output().summary,
        "found adjacent semiconductor candidate"
    );
    assert_eq!(
        response.output.output().candidates[0].symbol,
        Symbol::new("000660").expect("symbol")
    );
    assert_eq!(
        response.output.output().candidates[0].confidence,
        Decimal::new(62, 2)
    );
    assert_eq!(
        response.output.output().candidates[0].bucket,
        Some(UniverseDiscoveryBucket::ShortTermMomentum)
    );
    assert_eq!(
        response.output.output().candidates[0].signals,
        vec![
            UniverseDiscoverySignal::VolumeBreakout,
            UniverseDiscoverySignal::RelativeStrength
        ]
    );
    assert_eq!(
        response.output.output().candidates[0].risk_flags,
        vec![UniverseDiscoveryRiskFlag::UnverifiedSource]
    );
    assert_eq!(
        response.output.output().candidates[0]
            .target_support
            .as_ref()
            .expect("target support")
            .source_count,
        Some(2)
    );
    assert_eq!(
        response.output.output().candidates[0]
            .target_support
            .as_ref()
            .expect("target support")
            .consensus,
        Some(true)
    );
    assert_eq!(
        response.output.output().candidates[0]
            .target_support
            .as_ref()
            .expect("target support")
            .freshness,
        Some(UniverseDiscoveryTargetFreshness::Recent)
    );
    assert_eq!(
        response.output.output().candidates[0].evidence[0].metric,
        "20d volume percentile"
    );
    let request_json = fs::read_to_string(request_path).expect("request json");
    assert!(request_json.contains("\"runtime\""));
    assert!(request_json.contains("\"market\":{\"kind\":\"domestic_stock\""));
    assert!(request_json.contains("\"user_watchlist\""));
    assert!(request_json.contains("\"account_suffix\":\"5678\""));
    assert!(!request_json.contains("12345678-01"));
    assert!(!request_json.contains("KIS_APP_SECRET"));
}

#[tokio::test]
async fn command_universe_provider_rejects_stdout_above_cap() {
    let tempdir = tempdir().expect("tempdir");
    let script_path = tempdir.path().join("universe.sh");
    fs::write(
        &script_path,
        r#"while IFS= read -r line || [ -n "$line" ]; do :; done
printf '%s' '{"summary":"found adjacent semiconductor candidate","candidates":[{"symbol":"000660","reason":"same sector momentum candidate","confidence":"0.62"}]}'
"#,
    )
    .expect("write universe script");

    let provider = CommandUniverseDiscoveryProvider::new(
        "sh",
        vec![script_path.to_string_lossy().into_owned()],
        Duration::from_secs(5),
        4096,
        32,
    );

    let error = provider
        .discover(&universe_request())
        .await
        .expect_err("over-cap stdout must be rejected");

    match error {
        UniverseDiscoveryProviderError::InvalidOutput {
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

fn universe_request() -> UniverseDiscoveryRequest {
    UniverseDiscoveryRequest {
        schema_version: trade_engine::universe::UNIVERSE_DISCOVERY_REQUEST_SCHEMA_VERSION,
        runtime: UniverseDiscoveryRuntimeContext {
            mode: "dry_run".to_owned(),
            environment: BrokerEnvironment::Live.as_str().to_owned(),
            account_suffix: "5678".to_owned(),
            market: UniverseDiscoveryMarketContext {
                kind: "domestic_stock".to_owned(),
                exchange_code: None,
                currency: None,
            },
            market_hours: "regular".to_owned(),
            tick_index: 1,
            market_tick_count: 1,
        },
        user_watchlist: vec![Symbol::new("005930").expect("symbol")],
    }
}

#[test]
fn universe_request_serializes_overseas_market_scope() {
    let request = UniverseDiscoveryRequest {
        schema_version: trade_engine::universe::UNIVERSE_DISCOVERY_REQUEST_SCHEMA_VERSION,
        runtime: UniverseDiscoveryRuntimeContext {
            mode: "universe_refresh".to_owned(),
            environment: BrokerEnvironment::Live.as_str().to_owned(),
            account_suffix: "5678".to_owned(),
            market: UniverseDiscoveryMarketContext {
                kind: "overseas_stock".to_owned(),
                exchange_code: Some("NASD".to_owned()),
                currency: Some("USD".to_owned()),
            },
            market_hours: "refresh".to_owned(),
            tick_index: 1,
            market_tick_count: 1,
        },
        user_watchlist: vec![Symbol::new("AAPL").expect("symbol")],
    };

    let request_json = serde_json::to_value(&request).expect("request json");

    assert_eq!(
        request_json["runtime"]["market"],
        serde_json::json!({
            "kind": "overseas_stock",
            "exchange_code": "NASD",
            "currency": "USD"
        })
    );
}
