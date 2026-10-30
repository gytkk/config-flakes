use rust_decimal::{Decimal, RoundingStrategy};
use trading_agent_kis_client::OrderSide;

pub(crate) fn normalize_overseas_stock_limit_price(
    side: OrderSide,
    limit_price: Decimal,
) -> Decimal {
    if limit_price < Decimal::ONE {
        return limit_price;
    }
    match side {
        OrderSide::Buy => limit_price.round_dp_with_strategy(2, RoundingStrategy::AwayFromZero),
        OrderSide::Sell => limit_price.round_dp_with_strategy(2, RoundingStrategy::ToZero),
    }
}
