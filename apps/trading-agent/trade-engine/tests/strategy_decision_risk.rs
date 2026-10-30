use rust_decimal::Decimal;
use trade_engine::decision::{
    DecisionOrderType, DecisionProposal, DecisionType, DecisionValidationContext,
    DecisionValidationError, MAX_DECISION_REASON_CHARS, MAX_DECISION_RISK_NOTE_CHARS,
    MAX_DECISION_RISK_NOTES,
};
use trade_engine::gateway::types::{
    AccountSnapshot, CashBalance, Fill, Position, Quantity, Quote, Symbol,
};
use trade_engine::risk::{
    DailyLossState, DailyLossUnavailableReason, RiskBlockReason, RiskEvaluationContext,
    RiskEvaluationRequest, RiskEvaluationStatus, RiskPolicy,
};
use trade_engine::strategy::{
    HoldOnlyStrategy, SmaCrossoverStrategy, StrategyInput, StrategySignalAction, TradingStrategy,
};
use trading_agent_kis_client::{
    AccountKey, BrokerEnvironment, ClientOrderId, OrderReference, OrderSide, OrderStatusEvidence,
};

fn symbol(value: &str) -> Symbol {
    Symbol::new(value).expect("valid test symbol")
}

fn allowed_symbols() -> Vec<Symbol> {
    vec![symbol("005930")]
}

fn valid_buy_limit() -> DecisionProposal {
    DecisionProposal {
        decision_type: DecisionType::Buy,
        symbol: symbol("005930"),
        quantity: Some(Quantity::new(10)),
        order_type: Some(DecisionOrderType::Limit),
        limit_price: Some(Decimal::new(73_000, 0)),
        confidence: Decimal::new(71, 2),
        time_horizon: Some("intraday".to_owned()),
        reason: "baseline test proposal".to_owned(),
        risk_notes: vec!["under max notional".to_owned()],
    }
}

fn valid_sell_limit() -> DecisionProposal {
    DecisionProposal {
        decision_type: DecisionType::Sell,
        symbol: symbol("005930"),
        quantity: Some(Quantity::new(10)),
        order_type: Some(DecisionOrderType::Limit),
        limit_price: Some(Decimal::new(73_000, 0)),
        confidence: Decimal::new(71, 2),
        time_horizon: Some("intraday".to_owned()),
        reason: "baseline test proposal".to_owned(),
        risk_notes: vec!["under max notional".to_owned()],
    }
}

#[test]
fn hold_only_strategy_emits_deterministic_hold_signal() {
    let strategy = HoldOnlyStrategy::new("hold-baseline").expect("valid strategy id");

    let signal = strategy.generate_signal(symbol("005930"));

    assert_eq!(signal.strategy_id.as_str(), "hold-baseline");
    assert_eq!(signal.symbol.as_str(), "005930");
    assert_eq!(signal.action, StrategySignalAction::Hold);
    assert_eq!(signal.strength, Decimal::ZERO);
}

#[test]
fn sma_crossover_holds_until_enough_prices_exist() {
    let strategy = SmaCrossoverStrategy::new("sma-test", 2, 3).expect("strategy");
    let prices = [Decimal::new(100, 0), Decimal::new(101, 0)];
    let input = StrategyInputFixture::new("005930", &prices, false);

    let signal = strategy.generate_signal(&input.as_strategy_input());

    assert_eq!(signal.action, StrategySignalAction::Hold);
    assert_eq!(signal.strength, Decimal::ZERO);
}

#[test]
fn sma_crossover_buys_when_short_average_crosses_above_long_average() {
    let strategy = SmaCrossoverStrategy::new("sma-test", 2, 3).expect("strategy");
    let prices = [
        Decimal::new(100, 0),
        Decimal::new(90, 0),
        Decimal::new(90, 0),
        Decimal::new(130, 0),
    ];
    let input = StrategyInputFixture::new("005930", &prices, false);

    let signal = strategy.generate_signal(&input.as_strategy_input());

    assert_eq!(signal.action, StrategySignalAction::Buy);
    assert!(signal.strength > Decimal::ZERO);
}

#[test]
fn sma_crossover_sells_when_short_average_crosses_below_long_average_and_position_exists() {
    let strategy = SmaCrossoverStrategy::new("sma-test", 2, 3).expect("strategy");
    let prices = [
        Decimal::new(90, 0),
        Decimal::new(110, 0),
        Decimal::new(110, 0),
        Decimal::new(70, 0),
    ];
    let input = StrategyInputFixture::new("005930", &prices, true);

    let signal = strategy.generate_signal(&input.as_strategy_input());

    assert_eq!(signal.action, StrategySignalAction::Sell);
    assert!(signal.strength > Decimal::ZERO);
}

