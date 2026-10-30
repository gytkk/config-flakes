# Trading Agent Plan V2

## Status

이 문서는 `plans/INITIAL_PLAN.md`를 기반으로, 아래 두 KIS 공식 GitHub 저장소와
현재 저장소에 이미 구현된 Rust `kis-client`를 반영한 제품 계획 V2다.

- 기준일: 2026-04-25
- 참고 저장소:
  - https://github.com/koreainvestment/open-trading-api
    - 확인 commit: `33e0e1e65cd1c8c8b639531483ec0b327087bab1`
  - https://github.com/koreainvestment/kis-ai-extensions
    - 확인 commit: `2f850a76e317631983edef56db96aa8b45b8eba3`
- 이 문서는 새 제품 계획안이다. `plans/INITIAL_PLAN.md`는 원본 기록으로 유지한다.

중요한 전제:

- `open-trading-api`는 공식 샘플과 도구의 좋은 기준점이지만, 샘플 코드는 계속
  바뀔 수 있고 사용 책임은 사용자에게 있다는 고지가 있다.
- `kis-ai-extensions`는 AI coding agent가 전략 설계, 백테스트, 주문 실행을
  자연어로 조작하기 위한 플러그인이다. 이 저장소의 안전 모델을 그대로 대체하지
  않고, agent workflow와 보안 훅 설계 참고 자료로 쓴다.
- 현재 이 저장소에는 이미 `kis-client` Rust crate가 있다. KIS 주문성 broker
  write path는 Python 샘플로 다시 만들지 않고, 기존 Rust 구현을 제품의
  broker adapter foundation으로 유지한다.

---

## V2 Direction

V1의 핵심 원칙은 유지한다.

- agent는 제안만 한다.
- 리스크와 정책 계층이 최종 권한을 가진다.
- 주문 실행과 기록은 결정론적 코드가 담당한다.
- broker response, fills, decisions, risk blocks, reconciliation 결과는
  감사 가능해야 한다.

V2에서 바뀌는 점은 구현 전략이다.

1. **Rust `kis-client`를 KIS broker adapter로 고정**
   - 이미 구현된 auth, transport, domain model, command journal, guardrail,
     reconciliation, fixture tests를 버리지 않는다.
   - Python 앱이 KIS REST write endpoint를 직접 재구현하지 않는다.
   - KIS endpoint 추가는 계속 Rust `kis-client`에서 contract test first로
     진행한다.

2. **trading-agent product layer도 Rust로 작성**
   - strategy spec, backtest orchestration, agent decision, risk policy,
     storage, reporting, Discord, operator CLI를 Rust workspace crate로 만든다.
   - KIS 주문 실행은 같은 workspace의 `trading-agent-kis-client` library
     contract를 통해 호출한다.
   - Python은 KIS 공식 샘플, external backtester, MCP sidecar를 참고하거나
     호출할 때만 경계 밖 도구로 사용한다.

3. **`.kis.yaml`을 first-class strategy artifact로 채택**
   - `open-trading-api`의 Strategy Builder와 Backtester가 공유하는 포맷이다.
   - 이 프로젝트에서는 그대로 복붙하지 않고 `StrategySpec` schema로 정규화한다.
   - agent는 즉시 주문을 내는 대신 strategy spec 또는 bounded order intent를
     제안할 수 있다.

4. **Backtest -> Paper -> Limited Live promotion gate**
   - 전략은 백테스트 결과, paper 결과, risk approval을 통과해야 live 후보가 된다.
   - 단순히 agent confidence가 높다는 이유로 live 주문을 허용하지 않는다.

5. **MCP와 AI extension은 research surface로 제한**
   - MCP는 전략 탐색, 백테스트, API 탐색에는 유용하다.
   - live 주문 권한은 이 프로젝트의 risk/execution path 밖으로 노출하지 않는다.

---

## What We Learned From KIS Repositories

### `open-trading-api`

이 저장소는 KIS Open API를 LLM과 Python 개발자가 쉽게 탐색하도록 구성된 공식
샘플 코드 및 도구 모음이다.

