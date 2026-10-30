use rust_decimal::Decimal;
use trade_engine::decision::{DecisionPortfolioSnapshot, DecisionRequest};
use trade_engine::gateway::types::{
    AccountSnapshot, CashBalance, Fill, OrderStatus, Quantity, Quote, Symbol,
};
use trade_engine::strategy::{StrategyId, StrategySignal, StrategySignalAction};
use trading_agent_kis_client::{
    AccountKey, BrokerEnvironment, ClientOrderId, OrderReference, OrderSide, OrderState,
    OrderStatusEvidence,
};

fn symbol(value: &str) -> Symbol {
    Symbol::new(value).expect("valid test symbol")
}

#[test]
fn decision_request_summarizes_orders_without_broker_references() {
    let quote = Quote {
        symbol: symbol("005930"),
        full_name: None,
        last_price: Decimal::new(73_000, 0),
    };
    let signal = StrategySignal {
        strategy_id: StrategyId::new("sma-test").expect("valid strategy id"),
        symbol: quote.symbol.clone(),
        action: StrategySignalAction::Hold,
        strength: Decimal::ZERO,
    };
    let evidence = OrderStatusEvidence {
        symbol: quote.symbol.clone(),
        side: Some(OrderSide::Buy),
        ordered_quantity: Quantity::new(1),
        filled_quantity: Quantity::new(0),
        order_price: Some(Decimal::new(73_000, 0)),
        trading_day: Some("20260426".to_owned()),
        observed_at: None,
    };
    let account = AccountSnapshot {
        cash: CashBalance {
            cash: Decimal::new(1_000_000, 0),
        },
        positions: Vec::new(),
    };
    let open_orders = [OrderStatus {
        reference: test_order_reference("open-summary"),
        state: OrderState::Accepted,
        evidence: Some(evidence.clone()),
    }];
    let today_fills = [Fill {
        reference: test_order_reference("fill-summary"),
        quantity: Quantity::new(1),
        price: Decimal::new(73_000, 0),
        evidence: Some(evidence),
    }];
    let request = DecisionRequest::dry_run(
        BrokerEnvironment::Paper,
        "5678",
        DecisionPortfolioSnapshot::new(&account, &open_orders, &today_fills),
        &quote,
        &signal,
        &[Decimal::new(73_000, 0)],
    );

    let request_json = serde_json::to_string(&request).expect("request json");
    assert!(request_json.contains("\"open_orders\""));
    assert!(request_json.contains("\"today_fills\""));
    assert!(request_json.contains("\"symbol\":\"005930\""));
    assert!(!request_json.contains("reference"));
    assert!(!request_json.contains("account_key"));
    assert!(!request_json.contains("12345678-01"));
}

fn test_order_reference(client_order_id: &str) -> OrderReference {
    OrderReference {
        client_order_id: ClientOrderId::new(client_order_id).expect("client order id"),
        broker_order_id: None,
        broker_order_orgno: None,
        exchange_code: None,
        account_key: AccountKey::new("12345678-01").expect("account key"),
        environment: BrokerEnvironment::Paper,
    }
}