#[test]
fn sma_crossover_does_not_sell_without_a_position() {
    let strategy = SmaCrossoverStrategy::new("sma-test", 2, 3).expect("strategy");
    let prices = [
        Decimal::new(90, 0),
        Decimal::new(110, 0),
        Decimal::new(110, 0),
        Decimal::new(70, 0),
    ];
    let input = StrategyInputFixture::new("005930", &prices, false);

    let signal = strategy.generate_signal(&input.as_strategy_input());

    assert_eq!(signal.action, StrategySignalAction::Hold);
    assert_eq!(signal.strength, Decimal::ZERO);
}

#[test]
fn decision_rejects_buy_limit_without_required_order_fields() {
    let context = DecisionValidationContext::new(allowed_symbols());
    let proposal = DecisionProposal {
        decision_type: DecisionType::Buy,
        symbol: symbol("005930"),
        quantity: None,
        order_type: Some(DecisionOrderType::Limit),
        limit_price: None,
        confidence: Decimal::new(80, 2),
        time_horizon: None,
        reason: "missing fields".to_owned(),
        risk_notes: Vec::new(),
    };

    let error = proposal
        .validate(&context)
        .expect_err("buy limit without quantity must be rejected");

    assert_eq!(
        error,
        DecisionValidationError::MissingQuantity {
            decision_type: DecisionType::Buy
        }
    );
}

#[test]
fn decision_rejects_unknown_symbol() {
    let context = DecisionValidationContext::new(allowed_symbols());
    let proposal = DecisionProposal {
        symbol: symbol("000660"),
        ..valid_buy_limit()
    };

    let error = proposal
        .validate(&context)
        .expect_err("unknown symbol must be rejected");

    assert_eq!(
        error,
        DecisionValidationError::UnknownSymbol {
            symbol: "000660".to_owned()
        }
    );
}

#[test]
fn hold_decision_is_valid_without_order_fields() {
    let context = DecisionValidationContext::new(allowed_symbols());
    let proposal = DecisionProposal::hold(symbol("005930"), Decimal::new(50, 2), "no trade");

    let validated = proposal
        .validate(&context)
        .expect("hold proposal is valid without order fields");

    assert!(validated.proposal().is_hold());
}

#[test]
fn decision_rejects_reason_that_exceeds_length_limit() {
    let context = DecisionValidationContext::new(allowed_symbols());
    let proposal = DecisionProposal {
        reason: "x".repeat(MAX_DECISION_REASON_CHARS + 1),
        ..valid_buy_limit()
    };

    let error = proposal
        .validate(&context)
        .expect_err("overlong reason must be rejected");

    assert_eq!(
        error,
        DecisionValidationError::ReasonTooLong {
            max_chars: MAX_DECISION_REASON_CHARS
        }
    );
}

#[test]
fn decision_rejects_risk_notes_that_exceed_limits() {
    let context = DecisionValidationContext::new(allowed_symbols());
    let proposal = DecisionProposal {
        risk_notes: vec!["note".to_owned(); MAX_DECISION_RISK_NOTES + 1],
        ..valid_buy_limit()
    };

    let error = proposal
        .validate(&context)
        .expect_err("too many risk notes must be rejected");

    assert_eq!(
        error,
        DecisionValidationError::TooManyRiskNotes {
            max_notes: MAX_DECISION_RISK_NOTES
        }
    );

    let proposal = DecisionProposal {
        risk_notes: vec!["x".repeat(MAX_DECISION_RISK_NOTE_CHARS + 1)],
        ..valid_buy_limit()
    };

    let error = proposal
        .validate(&context)
        .expect_err("overlong risk note must be rejected");

    assert_eq!(
        error,
        DecisionValidationError::RiskNoteTooLong {
            max_chars: MAX_DECISION_RISK_NOTE_CHARS
        }
    );
}

#[test]
fn risk_blocks_write_in_read_only_mode() {
    let context = DecisionValidationContext::new(allowed_symbols());
    let validated = valid_buy_limit()
        .validate(&context)
        .expect("valid buy proposal");
    let policy = RiskPolicy::new(true, false, allowed_symbols(), Decimal::new(1_000_000, 0));

    let evaluation = policy.evaluate(RiskEvaluationRequest {
        proposal: &validated,
        context: RiskEvaluationContext::default(),
    });

    assert_eq!(evaluation.status, RiskEvaluationStatus::Blocked);
    assert!(evaluation.reasons.contains(&RiskBlockReason::ReadOnlyMode));
}