활용할 부분:

- `examples_llm/`
  - API별 단일 기능 샘플과 `chk_*` 검증 스크립트가 있다.
  - 새로운 KIS endpoint를 `kis-client`에 추가할 때 request path, header,
    `tr_id`, payload, pagination, response fixture를 비교하는 참고 자료로 쓴다.

- `examples_user/`
  - 상품군별 통합 함수 예제가 있다.
  - domestic stock 외 상품을 나중에 확장할 때 adapter scope를 잡는 데 쓴다.

- 지원 API 카테고리
  - auth, domestic stock, domestic bond, domestic future/option,
    overseas stock, overseas future/option, ELW, ETF/ETN.
  - V2 제품 범위는 여전히 국내주식 우선이다. 다른 상품군은 architecture가
    확장 가능하다는 증거로만 반영한다.

- `stocks_info/`
  - 종목 마스터, 업종, 테마, 해외 종목 코드 등 universe metadata가 있다.
  - 이 프로젝트의 universe loader와 symbol search seed source로 활용 가능하다.

- `strategy_builder/`
  - FastAPI + Next.js 기반 비주얼 전략 빌더다.
  - 10개 프리셋, 80개 이상 기술지표, 캔들 패턴, YAML/Python preview,
    BUY/SELL/HOLD signal, 모의/실전 주문 실행 화면을 제공한다.
  - 이 프로젝트는 UI를 바로 가져오기보다 `.kis.yaml` schema, signal contract,
    strategy registry 개념을 가져온다.

- `backtester/`
  - QuantConnect Lean Docker 기반 백테스트 엔진이다.
  - `.kis.yaml` import, preset/custom strategy, Grid/Random optimization,
    HTML report, metrics(total return, CAGR, Sharpe, MDD, win rate 등)를 제공한다.
  - V2에서는 backtester를 sidecar로 호출하거나, 필요한 schema와 result contract를
    내부 adapter로 감싼다.

- `MCP/`
  - KIS Trading MCP는 다수 API를 MCP로 노출한다.
  - API 탐색과 read-only research에는 가치가 있지만, dynamic code execution과
    live write exposure는 제품 실행 경로와 분리해야 한다.

### `kis-ai-extensions`

이 저장소는 npm package `@koreainvestment/kis-quant-plugin`으로 배포되는
AI coding agent 확장 모음이다.

활용할 부분:

- agent별 self-contained 설치 구조
  - Claude Code, Cursor, Codex, Gemini CLI에 scripts, skills, commands, hooks,
    MCP 설정을 설치한다.
  - 이 저장소에서는 repo-managed skill이나 command를 만들 때 참고하되,
    제품 코드와 operator safety를 agent plugin에 의존시키지 않는다.

- skill workflow
  - `kis-strategy-builder`: 전략 설계와 `.kis.yaml` 생성.
  - `kis-backtester`: 백테스트, 최적화, HTML report.
  - `kis-order-executor`: signal 확인 후 주문 실행.
  - `kis-team`: 설계 -> 검증 -> 실행을 단계별 확인으로 묶는다.
  - 이 프로젝트에서는 이 흐름을 제품 workflow로 흡수하되, live 주문 단계는
    반드시 내부 risk/execution engine을 통과하게 한다.

- security hooks
  - secret guard, prod guard, trade log, MCP log 패턴은 유용하다.
  - 그러나 shell hook은 보조 안전장치일 뿐이다. 최종 안전은 storage,
    command journal, risk gate, single-writer lock, reconciliation이 책임진다.

- order execution safety hints
  - signal strength `< 0.5`이면 주문을 건너뛰는 규칙.
  - 실전 주문 전 종목, 수량, 예상금액, 모드 확인.
  - V2에서는 이 규칙을 UI/agent hint가 아니라 configurable policy로 격상한다.

---

## Current Repository Baseline

현재 구현된 코드는 `kis-client` Rust crate와 초기 `trading-agent` Rust product
crate다.

이미 있는 것:

