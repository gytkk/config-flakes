# Technical Analysis Improvement Plan

Access date: 2026-05-03

This plan proposes practical improvements for this repository using publicly
available chapter/topic themes from John J. Murphy's `Technical Analysis of the
Financial Markets`. It does not rely on full book text. The Murphy-derived ideas
below are inferred from public table-of-contents entries, publisher/catalog
descriptions, selected Google Books contents, and secondary summaries. Where
the plan maps a book theme to an implementation idea, treat it as a design
interpretation, not a claim about Murphy's exact wording.

This is a planning document only. It intentionally does not modify source code,
configs, schemas, or migrations.

## Sources Consulted

- Penguin Random House product page for `Technical Analysis of the Financial
  Markets`, which describes the 1999 edition as an update of `Technical Analysis
  of the Futures Markets` expanded across financial markets, with material on
  computer technology, technical tools, indicators, candlestick charting,
  intermarket relationships, stocks, and stock rotation:
  <https://www.penguinrandomhouse.com/books/350647/technical-analysis-of-the-financial-markets-by-john-j-murphy/>
- Windsor Books table of contents listing chapters on philosophy, Dow Theory,
  chart construction, trend, reversal and continuation patterns, volume and
  open interest, long-term charts, moving averages, oscillators, point and
  figure charting, candlesticks, Elliott Wave, cycles, trading systems, money
  management, intermarket analysis, stock market indicators, Market Profile,
  and system building:
  <https://windsorpublishing.com/product/technical-analysis-of-the-financial-markets/>
- NYPL catalog summary and contents, confirming the major topic sequence and
  describing coverage of chart construction, trends, patterns, moving averages,
  oscillators, time cycles, market interrelationships, money management, and
  trading tactics:
  <https://www.nypl.org/research/research-catalog/bib/cb2257684>
- Google Books selected contents, used only as high-level topic evidence for
  subtopics such as trendlines, price gaps, volume, moving average envelopes,
  oscillator usage with trend, ROC, RSI threshold usage, cycles, and system
  trading:
  <https://books.google.com/books/about/Technical_Analysis_of_the_Financial_Mark.html?id=teitAAAAQBAJ>
- TraderLion public summary, used as a secondary summary only for broad themes
  such as trend hierarchy, support/resistance, pattern confirmation, moving
  averages, oscillators, intermarket analysis, risk management, and system
  design:
  <https://traderlion.com/trading-books/technical-analysis-of-the-financial-markets/>
- Schwab technical-analysis overview, used for general concepts and cautions:
  trend, support/resistance, price patterns, indicators, moving averages, and
  the warning that technical analysis is not a guarantee and should not be the
  only research input:
  <https://advisorservices.schwab.com/story/investing-basics-technical-analysis>
- FINRA Rule 2214, used for validation and disclosure expectations around
  investment analysis tools: disclose criteria, methodology, assumptions,
  limitations, universe selection, and the hypothetical/non-guaranteed nature
  of projections:
  <https://www.finra.org/rules-guidance/rulebooks/finra-rules/2214>
- Sullivan, Timmermann, and White summary page via LSE Financial Markets Group,
  used for the data-snooping caution around technical trading rules:
  <https://www.fmg.ac.uk/publications/discussion-papers/data-snooping-technical-trading-rule-performance-and-bootstrap>
- Novy-Marx NBER working paper summary on multiple-signal backtesting, used for
  the caution that combining many signals can create severe overfitting bias:
  <https://www.nber.org/papers/w21329>

## Repository Understanding

This repository is a Rust-first trading-agent product with two main crates:

- `kis-client`: deterministic KIS broker adapter. It owns KIS auth, token cache,
  transport, request/response mapping, domestic and overseas quote/account/order
  APIs, live-write guardrails, durable broker command journal contracts, and
  reconciliation helpers. It intentionally does not own strategy, decision,
  product database, runtime orchestration, or operator workflows.
- `trade-engine`: product runtime. It owns config loading, startup doctor,
  gateway abstraction, KIS gateway adapter wiring, market profile/session
  timing, runtime run/tick lifecycle, SQLite audit storage, strategy signals,
  decision/advisor/universe command-provider boundaries, risk policy, execution
  workflows, reconciliation workflow, and the minimal CLI surface
  (`doctor`, `run`, `universe`).

The current implementation already has several strong foundations for
technical-analysis work:

- A strict separation between deterministic execution/risk code and advisory or
  decision proposal code.
- Typed domain boundaries for symbols, quantities, prices, environments,
  market scope, order side, and decision proposal fields.
- SQLite audit tables and migrations for runtime runs/ticks, portfolio
  snapshots, market snapshots, strategy signals, decision inputs/outputs,
  proposals, risk evaluations/blocks, execution requests/events, broker journal
  records, fills, operator actions, panic stop state, and universe snapshots.
- Built-in `hold-only` and `sma-crossover-v1` strategies.
- Command-based advisor and universe discovery providers with JSON schemas,
  length limits, and audit storage.
- Risk gates for read-only mode, live-trading enablement, market session,
  unresolved journal commands, panic stop, allowed symbols, buying power,
  duplicate intent, max order notional, max position notional, max daily order
  count, and max daily loss.
- Execution flow that keeps broker writes behind durable storage, final risk
  checks, broker command journal records, and reconciliation paths.
- Tests covering strategy/decision/risk behavior, runtime loop behavior,
  storage audit, execution storage, KIS API contracts, live guardrails, and
  reconciliation.

The important current gaps for technical analysis are not primarily strategy
ideas. They are data, feature, and validation gaps:

- The runtime mostly works with current quotes and recently stored last prices.
  There is no first-class OHLCV bar model, interval normalization, adjusted
  price handling, or volume feature layer.
- There is no auditable `market_features` or `technical_features` layer that
  records computed trend, momentum, volatility, volume, support/resistance, or
  pattern evidence independently from advisor text.
- The existing SMA crossover is useful as a baseline, but it is only one
  trend-following signal and does not yet use higher-timeframe context,
  volume confirmation, volatility-adjusted sizing/stops, or explicit
  invalidation levels.
- There is no integrated backtest sidecar, walk-forward experiment harness,
  paper-trading promotion record, or strategy certification gate tied to
  measured evidence.
- Universe discovery already has signals such as `price_breakout`,
  `volume_breakout`, and `relative_strength`, but those are currently
  provider-supplied evidence labels rather than deterministic feature outputs
  from a local market-data pipeline.
- Intermarket and sector-rotation concepts are not supported as deterministic
  data features. They would require benchmarks, sector/industry metadata,
  ETFs/proxies, macro/commodity/rates/currency inputs, or other trusted
  external data.

The core design implication: technical-analysis improvements should enter
through `trade-engine` as auditable features, experiments, and risk inputs. They
should not be added to `kis-client`, and they should not allow an advisor or LLM
to bypass deterministic validation, risk, journal, execution, or reconciliation.

## Murphy Theme Mapping

| Public Murphy theme | Current repository anchor | Gap | Practical implication |
| --- | --- | --- | --- |
| Philosophy of technical analysis | V4 separates analysis/proposals from execution authority | The system does not yet define a reusable `context -> signal -> risk` technical workflow | Treat TA as evidence generation, not prediction. Persist context, signal, invalidation, and risk before any proposal can matter. |
| Dow Theory and trend hierarchy | `MarketProfile`, runtime tick history, current SMA strategy | No primary/secondary/minor trend model or multi-timeframe state | Add higher-timeframe trend context before adding many entry indicators. For example, daily/weekly trend filters should gate shorter-term entries. |
| Chart construction | `market_snapshots`, `recent_market_prices`, gateway quotes | No OHLCV bars, interval metadata, adjusted prices, high/low/volume, or session-aware bar building | Define bar data contracts first. Indicators and patterns should depend on typed bar series, not ad hoc last-price lists. |
| Basic trend concepts | `sma-crossover-v1`, strategy signal audit | No support/resistance, pivots, channels, trendline, retracement, or role-reversal features | Build deterministic trend structure features with confidence and invalidation levels before allowing pattern-driven proposals. |
| Reversal and continuation patterns | Advisor can describe observations; universe can label breakouts | No local pattern detector or objective pattern evidence | Keep patterns as candidate annotations until detection rules are testable. Require volume/trend/context confirmation and measured invalidation. |
| Volume and open interest | Gateway quote/account/order/fill paths; universe signal labels | Equity volume is not first-class in stored snapshots; open interest is mostly not relevant to current stock scope | Prioritize equity volume/trading-value features. Treat futures open interest and continuous futures as out of scope unless product scope changes. |
| Long-term charts | Runtime can persist snapshots over time | No daily/weekly/monthly aggregation or long lookback storage policy | Add long-horizon context for regime and trend filters. Avoid intraday-only signals without higher-timeframe context. |
| Moving averages | Existing SMA crossover strategy | Only SMA crossover; no EMA, envelopes, band tests, slope, distance-from-average, or baseline comparison | Keep SMA as benchmark. Add any new MA variant only through experiment records and parameter stability checks. |
| Oscillators and contrary opinion | Decision/advisor/risk can carry structured evidence | No RSI, stochastic, MACD, ROC, divergence, or range/trend regime classifier | Add oscillators only after trend/range regime classification. Oscillators should not trigger standalone live writes. |
| Point and figure / candlesticks / Elliott Wave / cycles | Advisor can emit evidence, but deterministic runtime has no pattern layer | These can be subjective or parameter-heavy | Defer. If explored, use them as audited research features first, never as sole execution authority. |
| Computers and trading systems | Rust runtime, command-provider schemas, tests, audit store | No research harness or promotion gate | Build a reproducible experiment workflow before adding strategy variety. This maps strongly to the repo's deterministic/auditable design. |
| Money management and tactics | `RiskPolicy`, sizing defaults, stop-loss/take-profit exit policy, final execution risk | Stops and sizing are mostly percentage/notional based rather than volatility/structure based | Add ATR or volatility-aware sizing and structure-based invalidation as risk inputs, not advisor-only notes. |
| Intermarket analysis and stock market indicators | `MarketScope`, universe discovery, domestic/overseas support | No benchmark/sector/ETF/macro relationship data | Long-term improvement: add benchmark and sector-relative strength first; add commodities/rates/currency only with trusted data and clear scope. |
| Pulling it together/checklist | Startup doctor, runtime tick lifecycle, schemas, risk blocks | No TA-specific checklist for signal eligibility and promotion | Define a checklist: data freshness, market regime, trend context, signal, confirmation, invalidation, sizing, risk gate, audit evidence. |

## Guiding Principles

1. Start with data contracts, not indicators.
   A technical-analysis feature layer is only useful if it knows what data it
   used, when it was observed, how stale it is, and what market/session it
   belongs to.

2. Keep features deterministic and advisor-readable.
   Advisors may summarize evidence or propose decisions, but deterministic Rust
   should compute feature values that can be audited and replayed.

3. Treat technical patterns as hypotheses.
   Pattern names should not be accepted as evidence unless there is a stored
   rule output behind them.

4. Promote only after measured evidence.
   A feature can be displayed before it can trade. A signal can be backtested
   before it can paper trade. A paper-traded strategy can be certified before it
   is allowed to influence live submit mode.

5. Prefer fewer, explainable signals.
   The repository already values safety and auditability. A small set of
   explainable, tested features fits better than a large opaque indicator pack.

6. Never let TA weaken existing live-write invariants.
   Any roadmap item that creates orders must still pass current live enablement,
   account confirmation, journal, single-writer, unresolved command, panic stop,
   market session, final risk, and reconciliation rules.

## Prioritized Roadmap

### Near Term: Planning, Data, and Baselines

These items prepare for code changes but do not require them yet.

1. Define a technical-analysis feature glossary.
   Document feature names, formulas, required input fields, lookback windows,
   timeframe, data freshness limits, and interpretation boundaries. Start with:
   SMA slope/crossover, EMA, ATR, RSI, MACD, ROC, volume trend, relative
   strength versus benchmark, simple pivot highs/lows, support/resistance zones,
   and breakout/retest flags.

2. Inventory current data availability against feature needs.
   For each target market (`domestic_stock`, `overseas_stock`), record whether
   KIS or another approved data source can provide open, high, low, close,
   volume, adjusted close, trading value, historical bars, corporate actions,
   benchmark series, and sector metadata. This must use official provider docs
   before endpoint implementation.

3. Define a `bar_series` contract before adding indicators.
   The minimum future contract should include symbol, market scope, exchange,
   currency, interval, session, open/high/low/close, volume if available,
   observed_at, source, adjustment policy, and data_quality flags.

4. Preserve the current SMA crossover as the benchmark strategy.
   Treat `sma-crossover-v1` as the control arm for future experiments. Do not
   replace it until there is an experiment record showing improvement after
   costs, slippage, and risk limits.

5. Draft an experiment-record template.
   Each experiment should record hypothesis, market, universe, timeframe,
   features, parameters, training window, validation window, holdout window,
   costs, slippage assumptions, risk rules, results, failure modes, and
   promotion decision.

6. Add TA evidence rules to advisor and universe operating guidance.
   Before code changes, decide that advisor/universe prompts should avoid
   unsupported pattern claims. For example, an advisor should reference
   deterministic feature names or directly observed quote/recent-price evidence,
   not free-form chart pattern assertions.

7. Define paper-trading promotion thresholds.
   Candidate thresholds should include minimum sample size, max drawdown,
   turnover, live paper drift, risk-block rate, data-quality pass rate, and
   no unresolved journal/reconciliation issues during the observation period.

### Mid Term: Feature Layer and Research Harness

These items likely require code, migrations, and tests, so they should be
handled as separate implementation tasks later.

1. Add a deterministic market feature layer in `trade-engine`.
   The natural ownership is `trade-engine`, not `kis-client`. A future module
   could compute features from typed bar series and store them in append-only
   audit tables.

2. Add OHLCV and historical bar ingestion.
   Start with daily bars. Intraday bars should wait until session handling,
   provider limits, and storage volume are understood. Store source and
   adjustment policy with every bar.

3. Add first-pass feature outputs.
   Suggested first pack: SMA/EMA slope and crossover, ATR, RSI, MACD/ROC,
   volume average/volume breakout, relative strength versus benchmark, and
   simple pivot/support/resistance zones.

4. Add strategy experiments around current runtime boundaries.
   Experiment candidates:
   - SMA crossover baseline with current defaults.
   - SMA crossover plus higher-timeframe trend filter.
   - RSI pullback only when the primary trend is up.
   - MACD/ROC confirmation as a filter, not a standalone trigger.
   - Breakout/retest with volume confirmation.
   - ATR-based stop distance and position sizing.
   - Relative-strength universe selection versus static watchlist.

5. Add a backtest sidecar rather than embedding research logic in runtime.
   The backtester should consume stored bars/features and emit immutable
   experiment records. It should not call broker write APIs and should not share
   live execution code paths except typed domain models where safe.

6. Add walk-forward validation and holdout discipline.
   Use chronological splits. Avoid random train/test splits for time series.
   Record all parameter sweeps, including failed ones, to reduce data-snooping
   bias.

7. Extend risk with volatility and structure.
   Use ATR/volatility for position sizing and stop distance. Add structural
   invalidation levels from support/resistance or recent pivots as risk inputs.
   Keep max notional, buying power, duplicate-intent, panic stop, and journal
   gates authoritative.

8. Improve operator review surfaces.
   Add future read-only reports for feature snapshots, strategy experiments,
   paper/live drift, risk-block reasons, and promotion status before adding any
   live-write automation based on new TA signals.

### Long Term: Market Context and Advanced Topics

These are useful but should not distract from data quality, feature audit, and
validation.

1. Multi-timeframe context engine.
   Add primary/secondary/minor trend context across weekly, daily, and intraday
   views. The advisor should see this as structured context rather than infer it
   from raw recent prices.

2. Sector rotation and benchmark-relative strength.
   For domestic stocks, map symbols to sector/industry and compare against KRX
   benchmarks or sector ETFs/proxies if available. For overseas stocks, compare
   against SPY/QQQ/sector ETFs only when the configured market scope and data
   source allow it.

3. Intermarket context.
   Add rates, currency, commodity, volatility, or futures relationships only
   after data provenance and scope are clear. Murphy's intermarket theme is
   relevant, but this repo does not yet have the data contracts to support it
   safely.

4. Market Profile, point and figure, candlestick groups, cycles, and Elliott
   Wave.
   Treat these as advanced research features. They are parameter-sensitive and
   often subjective. They should remain advisory-only until deterministic rules,
   sample size, and validation evidence exist.

5. Certification-gated live influence.
   The final long-term goal is not "let TA trade live". It is a promotion gate
   where a strategy version can influence live proposals only after reproducible
   backtest, walk-forward, paper-trading, risk, and operator approval records
   exist in the product database.

## Suggested Experiments

### Experiment 1: Current SMA Baseline

Hypothesis: The existing `sma-crossover-v1` gives a useful benchmark for
comparison even if it is not production-worthy.

Required data: close prices by symbol, daily first; intraday later.

Metrics:

- Net return after fees/slippage.
- Max drawdown.
- Trade count and turnover.
- Win rate, average win/loss, profit factor, expectancy.
- Time in market.
- Risk-block rate in dry-run/paper.
- Parameter stability across short/long windows.

### Experiment 2: Primary Trend Filter

Hypothesis: SMA crossover performs better when long entries are allowed only in
a higher-timeframe uptrend and exits are more aggressive when that context
breaks.

Required data: daily and weekly bars or a deterministic weekly aggregation.

Metrics:

- Change in drawdown versus SMA baseline.
- Missed opportunity cost.
- Reduction in losing trades.
- Regime-specific performance: uptrend, downtrend, sideways.

### Experiment 3: RSI Pullback in Uptrend

Hypothesis: Oscillator signals are more useful as pullback timing inside a
confirmed trend than as standalone reversal predictions.

Required data: close prices and trend regime.

Metrics:

- Entry quality measured by maximum adverse excursion.
- Exit quality measured by maximum favorable excursion capture.
- False entry rate in sideways/downtrend regimes.
- Sensitivity to RSI thresholds and lookback windows.

### Experiment 4: Breakout With Volume Confirmation

Hypothesis: Breakouts above prior resistance have better expectancy when volume
or trading value confirms the move.

Required data: high/low/close, volume or trading value, pivot/resistance
feature outputs.

Metrics:

- False breakout rate.
- Retest success rate.
- Average return to measured target.
- Stop-out rate after breakout.
- Volume-data availability and quality pass rate.

### Experiment 5: ATR Sizing and Stops

Hypothesis: Volatility-adjusted sizing and stops reduce tail losses versus
fixed percentage stop/take-profit defaults.

Required data: high/low/close.

Metrics:

- Max drawdown and tail loss.
- Average risk per trade.
- Stop distance distribution.
- Position concentration under volatility spikes.
- Slippage sensitivity around stop exits.

### Experiment 6: Relative Strength Universe Selection

Hypothesis: Universe discovery improves when candidates are ranked by
deterministic relative strength against a benchmark and sector peers, rather
than primarily by provider-supplied narrative evidence.

Required data: candidate symbols, benchmark series, sector/peer mapping where
available.

Metrics:

- Candidate hit rate after 1, 5, 20 trading days.
- Relative performance versus benchmark.
- Candidate turnover.
- Data-unavailable and stale-data rates.
- Advisor proposal quality when deterministic relative-strength evidence is
  present versus absent.

## Backtest and Validation Requirements

Backtests should be treated as decision-support artifacts, not proof. Minimum
requirements:

- Use chronological splits: in-sample, validation, walk-forward windows, and a
  final untouched holdout.
- Preserve every tested parameter set and failed experiment to reduce selection
  bias.
- Include explicit fees, tax assumptions if relevant, slippage, spread, market
  impact proxy, partial-fill assumptions, and market-hours constraints.
- Model unsupported order behavior conservatively. If a live broker path cannot
  execute a hypothetical backtest order, the backtest should not count it as
  executable.
- Avoid lookahead: indicators must only use bars and metadata available at the
  decision timestamp.
- Avoid survivorship bias: universe membership should be as-of-time, not based
  on today's known winners.
- Include data-quality metrics: missing bars, stale quotes, corporate-action
  gaps, odd lots or thin liquidity, exchange/session mismatches, and provider
  error rates.
- Compare against simple baselines: buy-and-hold benchmark, hold-only, current
  SMA crossover, and random-entry-with-same-holding-period where practical.
- Report risk and operations metrics, not just returns: drawdown, exposure,
  order count, turnover, risk-block reasons, rejected decisions, unresolved
  journal events, reconciliation needs, and runtime/provider failures.
- Require paper-trading observation before live influence. Paper results should
  compare expected orders to observed quotes/fills and record drift.

## Metrics to Add to Future Reports

Strategy performance:

- Net return, annualized return where horizon supports it, and benchmark excess
  return.
- Sharpe and Sortino, with sample-size caveats.
- Max drawdown, average drawdown, recovery time.
- Profit factor, expectancy, average win/loss, win rate.
- Exposure time, turnover, trade count, average holding period.
- Parameter stability and degradation from in-sample to out-of-sample.

Signal quality:

- Signal frequency by regime.
- False breakout rate.
- Divergence success/failure rate.
- Trend-filter pass/fail counts.
- Indicator disagreement rate.
- Feature freshness and missing-data rate.

Risk and execution:

- Risk-block rate by reason.
- Orders avoided due to read-only, market closed, panic stop, unresolved
  journal, buying power, duplicate intent, and max exposure.
- Slippage versus decision reference price.
- Partial-fill and cancel/amend outcomes.
- Reconciliation-needed rate.
- Paper-to-live drift before any scaling decision.

Advisor/universe:

- Proposal acceptance/rejection rate.
- Invalid JSON/schema failure rate.
- Evidence coverage: percentage of proposals referencing deterministic feature
  IDs or directly observed KIS data.
- Candidate hit rate and retention.
- Token usage/cost per accepted candidate or proposal.

## Risks, Anti-Patterns, and Validation Cautions

- Indicator pile-up: Adding RSI, MACD, Bollinger Bands, candlesticks, and
  patterns at once can make explanations look stronger while increasing
  overfitting. Add one feature family at a time.
- Data snooping: Public research on technical rules warns that testing many
  rules and selecting winners can overstate performance. Record the full rule
  universe and failed sweeps.
- Multiple-signal overfitting: Combining many weak signals can create impressive
  in-sample results even when the signals have no real predictive power.
- Lookahead and timestamp leakage: Bar aggregation, corporate actions,
  benchmark joins, and universe membership must be as-of-time.
- Survivorship bias: Today's available symbols and sector memberships may not
  represent the historical tradable universe.
- Pattern subjectivity: Reversal, continuation, candlestick, Elliott Wave,
  point-and-figure, and cycle interpretations should stay advisory until rules
  are deterministic and tested.
- Oscillator misuse: Oscillators can behave differently in trends versus ranges.
  Use regime context before treating overbought/oversold as actionable.
- Volume limitations: KIS quote volume availability, intraday session coverage,
  off-hours behavior, and overseas data quality need verification before volume
  confirmation is trusted.
- Corporate-action and adjustment risk: Splits/dividends can invalidate raw
  price indicators unless adjustment policy is explicit.
- Intermarket drift: Correlations change. Intermarket rules should be monitored
  for regime changes and should not be hardcoded as permanent truths.
- LLM overreach: Advisor and universe providers may produce plausible but
  unsupported TA language. Require deterministic evidence or direct observed
  data references.
- Live-safety erosion: No TA signal should bypass read-only mode, live enablement,
  account confirmation, single-writer lock, durable journal, unresolved command
  checks, panic stop, final risk, market session, or reconciliation.
- Regulatory/disclosure risk: If outputs are shown to users as analysis tools,
  document methodology, limitations, assumptions, universe selection, and that
  hypothetical projections are not guarantees.

## Concrete Next Steps Before Coding

1. Decide the first research market and timeframe.
   Recommended default: one market scope only, daily bars first. Pick either
   KRX domestic stocks or US overseas stocks, not both, for the first research
   pass.

2. Create a TA feature glossary issue or follow-up plan.
   Include exact formulas, lookbacks, required fields, and data-quality flags
   for the first feature pack: SMA/EMA, ATR, RSI, MACD/ROC, volume breakout,
   relative strength, and simple support/resistance pivots.

3. Verify official historical data availability.
   Before implementation, verify current official KIS documentation or approved
   provider documentation for historical bars, volume, adjusted prices, and
   rate limits. Record source URLs and access dates.

4. Define an experiment record template.
   The template should be ready before building the backtest sidecar, so every
   experiment can be reproduced and audited.

5. Define promotion criteria.
   Agree on the minimum evidence required for a feature to move from display,
   to dry-run proposals, to paper observation, to certified live influence.

6. Review advisor and universe prompts for TA evidence discipline.
   The current scripts already emphasize observed KIS data and proposals that
   pass deterministic gates. The next prompt iteration should add a rule that
   pattern/indicator claims must cite deterministic feature outputs once those
   exist.

7. Decide what remains out of scope.
   Recommended out of scope for now: Elliott Wave, cycles, Market Profile,
   point-and-figure, continuous futures, and broad intermarket rules. These need
   more data and stronger validation than the near-term system has.

## Recommended Implementation Order When Coding Begins

1. Historical bar contract and data-source verification.
2. Append-only bar storage and data-quality audit.
3. Deterministic feature computation for one daily-bar feature pack.
4. Feature audit/reporting in dry-run output or operator reports.
5. Backtest sidecar consuming stored bars/features.
6. Walk-forward experiment records and baseline comparisons.
7. Risk integration for volatility/structure-based sizing and invalidation.
8. Advisor/universe schema or prompt updates to reference feature evidence.
9. Paper-trading certification records.
10. Only then consider live proposal influence for a certified strategy version.

## Summary Recommendations

- Keep technical analysis in `trade-engine` as deterministic, audited market
  features and experiments.
- Do not add strategy or TA responsibilities to `kis-client`.
- Build OHLCV/bar and data-quality foundations before adding indicator variety.
- Use the existing SMA crossover as the baseline, not as the destination.
- Prioritize trend hierarchy, chart construction/data quality, risk, and system
  validation before subjective chart patterns.
- Treat advisors as evidence summarizers and proposal generators, never as
  feature calculators or execution authorities.
- Require walk-forward validation, holdout discipline, paper observation, and
  certification before any TA-derived strategy can influence live submit mode.
