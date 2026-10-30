use rust_decimal::Decimal;
use trading_agent_kis_client::{
    AccountKey, BrokerEnvironment, ClientOrderId, CreateOrderRequest, OrderReference, OrderSide,
    OrderType, Quantity, Symbol,
};

#[test]
fn order_reference_requires_broker_native_fields_for_follow_up_writes() {
    let reference = OrderReference {
        client_order_id: ClientOrderId::new("coid-0001").expect("valid client order id"),
        broker_order_id: None,
        broker_order_orgno: None,
        exchange_code: None,
        account_key: AccountKey::new("12345678-01").expect("valid account key"),
        environment: BrokerEnvironment::Paper,
    };

    assert!(!reference.is_complete_for_follow_up());

    let completed = reference
        .with_broker_order_id("0000001234")
        .expect("valid broker order id")
        .with_broker_order_orgno("001")
        .expect("valid broker org number");

    assert!(completed.is_complete_for_follow_up());
}

#[test]
fn create_order_request_rejects_zero_quantity() {
    let request = CreateOrderRequest::cash_limit(
        ClientOrderId::new("coid-0002").expect("valid client order id"),
        AccountKey::new("12345678-01").expect("valid account key"),
        Symbol::new("005930").expect("valid symbol"),
        OrderSide::Buy,
        Quantity::new(0),
        Decimal::new(72_300, 0),
    );

    assert!(request.is_err());
}

#[test]
fn create_order_request_keeps_decimal_price_without_float_conversion() {
    let request = CreateOrderRequest::cash_limit(
        ClientOrderId::new("coid-0003").expect("valid client order id"),
        AccountKey::new("12345678-01").expect("valid account key"),
        Symbol::new("005930").expect("valid symbol"),
        OrderSide::Sell,
        Quantity::new(10),
        Decimal::new(72_300, 0),
    )
    .expect("valid limit order");

    assert_eq!(request.order_type, OrderType::Limit);
    assert_eq!(request.limit_price, Some(Decimal::new(72_300, 0)));
}