- Cargo workspace root.
- `trading-agent` product crate.
- product-level trading gateway trait.
- deterministic `MockGateway` for runtime tests.
- `KisGateway` adapter that delegates to the existing `kis-client`
  `BrokerClient` trait.
- `trading-agent-kis-client` crate.
- broker trait와 typed domain model.
- `BrokerError` taxonomy.
- KIS auth contract, token provider, token cache, secret redaction.
- KIS transport foundation, redacted audit metadata, retry classification,
  request id, timeout/base URL validation.
- 국내주식 quote, balance, cash/buying power request/response mapping.
- 국내주식 cash order, amend, cancel contracts.
- daily order inquiry, fill mapping.
- durable command journal contract and agent-owned SQLite storage
  implementation.
- duplicate intent, state transition, parent/child integrity tests.
- read-only, live enablement, account allowlist, account suffix confirmation,
  panic stop, KRX/KIS session checks.
- file-backed single-writer guardrail.
- ambiguous write outcome and reconciliation workflow.
- pending reconciliation sweep and live startup halt.
- regression tests and fixtures.

아직 없는 것:

- product-level Rust modules beyond the initial broker layer.
- full run storage for market snapshots, agent decisions, risk checks,
  signals, strategy specs, backtests, reports.
- strategy spec parser/validator.
- backtest sidecar integration.
- agent decision layer.
- risk/policy engine above broker.
- execution engine that consumes agent/risk output and calls `kis-client`.
- Discord reporting.
- paper trading loop and production runbook.

V2 원칙:

- `kis-client`는 KIS broker adapter의 source of truth다.
- product runtime은 `kis-client`의 public contract를 호출한다.
- `kis-client` 내부에 strategy, agent, portfolio policy, scheduling, reporting을
  넣지 않는다.

---

## Product Architecture V2

```text
                  research / design
        ┌──────────────────────────────────┐
        │ StrategySpec(.kis.yaml) Builder  │
        │ Backtest Adapter / Reports       │
        │ Agent Proposal Generator         │
        └───────────────┬──────────────────┘
                        │ structured proposals only
                        ▼
┌─────────────┐   ┌──────────────┐   ┌────────────────┐
│ Market Data │──▶│ Strategy Run │──▶│ Agent Decision │
│ + Features  │   │ + Signals    │   │ Layer          │
└──────┬──────┘   └──────┬───────┘   └──────┬─────────┘
       │                 │                  │
       └─────────────────┴──────────────────▼
                                      ┌──────────────┐
                                      │ Risk/Policy  │
                                      │ Final Gate   │
                                      └──────┬───────┘
                                             │ approved commands
                                             ▼
                                      ┌──────────────┐
                                      │ Execution    │
                                      │ Engine       │
                                      └──────┬───────┘
                                             │ broker command
                                             ▼
                                      ┌──────────────┐
                                      │ Rust         │
                                      │ kis-client   │
                                      └──────┬───────┘
                                             │ KIS REST
                                             ▼
                                      ┌──────────────┐
                                      │ KIS Open API │
                                      └──────────────┘

All arrows that affect trading state write audit records to SQLite.
```

### 1. Broker Adapter Layer

Owner: Rust `kis-client`.

Responsibilities:

- KIS auth, headers, hashkey where required, transport, retry classification.
- quote, account, buying power, order, amend/cancel, order inquiry, fills.
- command journal and single-writer safety for broker writes.
- ambiguous outcome classification and reconciliation.
- library APIs for health, safety, and reconciliation diagnostics consumed by
  product operator surfaces.

Non-responsibilities:

- agent prompt.
- strategy selection.
- portfolio policy.
- Discord reporting.
- scheduling.
- backtest analytics.

Product integration options:

1. **Phase 1: in-process Rust adapter**
   - `trading-agent` crate depends on `trading-agent-kis-client`.
   - Product runtime sees a product-level trading gateway trait.
   - KIS-specific implementation is wrapped behind an adapter that delegates to
     the existing `BrokerClient` trait.
   - Tests use a deterministic mock broker and a recording KIS trait adapter.