#[test]
fn risk_blocks_live_write_when_live_trading_is_disabled() {
    let context = DecisionValidationContext::new(allowed_symbols());
    let validated = valid_buy_limit()
        .validate(&context)
        .expect("valid buy proposal");
    let policy = RiskPolicy::new(false, false, allowed_symbols(), Decimal::new(1_000_000, 0));

    let evaluation = policy.evaluate(RiskEvaluationRequest {
        proposal: &validated,
        context: RiskEvaluationContext::default(),
    });

    assert_eq!(evaluation.status, RiskEvaluationStatus::Blocked);
    assert!(
        evaluation
            .reasons
            .contains(&RiskBlockReason::LiveTradingDisabled)
    );
}

#[test]
fn risk_blocks_write_when_market_session_is_closed() {
    let context = DecisionValidationContext::new(allowed_symbols());
    let validated = valid_buy_limit()
        .validate(&context)
        .expect("valid buy proposal");
    let policy = RiskPolicy::new(false, true, allowed_symbols(), Decimal::new(1_000_000, 0));

    let evaluation = policy.evaluate(RiskEvaluationRequest {
        proposal: &validated,
        context: RiskEvaluationContext {
            market_session_open: false,
            ..RiskEvaluationContext::default()
        },
    });

    assert_eq!(evaluation.status, RiskEvaluationStatus::Blocked);
    assert!(evaluation.reasons.contains(&RiskBlockReason::MarketClosed));
}

#[test]
fn risk_blocks_write_when_unresolved_journal_commands_exist() {
    let context = DecisionValidationContext::new(allowed_symbols());
    let validated = valid_buy_limit()
        .validate(&context)
        .expect("valid buy proposal");
    let policy = RiskPolicy::new(false, true, allowed_symbols(), Decimal::new(1_000_000, 0));

    let evaluation = policy.evaluate(RiskEvaluationRequest {
        proposal: &validated,
        context: RiskEvaluationContext {
            unresolved_journal_commands: true,
            ..RiskEvaluationContext::default()
        },
    });

    assert_eq!(evaluation.status, RiskEvaluationStatus::Blocked);
    assert!(
        evaluation
            .reasons
            .contains(&RiskBlockReason::UnresolvedJournalCommands)
    );
}

#[test]
fn risk_blocks_write_when_panic_stop_is_active() {
    let context = DecisionValidationContext::new(allowed_symbols());
    let validated = valid_buy_limit()
        .validate(&context)
        .expect("valid buy proposal");
    let policy = RiskPolicy::new(false, true, allowed_symbols(), Decimal::new(1_000_000, 0));

    let evaluation = policy.evaluate(RiskEvaluationRequest {
        proposal: &validated,
        context: RiskEvaluationContext {
            panic_stop_active: true,
            ..RiskEvaluationContext::default()
        },
    });

    assert_eq!(evaluation.status, RiskEvaluationStatus::Blocked);
    assert!(
        evaluation
            .reasons
            .contains(&RiskBlockReason::PanicStopActive)
    );
}

#[test]
fn risk_blocks_buy_when_buying_power_is_insufficient() {
    let context = DecisionValidationContext::new(allowed_symbols());
    let validated = valid_buy_limit()
        .validate(&context)
        .expect("valid buy proposal");
    let policy = RiskPolicy::new(false, true, allowed_symbols(), Decimal::new(1_000_000, 0));

    let evaluation = policy.evaluate(RiskEvaluationRequest {
        proposal: &validated,
        context: RiskEvaluationContext {
            available_buying_power: Some(Decimal::new(10_000, 0)),
            ..RiskEvaluationContext::default()
        },
    });

    assert_eq!(evaluation.status, RiskEvaluationStatus::Blocked);
    assert!(
        evaluation
            .reasons
            .contains(&RiskBlockReason::InsufficientBuyingPower {
                required: Decimal::new(730_000, 0),
                available: Decimal::new(10_000, 0)
            })
    );
}

