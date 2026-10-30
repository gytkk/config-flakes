use rust_decimal::Decimal;
use trade_engine::gateway::testing::MockTradingGateway;
use trade_engine::gateway::traits::TradingGateway;
use trade_engine::gateway::types::{
    AmendOrderRequest, AmendResult, CancelOrderRequest, CancelResult, Quantity,
};
use trading_agent_kis_client::{
    AccountKey, BrokerEnvironment, BrokerOrderId, BrokerOrderOrgNo, ClientOrderId, OrderReference,
};

#[tokio::test]
async fn mock_gateway_queues_cancel_and_amend_results() {
    let parent = order_reference("parent-order");
    let cancel_request = CancelOrderRequest {
        client_order_id: ClientOrderId::new("cancel-order").expect("cancel client id"),
        reference: parent.clone(),
        quantity: Some(Quantity::new(1)),
    };
    let amend_request = AmendOrderRequest {
        client_order_id: ClientOrderId::new("amend-order").expect("amend client id"),
        reference: parent.clone(),
        quantity: Some(Quantity::new(1)),
        limit_price: Some(Decimal::new(72_000, 0)),
    };
    let gateway = MockTradingGateway::new()
        .with_cancel_result(CancelResult {
            reference: parent.clone(),
            accepted: true,
        })
        .with_amend_result(AmendResult {
            reference: parent.clone(),
            accepted: true,
        });

    let cancel = gateway
        .cancel_order(cancel_request)
        .await
        .expect("cancel result");
    let amend = gateway
        .amend_order(amend_request)
        .await
        .expect("amend result");

    assert!(cancel.accepted);
    assert!(amend.accepted);
    assert_eq!(gateway.call_counts().cancel_order, 1);
    assert_eq!(gateway.call_counts().amend_order, 1);
}

fn order_reference(client_order_id: &str) -> OrderReference {
    OrderReference {
        client_order_id: ClientOrderId::new(client_order_id).expect("client id"),
        broker_order_id: Some(BrokerOrderId::new("broker-order").expect("broker order id")),
        broker_order_orgno: Some(BrokerOrderOrgNo::new("001").expect("broker orgno")),
        exchange_code: None,
        account_key: AccountKey::new("12345678-01").expect("account key"),
        environment: BrokerEnvironment::Paper,
    }
}