2. **Phase 2: service or binary gateway only if needed**
   - A local service or machine-readable CLI gateway can be added if process
     isolation, operational separation, or independent deployment becomes useful.
   - Do not introduce a service before the in-process trait boundary is proven
     insufficient.

### 2. Strategy Spec Layer

Owner: Rust `trading-agent` product crate.

Responsibilities:

- Load and validate `.kis.yaml`.
- Normalize KIS/open-trading-api style strategy definitions into internal
  `StrategySpec`.
- Keep strategy params, indicator aliases, entry/exit logic, and risk hints
  explicit.
- Store every strategy version by content hash.
- Allow agent to propose strategy edits, but require schema validation and
  user/policy approval before activation.

Rules:

- `.kis.yaml` is an input/output format, not the only internal model.
- Unknown fields fail closed unless explicitly accepted.
- Strategy risk hints are advisory. Product risk policy can override or block.
- Backtest and live signal engines must use the same normalized strategy spec.

Minimal internal contract:

```json
{
  "strategy_id": "rsi_reversion_v1",
  "version": "1.0",
  "universe": ["005930"],
  "indicators": [],
  "entry": {"logic": "AND", "conditions": []},
  "exit": {"logic": "OR", "conditions": []},
  "risk_hints": {
    "stop_loss_pct": "3.0",
    "take_profit_pct": "8.0",
    "max_position_pct": "10.0"
  }
}
```

### 3. Market Data / Universe Layer

Owner: Rust `trading-agent` product crate, broker reads via `kis-client` when
KIS data is needed.

Responsibilities:

- Universe definition and symbol metadata.
- KRX/KIS market session state.
- Current quotes, OHLCV, orderbook snapshots where needed.
- Feature snapshots for strategy and agent input.
- Rate-limit aware caching.

Inputs:

- `kis-client` read APIs.
- KIS sample `stocks_info/` metadata as a seed source.
- KIS official docs/sample behavior for endpoint contract verification.

Rules:

- Feature generation must be deterministic and replayable from stored snapshots.
- Raw market data and derived feature data should be stored separately.
- Backtest data quality warnings must be persisted with backtest results.

### 4. Backtest / Research Layer

Owner: Rust `trading-agent` product crate, optional sidecar integration with
`open-trading-api/backtester`.

Responsibilities:

- Validate `.kis.yaml` before backtest.
- Run preset/custom backtests.
- Store inputs, parameters, source data period, metrics, and report artifacts.
- Compare strategies and parameter sweeps.
- Produce promotion evidence for paper/live gates.

Initial approach:

- Treat `open-trading-api/backtester` as an external reference or sidecar.
- Define an internal `BacktestAdapter` interface first.
- Implement one adapter that can call a local backtester HTTP/MCP process when
  available.
- Keep result schema in our DB so historical evidence survives sidecar changes.

Required metrics:

- total return.
- CAGR.
- max drawdown.
- Sharpe ratio.
- Sortino ratio when available.
- win rate.
- profit factor.
- total trades.
- turnover estimate.
- data warnings.

Promotion gate examples:

- strategy has at least one stored backtest over configured lookback.
- max drawdown under configured threshold.
- minimum trade count met.
- no unresolved data quality warning.
- paper trading run passes minimum observation window.
- human/operator approval recorded for live candidate.

### 5. Agent Decision Layer

Owner: Rust `trading-agent` product crate.

The agent may produce:

- strategy proposal.
- strategy parameter adjustment.
- watchlist change proposal.
- bounded order intent.
- hold/no-op explanation.
- risk concern summary.

The agent may not:

- call KIS directly.
- read secrets.
- submit orders.
- bypass risk policy.
- mutate durable trading state without structured validation.

Structured output examples:

```json
{
  "decision_type": "order_intent",
  "run_id": "run_20260425_001",
  "strategy_id": "rsi_reversion_v1",
  "action": "buy",
  "symbol": "005930",
  "quantity": 10,
  "order_type": "limit",
  "limit_price": "73000",
  "confidence": "0.71",
  "time_horizon": "intraday",
  "reason": "RSI rebound with volume confirmation",
  "risk_notes": ["near earnings date"]
}
```