#[test]
fn risk_blocks_sell_when_quantity_exceeds_current_position() {
    let context = DecisionValidationContext::new(allowed_symbols());
    let validated = valid_sell_limit()
        .validate(&context)
        .expect("valid sell proposal");
    let policy = RiskPolicy::new(false, true, allowed_symbols(), Decimal::new(1_000_000, 0));

    let evaluation = policy.evaluate(RiskEvaluationRequest {
        proposal: &validated,
        context: RiskEvaluationContext {
            current_position_quantity: Quantity::new(5),
            ..RiskEvaluationContext::default()
        },
    });

    assert_eq!(evaluation.status, RiskEvaluationStatus::Blocked);
    assert!(
        evaluation
            .reasons
            .contains(&RiskBlockReason::SellQuantityExceedsPosition {
                requested: Quantity::new(10),
                available: Quantity::new(5)
            })
    );
}

#[test]
fn risk_approves_sell_when_quantity_matches_current_position() {
    let context = DecisionValidationContext::new(allowed_symbols());
    let validated = valid_sell_limit()
        .validate(&context)
        .expect("valid sell proposal");
    let policy = RiskPolicy::new(false, true, allowed_symbols(), Decimal::new(1_000_000, 0));

    let evaluation = policy.evaluate(RiskEvaluationRequest {
        proposal: &validated,
        context: RiskEvaluationContext {
            current_position_quantity: Quantity::new(10),
            ..RiskEvaluationContext::default()
        },
    });

    assert_eq!(evaluation.status, RiskEvaluationStatus::Approved);
    assert!(evaluation.reasons.is_empty());
}

#[test]
fn risk_blocks_duplicate_order_intent() {
    let context = DecisionValidationContext::new(allowed_symbols());
    let validated = valid_buy_limit()
        .validate(&context)
        .expect("valid buy proposal");
    let policy = RiskPolicy::new(false, true, allowed_symbols(), Decimal::new(1_000_000, 0));

    let evaluation = policy.evaluate(RiskEvaluationRequest {
        proposal: &validated,
        context: RiskEvaluationContext {
            duplicate_order_intent: true,
            ..RiskEvaluationContext::default()
        },
    });

    assert_eq!(evaluation.status, RiskEvaluationStatus::Blocked);
    assert!(
        evaluation
            .reasons
            .contains(&RiskBlockReason::DuplicateOrderIntent)
    );
}

#[test]
fn risk_blocks_exposure_and_daily_limit() {
    let context = DecisionValidationContext::new(allowed_symbols());
    let validated = valid_buy_limit()
        .validate(&context)
        .expect("valid buy proposal");
    let policy = RiskPolicy::new(false, true, allowed_symbols(), Decimal::new(1_000_000, 0))
        .with_position_notional_limit(Decimal::new(2_000_000, 0))
        .with_daily_order_limit(3);

    let evaluation = policy.evaluate(RiskEvaluationRequest {
        proposal: &validated,
        context: RiskEvaluationContext {
            projected_position_notional: Some(Decimal::new(2_500_000, 0)),
            submitted_order_count_today: 3,
            ..RiskEvaluationContext::default()
        },
    });

    assert_eq!(evaluation.status, RiskEvaluationStatus::Blocked);
    assert!(
        evaluation
            .reasons
            .contains(&RiskBlockReason::MaxPositionNotionalExceeded {
                projected_notional: Decimal::new(2_500_000, 0),
                max_position_notional: Decimal::new(2_000_000, 0)
            })
    );
    assert!(
        evaluation
            .reasons
            .contains(&RiskBlockReason::MaxDailyOrderCountExceeded {
                submitted_orders: 3,
                max_daily_orders: 3
            })
    );
}

#[test]
fn risk_blocks_when_daily_loss_limit_is_reached() {
    let context = DecisionValidationContext::new(allowed_symbols());
    let validated = valid_buy_limit()
        .validate(&context)
        .expect("valid buy proposal");
    let policy = RiskPolicy::new(false, true, allowed_symbols(), Decimal::new(1_000_000, 0))
        .with_daily_loss_limit(Decimal::new(500_000, 0));

    let evaluation = policy.evaluate(RiskEvaluationRequest {
        proposal: &validated,
        context: RiskEvaluationContext {
            realized_loss_today: DailyLossState::Known(Decimal::new(500_000, 0)),
            ..RiskEvaluationContext::default()
        },
    });

    assert_eq!(evaluation.status, RiskEvaluationStatus::Blocked);
    assert!(
        evaluation
            .reasons
            .contains(&RiskBlockReason::MaxDailyLossExceeded {
                realized_loss: Decimal::new(500_000, 0),
                max_daily_loss: Decimal::new(500_000, 0)
            })
    );
}