Rules:

- JSON schema validation is mandatory.
- Invalid output is stored and rejected.
- Free text reason is audit context only.
- Execution uses structured fields only.
- Agent confidence is never a substitute for risk approval.

### 6. Risk / Policy Engine

Owner: Rust `trading-agent` product crate.

Responsibilities:

- Validate strategy activation.
- Validate every order intent before execution.
- Enforce account, symbol, order size, exposure, loss, session, cooldown,
  duplicate-intent, and promotion policies.
- Block when broker journal has unresolved reconciliation.
- Explain every block with a durable reason.

Policy examples:

- signal strength minimum.
- max order notional.
- max position notional.
- max strategy allocation.
- daily realized loss limit.
- daily order count limit.
- repeated loss cooldown.
- no live order outside allowed session.
- no live order without successful startup doctor.
- no live order while panic stop is active.
- no live order when unresolved broker command exists.
- no live order for strategy without promotion approval.

Important split:

- Strategy-level risk hints come from `.kis.yaml`.
- Product risk policy is configured separately and has final authority.

### 7. Execution Engine

Owner: Rust `trading-agent` product crate for orchestration, Rust `kis-client`
for broker write safety.

Responsibilities:

- Convert approved risk result into broker command.
- Reserve product-level execution record before broker call.
- Call `kis-client` gateway.
- Persist broker response, command journal reference, fills, and final status.
- Never blind retry ambiguous broker writes.
- Trigger reconciliation when status is unknown.

Execution states:

- `proposed`
- `risk_blocked`
- `risk_approved`
- `broker_submitting`
- `broker_acknowledged`
- `partially_filled`
- `filled`
- `cancelled`
- `rejected`
- `reconcile_needed`
- `terminal_error`

Rules:

- `client_order_id` is generated before broker submit.
- The execution engine must pass idempotency metadata into `kis-client`.
- Product storage and `kis-client` journal ids must be cross-linked.
- Reconciliation is an explicit workflow, not a hidden retry loop.

### 8. Storage / Audit Layer

Owner: Rust `trading-agent` product crate plus existing `kis-client` journal.

Initial DB: SQLite.

Core product tables:

- `runs`
- `strategy_specs`
- `strategy_versions`
- `backtest_runs`
- `backtest_metrics`
- `market_snapshots`
- `feature_snapshots`
- `agent_inputs`
- `agent_outputs`
- `agent_decisions`
- `signals`
- `risk_evaluations`
- `execution_requests`
- `execution_events`
- `broker_command_links`
- `fills`
- `portfolio_snapshots`
- `daily_reports`
- `operator_actions`

Storage rules:

- Store raw agent output and parsed decision separately.
- Store raw broker response metadata without secrets.
- Store prompt input summaries sufficient for audit without leaking secrets.
- Store policy version/hash with each risk evaluation.
- Store strategy content hash with each signal/backtest/run.
- Do not make Discord or CLI a source of truth.

### 9. Operator CLI

Owner: Rust `trading-agent` product crate.

Product CLI commands:

- `trading-agent doctor`
- `trading-agent config validate`
- `trading-agent strategy validate <file>`
- `trading-agent strategy import <file>`
- `trading-agent backtest run <strategy>`
- `trading-agent backtest list`
- `trading-agent run-once`
- `trading-agent paper loop`
- `trading-agent decisions list`
- `trading-agent risk blocks`
- `trading-agent executions list`
- `trading-agent portfolio show`
- `trading-agent report daily`
- `trading-agent panic-stop`
- `trading-agent resume-live`

Broker-specific diagnostics should be exposed through product commands that call
`TradingGateway`/`KisGateway`. `kis-client` remains a library-only crate.

### 10. Discord Reporting

Owner: Rust `trading-agent` product crate.

Events:

- strategy imported/activated.
- backtest completed/failed.
- agent decision generated.
- risk blocked.
- order submitted.
- order acknowledged.
- partial/full fill.
- ambiguous outcome/reconciliation needed.
- panic stop activated.
- live resume.

Reports:

- pre-market plan.
- intraday status.
- market close summary.
- daily PnL summary.
- strategy performance summary.

Rules:

- Discord contains summaries and links/ids.
- DB remains source of truth.
- Secrets and full account numbers are never sent.

### 11. MCP / AI Tooling Boundary

MCP tools can be used for:

- backtest execution.
- strategy template lookup.
- KIS API discovery.
- read-only account/market exploration in non-live contexts.

MCP tools must not be used for:

- direct live order submission.
- bypassing product risk policy.
- storing or displaying secrets.
- hidden state changes outside the product DB.

If this repository later adds Codex skills, they should be repo-managed and
focused:

- strategy authoring helper.
- backtest analysis helper.
- operator diagnosis helper.

They should not become the execution authority.

---

## Repository Shape V2

Target structure:

```text
trading-agent/
  Cargo.toml
  README.md
  plans/
    INITIAL_PLAN.md
    PLAN_V2.md
    KIS_CLIENT_IMPLEMENTATION_PLAN.md
  configs/
    app.yaml
    risk.yaml
    universe.yaml
    schedules.yaml
    prompts/
      decision.md
  strategies/
    examples/
      rsi_reversion.kis.yaml
  agent/
    Cargo.toml
    src/
      lib.rs
      error.rs
      gateway/
        mod.rs
        traits.rs
        types.rs
        adapters/
          kis.rs
        testing/
          mock.rs
      domain/
        mod.rs
      strategy/
        mod.rs
      market/
        mod.rs
      research/
        mod.rs
      agent/
        mod.rs
      risk/
        mod.rs
      execution/
        mod.rs
      storage/
        mod.rs
      reporting/
        mod.rs
      runtime/
        mod.rs
      cli/
        mod.rs

  kis-client/
    src/
    migrations/
    tests/
```

Notes:

- `kis-client` remains a workspace member.
- `agent` is the Rust product crate and should remain importable and modular,
  not centered on a web server.
- A web dashboard can be added later after CLI/storage contracts are stable.
- Do not vendor the full `open-trading-api` repo into this repository at the
  start. Add adapters or fixtures deliberately.

---

## Implementation Sequence

### Phase 0: Planning and Baseline

Goal:

- Adopt `PLAN_V2.md`.
- Keep `kis-client` as broker adapter foundation.
- Document current implemented scope.

Acceptance checks:

- `plans/PLAN_V2.md` exists.
- README points to V2 after approval.
- Existing `kis-client` tests still pass when Rust code changes are made.

### Phase 1: Product Rust Skeleton

Goal:

- Create importable Rust product crate.
- Add product error type, trading gateway interface, KIS adapter, and
  deterministic mock.
- Add settings, logging, domain ids, storage migration framework, and CLI
  entrypoint as follow-up modules.

Acceptance checks:

- `cargo test -p trading-agent` passes.
- `trading-agent doctor` runs without KIS credentials and reports missing optional
  dependencies gracefully.
- no secret values are logged.

### Phase 2: Trading Gateway Contract

Goal:

- Define product-facing trading gateway interface.
- Wrap the existing `trading_agent_kis_client::BrokerClient` trait behind the
  product gateway trait.
- Keep operator output in the product CLI layer. It should consume typed
  `TradingGateway`/`KisGateway` results rather than parsing KIS-specific command
  output.

Acceptance checks:

- fake gateway tests cover wrapper behavior.
- `KisGateway` maps `BrokerError` failures to specific product errors.
- `kis-client` exposes no CLI output for the product runtime to parse.

### Phase 3: Strategy Spec and Signal Engine

Goal:

- Load `.kis.yaml`.
- Validate and normalize to internal `StrategySpec`.
- Store strategy versions by hash.
- Generate deterministic signals from stored market snapshots for a small
  initial indicator set.

Acceptance checks:

- fixtures based on KIS sample YAML validate.
- invalid YAML fails with actionable errors.
- signal generation is replayable from stored snapshots.