#[test]
fn risk_blocks_when_daily_loss_cannot_be_computed() {
    let context = DecisionValidationContext::new(allowed_symbols());
    let validated = valid_buy_limit()
        .validate(&context)
        .expect("valid buy proposal");
    let policy = RiskPolicy::new(false, true, allowed_symbols(), Decimal::new(1_000_000, 0))
        .with_daily_loss_limit(Decimal::new(500_000, 0));

    let evaluation = policy.evaluate(RiskEvaluationRequest {
        proposal: &validated,
        context: RiskEvaluationContext {
            realized_loss_today: DailyLossState::Unavailable(
                DailyLossUnavailableReason::MissingCostBasis,
            ),
            ..RiskEvaluationContext::default()
        },
    });

    assert_eq!(evaluation.status, RiskEvaluationStatus::Blocked);
    assert!(
        evaluation
            .reasons
            .contains(&RiskBlockReason::DailyLossUnavailable {
                reason: DailyLossUnavailableReason::MissingCostBasis
            })
    );
}

#[test]
fn daily_loss_uses_supplemental_cost_basis_for_sold_out_fill() {
    let sold_symbol = symbol("111111");
    let account = AccountSnapshot {
        cash: CashBalance {
            cash: Decimal::new(1_000_000, 0),
        },
        positions: vec![Position {
            symbol: symbol("005930"),
            quantity: Quantity::new(10),
            average_price: Decimal::new(73_000, 0),
        }],
    };
    let fills = [Fill {
        reference: OrderReference {
            client_order_id: ClientOrderId::new("sold-out-fill").expect("client order id"),
            broker_order_id: None,
            broker_order_orgno: None,
            exchange_code: None,
            account_key: AccountKey::new("12345678-01").expect("account key"),
            environment: BrokerEnvironment::Paper,
        },
        quantity: Quantity::new(8),
        price: Decimal::new(22_4300, 4),
        evidence: Some(OrderStatusEvidence {
            symbol: sold_symbol.clone(),
            side: Some(OrderSide::Sell),
            ordered_quantity: Quantity::new(8),
            filled_quantity: Quantity::new(8),
            order_price: Some(Decimal::new(22_4300, 4)),
            trading_day: Some("20260622".to_owned()),
            observed_at: None,
        }),
    }];

    let state = DailyLossState::from_account_fills_and_cost_basis(
        &account,
        &fills,
        &[(sold_symbol, Decimal::new(25_6312, 4))],
    );

    assert_eq!(state, DailyLossState::Known(Decimal::new(25_6096, 4)));
}

#[test]
fn risk_approves_simple_write_when_live_trading_is_enabled() {
    let context = DecisionValidationContext::new(allowed_symbols());
    let validated = valid_buy_limit()
        .validate(&context)
        .expect("valid buy proposal");
    let policy = RiskPolicy::new(false, true, allowed_symbols(), Decimal::new(1_000_000, 0));

    let evaluation = policy.evaluate(RiskEvaluationRequest {
        proposal: &validated,
        context: RiskEvaluationContext::default(),
    });

    assert_eq!(evaluation.status, RiskEvaluationStatus::Approved);
    assert!(evaluation.reasons.is_empty());
    assert_eq!(evaluation.order_notional, Some(Decimal::new(730_000, 0)));
}

struct StrategyInputFixture<'a> {
    account: AccountSnapshot,
    quote: Quote,
    prices: &'a [Decimal],
}

impl<'a> StrategyInputFixture<'a> {
    fn new(symbol: &str, prices: &'a [Decimal], has_position: bool) -> Self {
        let symbol = Symbol::new(symbol).expect("valid test symbol");
        Self {
            account: AccountSnapshot {
                cash: CashBalance {
                    cash: Decimal::new(1_000_000, 0),
                },
                positions: if has_position {
                    vec![Position {
                        symbol: symbol.clone(),
                        quantity: Quantity::new(10),
                        average_price: Decimal::new(100, 0),
                    }]
                } else {
                    Vec::new()
                },
            },
            quote: Quote {
                symbol,
                full_name: None,
                last_price: *prices.last().expect("at least one price"),
            },
            prices,
        }
    }

    fn as_strategy_input(&self) -> StrategyInput<'_> {
        StrategyInput {
            account: &self.account,
            quote: &self.quote,
            recent_prices: self.prices,
        }
    }
}