### Phase 4: Backtest Adapter

Goal:

- Add `BacktestAdapter` contract.
- Support local sidecar call to `open-trading-api/backtester` or a compatible
  process.
- Store result metrics and report references.

Acceptance checks:

- completed, failed, timeout, and data-warning cases are tested.
- backtest result can be linked to a strategy version.
- promotion gate can query stored metrics without calling the sidecar.

### Phase 5: Agent and Risk Gate

Goal:

- Add structured agent decision schema.
- Add risk engine with configurable policies.
- Persist decisions and risk results.

Acceptance checks:

- malformed agent output is rejected and stored.
- risk blocks include durable reasons.
- agent cannot create an executable command without risk approval.

### Phase 6: Paper Run Loop

Goal:

- Implement `run_once` and paper loop.
- Use strategy signals, agent decision, risk gate, and broker gateway.
- Produce CLI and Discord reports.

Acceptance checks:

- dry-run mode submits no broker writes.
- paper mode records all decisions and broker outcomes.
- ambiguous broker outcomes enter reconciliation path.

### Phase 7: Limited Live Readiness

Goal:

- Enable live-read-only diagnostics.
- Then allow tightly limited live write after promotion, operator approval, and
  all guardrails.

Acceptance checks:

- live write blocked by default.
- live write requires explicit enablement, account suffix confirmation,
  allowlist, single-writer lock, clean reconciliation state, open session, and
  approved strategy.
- startup doctor blocks live write on unresolved command journal records.

---

## Safety Invariants

These invariants apply across the full product.

- Agent output is never an order by itself.
- Live order write is disabled by default.
- Product risk policy has final authority over strategy hints and agent output.
- Broker command must be durable before live broker write.
- Same `(environment, account)` must have one writer.
- Unresolved journal records block new live writes.
- Ambiguous broker write outcome must not be blindly retried.
- Secrets must not appear in logs, fixtures, debug output, Discord, or reports.
- Account numbers must be redacted except for approved suffix display.
- Backtest success alone does not authorize live trading.
- MCP/AI tools cannot bypass product execution path.
- Operator panic stop blocks writes until explicitly cleared.

---

## KIS Endpoint Workflow

For every new KIS REST endpoint added to `kis-client`:

1. Verify current official KIS documentation or official sample behavior.
2. Compare relevant `open-trading-api/examples_llm` and `examples_user` sample
   code when available.
3. Add executable contract tests for method, path, headers, `tr_id`, request
   shape, response mapping, and error mapping.
4. Add pagination tests when `tr_cont` or `CTX_AREA_*` tokens are involved.
5. Add rate-limit and retry classification tests where relevant.
6. Add journal, safety, ambiguous-outcome, and reconciliation tests before any
   order write endpoint.
7. Implement only after tests fail for the expected reason.

Sample code can guide test fixtures, but the `kis-client` contract owns the
product behavior.

---

## Explicit Non-Goals For V2

- Replace the Rust `kis-client` with Python KIS sample wrappers.
- Build a web dashboard before CLI/storage/runtime contracts are stable.
- Expose live order submission through arbitrary MCP tools.
- Support every KIS product category in the first product version.
- Support high-frequency trading.
- Support active-active multi-host live writers.
- Treat `kis-ai-extensions` hooks as sufficient trading safety.
- Let Discord become a state store.

---

## Product Definition Of Done

The product is not live-ready until all of the following are true:

- `kis-client` broker write path has passing safety/reconciliation tests.
- Rust product runtime has persistent storage for runs, decisions, risk,
  execution, fills, portfolio snapshots, and reports.
- Strategy specs are versioned and linked to every signal/order.
- Backtest evidence is stored and queryable.
- Paper loop has run successfully over the configured observation window.
- Risk policy blocks are visible in CLI and Discord.
- Startup doctor validates config, DB, panic stop, unresolved broker commands,
  market session, and broker connectivity.
- Live write path requires explicit operator action and account confirmation.
- A documented reconciliation runbook exists.
- A documented rollback/panic-stop runbook exists.
