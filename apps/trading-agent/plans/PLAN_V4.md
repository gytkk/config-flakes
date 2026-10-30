# 트레이딩 에이전트 계획 V4

## 상태

이 문서는 `plans/PLAN_V3.md`와
`plans/TRADE_ENGINE_REVIEW_2026-04-26.md`를 바탕으로, 현재 구현 상태와
후속 설계 결정을 반영한 V4 계획이다.

- 기준일: 2026-04-26
- 기준 코드 상태:
  - `trade-engine` crate가 workspace member로 존재한다.
  - `kis-client`는 library-only KIS broker adapter로 유지된다.
  - Main DB, migrations, DB-backed broker command journal이 존재한다.
  - `trade-engine doctor`와 `run --dry-run` CLI가 존재한다.
  - Dry-run runtime loop는 하나의 `runtime_runs` 아래 여러 `runtime_ticks`를
    기록한다.
  - Tick advisory는 각 market tick 안에서 Codex CLI 등 외부 command provider를
    advisory-only로 호출하고, 결과를 `operator_actions`에 감사 기록으로 남긴다.
  - Snapshot 수집 실패는 실패 tick으로 감사된다.
  - 상태 문자열 일부는 Rust enum의 `as_str`/`parse` 경계로 정리되어 있다.
  - SQLite는 async runtime에서 직접 blocking하지 않도록 `BlockingMainDb`
    boundary를 사용하기 시작했다.

V4는 V3를 대체한다. V3와 충돌하면 V4가 우선한다.

V4에서 유지하는 핵심 원칙:

- LLM은 주문을 실행하지 않는다. LLM은 structured decision proposal만 만든다.
- 주문 실행, 리스크 평가, journal, reconciliation, 감사 기록은 결정론적인 Rust
  코드가 담당한다.
- Risk/policy 계층이 최종 권한을 가진다.
- Broker write 전에 execution request와 broker command journal record가 durable해야
  한다.
- Ambiguous broker outcome 이후에는 blind retry하지 않는다.
- Main DB는 제품 runtime의 system of record다.

---

## V4에서 바뀐 결정

V4는 V3와 리뷰 문서의 일부 결정을 다음처럼 갱신한다.

### DB는 필수다

V3의 Read-only MVP에는 "DB 없이 실행 가능" 기준이 있었다. V4에서는 폐기한다.

V4 기준:

- `doctor`와 `run --dry-run`은 `storage.main_db_path`를 필수로 요구한다.
- Read-only smoke도 DB-backed audit을 남긴다.
- DB 없는 smoke mode는 V4 MVP 범위가 아니다.
- Main DB는 runtime, decision, risk, execution, reconciliation, certification의
  system of record다.

### KIS 환경은 config로 명시한다

현재 구현은 live read-only dry-run에 가깝지만, V4에서는 환경을 설정으로 명시한다.

필수 방향:

- `kis.environment = "paper" | "live"`
- `read_only = true`인 live 환경은 balance/quote 조회를 허용한다.
- Live write는 `read_only=false`만으로 허용되지 않는다. 별도 live enable flag,
  account guard, unresolved journal check, panic stop, certification gate가 모두
  필요하다.
- DB의 `runtime_runs.environment`와 journal의 environment는 실제 설정 환경과
  일치해야 한다.

### Account key는 로컬 runtime 데이터로 취급한다

V3는 전체 계좌번호 redaction을 강하게 요구했다. V4에서는 로컬 시스템 운영 관점에
맞춰 조정한다.

V4 기준:

- full `AccountKey`는 로컬 Main DB와 broker command journal에 저장할 수 있다.
- CLI/stdout/log/report에도 full account key 표시를 금지하지 않는다.
- 단, 실제 계좌번호를 공개 문서, test fixture, commit에 넣으면 안 된다.
- `KIS_APP_SECRET`, access token, hashkey, token cache 내용은 DB, log, CLI,
  report, fixture에 저장하지 않는다.
- External LLM decision prompt에는 full account key를 보내지 않는다.

### Strategy는 Rust 코드와 TOML parameter로 시작한다

V3의 `.kis.yaml` first-class strategy artifact 방향은 V4 MVP에서 제외한다.

V4 기준:

- Strategy는 Rust 내장 구현으로 둔다.
- 우선 지원 전략은 `hold-only`와 `sma-crossover-v1`이다.
- Strategy 선택과 parameter는 TOML config에 둔다.
- `strategy validate/import` CLI와 YAML parser는 V4 MVP 범위가 아니다.
- 외부 strategy artifact는 나중에 필요해지면 재검토한다.
- 저장된 OHLCV bar를 읽는 `sma-crossover-v1` baseline backtest sidecar는
  허용하되, broker/runtime write 경로와 분리하고 결과 저장/promotion harness는
  별도 단계로 둔다.

### Proposal provider boundary는 advisor command로 둔다

LLM proposal 생성은 V4 MVP에 포함한다. 다만 `trade-engine`이 OpenAI HTTP API나
SDK에 직접 결합하지 않는다.

V4 기준:

- Runtime proposal source는 하나로 유지한다. 외부 command boundary는 `[advisor]`
  provider가 담당한다.
- `[decision] provider = "command"`는 runtime에 연결되지 않은 legacy surface라서
  deprecated 처리하고 config validation에서 거부한다.
- Deterministic path는 내장 strategy signal을 advisor-style proposal로 변환하는
  fallback으로 유지한다.
- OpenAI 호출은 별도 sidecar/script가 담당할 수 있다.
- `trade-engine`은 advisor command의 입출력 schema, timeout, failure handling,
  DB audit만 소유한다.
- Tick advisory도 같은 원칙을 따른다. Codex CLI/MCP 같은 agent tooling은 외부
  command boundary 뒤에 두고, output은 operator advisory audit일 뿐 decision,
  risk approval, execution request, broker write로 승격하지 않는다.
- Advisor input은 `runtime`과 현재 market tick의 KIS/gateway read 결과인
  `current_state.current_tick.kis_api`를 분리해 전달한다. `runtime`에는 시장/거래소/통화
  context와 universe discovery 후보/freshness metadata를 포함한다. Advisor output은
  operator가 읽는 요약 필드, audit용 `rationale`/`evidence`, 그리고 deterministic
  validation과 risk gate로 넘길 structured `proposals`를 함께 남긴다.

### Gateway trait는 단일 abstraction으로 유지한다

리뷰 문서에서는 read/write gateway trait 분리를 권장했다. V4에서는 단일
`TradingGateway`를 유지하고, 실행 workflow와 risk/journal/final check로 write
안전을 보장한다.

V4 기준:

- Gateway abstraction은 read/write method를 함께 가질 수 있다.
- `doctor`와 `run --dry-run`은 write method를 호출하지 않는다.
- Execution workflow만 write method를 호출한다.
- Read-only config에서는 risk policy와 broker guardrail이 write를 차단한다.

### CLI는 최소 실행 표면만 유지한다

V3의 operator CLI 목록은 V4 MVP에서 제외한다.

V4 기준:

- 유지: `trade-engine doctor`, `trade-engine run --dry-run --once`,
  `trade-engine run --dry-run`, `trade-engine backtest`
- 제외: `decisions list`, `risk blocks`, `executions list`, `portfolio show`,
  `report daily`, `strategy validate/import`, `panic-stop`, `resume-live`
- 운영/조회 표면은 나중에 필요할 때 별도 설계한다.
- 현재 CLI는 제품의 전체 operator surface가 아니라 runtime smoke/dev harness에
  가까운 최소 실행 경계다.

---

## 현재 구현 기준선

| 영역 | V4 기준 현재 상태 |
| --- | --- |
| Workspace | `kis-client`, `trade-engine` workspace 구성 완료 |
| KIS client | auth, token cache, transport, account/market/order/reconcile foundation 구현 |
| Main DB | migrations와 주요 audit table 일부 구현 |
| Broker journal | `SqliteBrokerCommandJournal` 구현 |
| Gateway | read 중심 `TradingGateway`, `KisGateway`, `MockTradingGateway` 구현 |
| Startup doctor | `trade-engine doctor`와 startup preflight 구현 |
| Runtime lifecycle | 하나의 run 아래 여러 tick 저장, 실패 tick 저장 구현 |
| Snapshot collection | cash/positions/quotes 저장, open orders/fills는 확장 필요 |
| Strategy | Rust 내장 hold/SMA 기반 |
| Decision | domain/schema validation 일부 구현, provider boundary 미구현 |
| Risk | read-only/live-disabled/symbol/max notional 일부 구현 |
| Execution | domain object와 DB link helper 일부 구현, broker submit workflow 미구현 |
| Reconciliation | kis-client helper와 journal primitive 존재, product workflow 미구현 |
| Panic stop | table/계획 수준, risk gate 미구현 |
| Certification | 계획 수준 |

---

## 제품 경계

### `kis-client`

소유 책임: KIS broker adapter.

책임:

- KIS auth/token/cache/redaction.
- KIS REST transport, headers, TR ID, hashkey, retry/rate limit.
- KIS raw payload mapping.
- Broker domain model과 `BrokerError`.
- `BrokerClient` trait와 `KisBrokerClient`.
- Broker command journal trait.
- Broker write guardrail.
- Ambiguous outcome 분류와 reconciliation helper.
- KIS endpoint contract test.

비책임:

- 제품 DB 소유.
- 제품 runtime scheduling.
- Strategy, decision provider, risk policy.
- Operator/reporting surface.

### `trade-engine`

소유 책임: 제품 runtime engine.

책임:

- Config loading과 startup doctor.
- Main DB, migrations, audit schema.
- DB-backed broker command journal implementation.
- Gateway abstraction과 KIS adapter 조립.
- Runtime run/tick lifecycle.
- Snapshot collection.
- Rust built-in strategy execution.
- External command advisor proposal provider boundary.
- Risk/policy engine.
- Execution workflow.
- Product-level reconciliation workflow.
- Panic stop과 certification gate.

중요 경계:

- Runtime, strategy, decision, risk, execution code는 KIS transport를 직접 호출하지
  않는다. Gateway 또는 `kis-client` public broker API를 통해서만 접근한다.
- LLM provider는 proposal JSON만 반환한다. Gateway, DB write, broker write 권한을
  갖지 않는다.

---

## Main DB First

V4에서 Main DB는 선택 사항이 아니라 제품의 system of record다.

필수 규칙:

- 모든 runtime run과 tick은 DB에 기록한다.
- 모든 snapshot 수집 성공/실패는 DB에 감사 가능하게 남긴다.
- 모든 decision input/output/proposal은 DB에 남긴다.
- 모든 risk evaluation과 block reason은 DB에 남긴다.
- 모든 execution request와 execution event는 DB에 남긴다.
- 모든 broker write command는 broker command journal에 먼저 durable하게 남긴다.
- Certification gate와 operator approval record도 DB에 남긴다.
- DB migration history는 append-only다.

V4에서 우선 확정할 table 범위:

- `runtime_runs`
- `runtime_ticks`
- `portfolio_snapshots`
- `market_snapshots`
- `strategy_signals`
- `decision_inputs`
- `decision_outputs`
- `decision_proposals`
- `risk_evaluations`
- `risk_blocks`
- `execution_requests`
- `execution_events`
- `broker_command_journal`
- `broker_command_events`
- `operator_actions`
- `panic_stop_state` 또는 동등한 operator state table
- `certification_records`

Open orders, fills, buying power는 execution/risk 확장 단계에서 snapshot 또는
execution 관련 table로 추가한다.

---

## Config 모델

V4 config는 runtime의 안전 조건을 명시해야 한다.

예상 TOML 구조:

```toml
read_only = true
watchlist = ["005930", "000660"]

[kis]
environment = "live"
app_key_env = "KIS_APP_KEY"
app_secret_env = "KIS_APP_SECRET"
account_key_env = "KIS_ACCOUNT_KEY"
token_cache_path = ".local/state/trade-engine/kis-token.json"

[storage]
main_db_path = ".local/state/trade-engine/main.db"

[runtime]
market_tick_seconds = 30
off_hours_market_tick_seconds = 900
max_tick_runtime_seconds = 180
skip_if_previous_tick_running = true
market_session_ttl_seconds = 300

[strategy]
id = "sma-crossover-v1"
short_window = 5
long_window = 20

[advisor]
provider = "command"
command = "sh"
args = ["trade-engine/scripts/codex-advisor"]
timeout_seconds = 120
max_stderr_bytes = 4096
max_stdout_bytes = 262144

[risk]
enable_live_trading = false
max_order_notional = "1000000"
max_position_notional = "5000000"
max_daily_order_count = 3

[live]
confirmed_account_key = "12345678-01"
allowed_accounts = ["12345678-01"]
require_certification = true
```

Config 원칙:

- Live write 관련 field가 빠지면 fail closed한다.
- `kis.environment`와 DB audit environment는 일치해야 한다.
- Runtime config는 logging용 값이 아니라 실제 control flow에 연결해야 한다.
- Decision provider command는 shell 문자열이 아니라 path와 args 배열로 모델링한다.
- Secret 값 자체는 config 파일에 넣지 않는다. env var 이름만 config에 둔다.

---

## Startup Doctor

`trade-engine` startup은 runtime loop보다 먼저 결정론적으로 실행되어야 한다.

Read-only/live dry-run doctor 범위:

1. Config를 로드하고 validate한다.
2. `storage.main_db_path`로 Main DB를 열고 migration을 적용한다.
3. DB-backed broker command journal을 연다.
4. unresolved journal record가 있으면 startup을 실패시킨다.
5. KIS credential env var 존재를 확인한다.
6. KIS gateway를 만든다.
7. Broker `ping`을 실행한다.
8. Account snapshot을 수집한다.
9. Watchlist quote를 모두 조회한다.
10. Market session을 조회한다.
11. Doctor report를 출력하고 DB에 남긴다.

Full live runtime doctor 범위:

1. Read-only doctor 범위를 모두 통과한다.
2. Live write config가 모두 있는지 확인한다.
3. Account allowlist와 confirmed account key를 확인한다.
4. Single-writer lock을 획득한다.
5. Panic stop 상태가 아닌지 확인한다.
6. Certification record가 live write를 허용하는지 확인한다.
7. Startup reconciliation을 실행한다.
8. unresolved journal record가 남아 있으면 live write를 차단한다.

Doctor는 통과 여부를 숨겨진 메모리 상태로만 두지 않는다. DB에 실행 결과와 실패
reason을 남기는 방향으로 확장한다.

---

## Runtime Run/Tick Lifecycle

현재 구현된 기준:

- `run --dry-run`은 하나의 `runtime_runs`를 만들고 여러 `runtime_ticks`를 그
  아래에 기록한다.
- Tick sequence를 저장한다.
- Snapshot failure는 failed tick으로 기록한다.
- Loop 종료 시 run status를 completed/failed로 갱신한다.

V4에서 남은 runtime 보강:

- tick row를 더 이른 시점에 `started`로 먼저 insert한다.
- tick lease를 구현한다.
- `skip_if_previous_tick_running`을 실제 control flow에 연결한다.
- `max_tick_runtime_seconds` timeout을 실제 tick 실행에 적용한다.
- graceful shutdown을 처리하고 run을 `interrupted` 또는 적절한 terminal status로
  기록한다.
- Market session은 매 tick 직접 조회하지 않고 TTL cache를 둔다.
- 장이 닫힌 경우에도 read-only snapshot/decision/risk audit은 수집하되,
  write 의도는 market-closed risk gate로 차단한다.

후순위:

- restart 후 기존 run resume.
- multi-process scheduler.
- WebSocket/event-driven market feed.

---

## Tick Flow와 실패 정책

V4 tick은 보수적인 fail-closed 정책을 따른다.

Tick 순서:

1. Panic stop과 shutdown signal을 확인한다.
2. Runtime mode, read-only, live gate를 확인한다.
3. Tick lease를 획득한다.
4. Market session cache를 확인한다.
5. Account snapshot을 수집한다.
6. Watchlist quote snapshot을 모두 수집한다.
7. Snapshot을 DB에 저장한다.
8. Rust strategy signal을 생성한다.
9. Decision request를 만든다.
10. Decision provider를 호출한다.
11. Decision output을 schema validate한다.
12. Invalid decision은 저장 후 reject한다.
13. Risk policy를 실행한다. 장이 닫힌 경우 write proposal은 market-closed로 block된다.
14. Risk block이면 block reason을 저장하고 tick을 종료한다.
15. Hold이면 execution 없이 tick을 완료한다.
16. Risk approved buy/sell이면 execution request를 만든다.
17. Tick summary와 terminal status를 저장한다.

실패 정책:

- Account snapshot 실패: tick `snapshot_failed`, 주문 판단 없음.
- Watchlist quote 중 하나라도 실패: 전체 tick `snapshot_failed`, 주문 판단 없음.
- Stale quote 재사용 금지.
- Symbol별 skip은 V4 MVP에서 허용하지 않는다.
- Provider timeout/non-zero exit/invalid JSON: decision failure/rejection으로 저장하고
  주문 없음.
- 장이 닫힌 경우는 실패가 아니다. Snapshot/decision/risk audit은 남기고 write
  proposal은 market-closed risk block으로 종료한다.
- Snapshot 실패는 runtime crash가 아니라 "데이터 불충분으로 거래하지 않음"으로
  감사한다.

Tick status 후보:

- `started`
- `skipped_previous_tick_running`
- `snapshot_failed`
- `snapshot_collected`
- `signal_generated`
- `decision_failed`
- `decision_rejected`
- `decision_proposed`
- `risk_blocked`
- `execution_requested`
- `completed`
- `failed`
- `interrupted`

---

## Strategy

V4에서는 strategy를 Rust 코드로 유지한다.

지원할 내장 전략:

- `hold-only`
- `sma-crossover-v1`

Strategy config:

- strategy id
- strategy parameter
- watched symbols
- minimum required price history

규칙:

- Strategy는 broker write를 할 수 없다.
- Strategy는 `StrategySignal`만 만든다.
- Strategy output은 DB에 저장되어야 한다.
- Runtime은 hard-coded default strategy가 아니라 config의 strategy id와 parameter를
  사용한다.
- Strategy 다양화보다 risk/execution/reconciliation 안전 경로를 우선한다.

후순위:

- `.kis.yaml`
- strategy import/validate CLI
- persisted backtest audit/promotion harness
- strategy marketplace

---

## Proposal Provider Boundary

V4는 LLM proposal 생성을 MVP 범위에 포함한다. 단, LLM은 proposal만 만든다.

Provider 구조:

- Built-in deterministic advisor fallback
- `CommandAdvisorProvider`
- Legacy `[decision] provider = "command"` config는 deprecated이며 runtime에서
  거부한다.

Command provider config:

```toml
[advisor]
provider = "command"
command = "sh"
args = ["trade-engine/scripts/codex-advisor"]
timeout_seconds = 120
max_stderr_bytes = 4096
max_stdout_bytes = 262144
```

동작:

1. `trade-engine`이 `AdvisorRequest` JSON을 provider stdin으로 전달한다.
2. Provider가 `AdvisorOutput` JSON을 stdout으로 반환한다.
3. stdout/stderr는 길이 제한 후 diagnostics로 저장할 수 있다.
4. Non-zero exit, timeout, invalid JSON, schema mismatch는 advisory failure/rejection이다.
5. 실패한 advisory는 주문을 만들지 않는다.

`DecisionRequest`에 포함할 수 있는 정보:

- runtime mode
- environment
- account label 또는 account suffix
- cash
- positions
- quotes
- strategy signal
- recent prices
- open orders summary
- fills summary
- risk hints

`DecisionRequest`에 포함하지 않는 정보:

- full account key
- app key
- app secret
- access token
- hashkey
- token cache content
- raw credential file

Output schema:

```json
{
  "decision_type": "buy",
  "symbol": "005930",
  "quantity": 10,
  "order_type": "limit",
  "limit_price": "73000",
  "confidence": "0.71",
  "time_horizon": "intraday",
  "reason": "signal summary",
  "risk_notes": ["risk note"]
}
```

Validation rules:

- Unknown field는 reject한다.
- Unknown symbol은 reject한다.
- Unsupported order type은 reject한다.
- Quantity와 price는 typed parsing을 통과해야 한다.
- `hold`는 order fields 없이 valid할 수 있다.
- `buy`/`sell`은 required order fields가 없으면 reject한다.
- Reason/risk notes는 길이 제한을 둔다.
- Confidence는 risk input일 뿐 execution approval이 아니다.

---

## Risk Policy

Risk policy는 execution 전 최종 권한이다.

V4 risk 확장 순서:

1. Runtime mode 기반 block reason 정합성
   - `read_only=true`면 `ReadOnlyMode`가 반드시 기록된다.
   - live disabled면 `LiveTradingDisabled`도 기록할 수 있다.
2. Market session gate
   - 장 닫힘이면 신규 주문을 차단한다.
3. Unresolved journal gate
   - unresolved broker command가 있으면 신규 live write를 차단한다.
4. Panic stop gate
   - panic stop active이면 write를 차단한다.
5. Buying power gate
   - 주문가능금액 부족을 차단한다.
6. Duplicate intent/open orders gate
   - 같은 symbol/side/order intent 중복을 방지한다.
7. Exposure and daily limits
   - max position notional
   - max daily order count
   - max daily loss
8. Strategy/certification gate
   - configured strategy id
   - paper observation period
   - certification record
   - operator approval record

Risk rules:

- Risk block은 모두 durable reason을 가져야 한다.
- Risk policy는 LLM decision을 신뢰하지 않는다.
- Risk policy는 broker guardrail보다 제품 수준에서 먼저 실패해야 한다.
- Submit 직전 final risk check를 다시 실행한다.
- Unresolved journal과 panic stop은 final risk check에서도 다시 확인한다.

---

## Gateway와 Execution

V4에서는 단일 `TradingGateway` abstraction을 유지한다.

Gateway 확장 방향:

- read methods:
  - ping
  - account snapshot
  - cash
  - positions
  - quote
  - market session
  - buying power
  - open orders
  - fills
- write methods:
  - place cash order
  - cancel order
  - amend order

Execution MVP 범위:

- cash buy place order
- cash sell place order

후순위:

- cancel
- amend
- complex order types
- overseas live write workflow

Execution workflow:

1. DB에서 risk-approved execution request를 읽는다.
2. `ClientOrderId`를 생성한다.
3. Broker command journal record를 reserve한다.
4. Execution request와 journal record를 link한다.
5. Submit 직전 final risk check를 실행한다.
6. Gateway를 통해 place order를 submit한다.
7. 결과별 execution event와 status를 저장한다.

결과 처리:

- Accepted:
  - execution status `accepted`
  - `OrderAccepted` event
  - broker order reference 저장
- Rejected:
  - execution status `rejected`
  - `OrderRejected` event
  - terminal state
- Ambiguous:
  - execution status `reconciliation_needed`
  - `ReconciliationNeeded` event
  - journal unresolved 유지
  - blind retry 금지
- Final risk failure:
  - execution status `failed` 또는 `blocked`
  - `ExecutionFailed` event
  - broker submit 없음

Execution 불변식:

- Broker write 전에 execution request가 DB에 있어야 한다.
- Broker write 전에 journal command가 durable해야 한다.
- KIS에 write가 도달했을 수 있으면 같은 command를 blind retry하지 않는다.
- Read-only runtime은 execution workflow를 시작하지 않는다.

---

## Reconciliation

Execution MVP가 들어가면 product-level reconciliation도 필수다.

V4 reconciliation MVP 범위:

- ambiguous place order 복구
- unresolved journal startup block
- pending reconciliation workflow
- KIS order/fill inquiry 기반 command state 갱신
- still unknown이면 unresolved 유지

Workflow:

1. DB-backed journal에서 unresolved command를 읽는다.
2. KIS order/fill inquiry를 조회한다.
3. client order id, broker order id, symbol/side/quantity/time window 순서로 매칭한다.
4. Journal state를 갱신한다.
5. Execution request state를 갱신한다.
6. Reconciliation event를 저장한다.

결과:

- `ReconciliationSucceeded`
- `ReconciliationStillUnknown`
- `ManualInterventionRequired`
- `OrderFilled`
- `OrderCancelled`
- `OrderRejected`

규칙:

- `ReconciliationStillUnknown` 또는 `ManualInterventionRequired`가 남아 있으면 live
  write는 계속 차단된다.
- `resume-live` 같은 operator action은 unresolved journal을 지우지 않는다.
- Cancel/amend reconciliation은 V4 후속 phase로 둔다.

---

## Panic Stop

V4에서는 panic stop을 CLI command가 아니라 risk gate로 먼저 구현한다.

필수 동작:

- Panic stop state를 DB 또는 명시적 runtime state에 저장한다.
- Panic stop active이면 모든 write risk evaluation이 block된다.
- Final risk check에서도 panic stop을 다시 확인한다.
- Panic stop clear는 unresolved journal을 clear하지 않는다.
- Panic stop activate/clear는 V4 MVP에서는 library API 또는 DB admin operation으로
  제공할 수 있다.

후순위:

- dedicated CLI
- Discord command
- Web UI
- approval workflow

---

## Runtime Surface

V4에서 유지하는 최소 실행 표면:

- `trade-engine doctor`
- `trade-engine run --dry-run --once`
- `trade-engine run --dry-run`
- `trade-engine backtest`

이 CLI는 운영 제품 전체 표면이 아니다. 다음 용도로만 본다.

- local smoke
- runtime dev harness
- KIS read-only connectivity check
- DB-backed dry-run verification
- stored OHLCV baseline research without broker access

V4에서 제외하는 CLI:

- `config validate`
- `strategy validate`
- `strategy import`
- `decisions list`
- `risk blocks`
- `executions list`
- `portfolio show`
- `report daily`
- `panic-stop`
- `resume-live`

필요해지면 operator surface는 별도 phase에서 다시 설계한다.

---

## Security와 Secret 정책

V4는 계좌번호와 secret을 다르게 취급한다.

Account key:

- 로컬 DB 저장 허용.
- 로컬 CLI/log/report 출력 허용.
- LLM prompt 전송 금지.
- 실제 계좌번호를 git-tracked fixture나 문서 예시에 넣지 않는다.

Secret:

- `KIS_APP_SECRET` 저장 금지.
- Main DB, log, report, prompt에는 access token 저장 금지. 단, app-managed token
  cache file은 runtime state로 허용하고 git에서 제외한다.
- hashkey 저장 금지.
- OpenAI/API provider secret 저장 금지.
- Secret은 log, DB, report, decision input, decision output, prompt에 포함하지 않는다.

Runtime state:

- Main DB
- token cache
- runtime logs
- lock files

위 파일들은 commit 대상이 아니다.

---

## Certification Gate

기능 구현과 live readiness는 다르다. V4는 live write 전에 certification gate를
필수로 둔다.

필수 순서:

1. 전체 Rust verification 통과
2. Fake gateway runtime loop
3. KIS live read-only doctor
4. DB-backed dry-run loop 일정 기간
5. External command LLM decision dry-run
6. Risk block 감사 확인
7. Controlled paper place order
8. Paper ambiguous outcome/reconcile drill
9. Seeded unresolved startup block drill
10. Operator approval record
11. Explicit live enable config
12. Limited live write

V4 MVP에서는 certification UI를 만들지 않는다. 대신 certification record와 risk/config
조건으로 live write를 차단한다.

규칙:

- Backtest success만으로 live write를 승인하지 않는다.
- LLM dry-run success만으로 live write를 승인하지 않는다.
- Operator approval은 DB에 durable하게 남아야 한다.
- Certification record가 없으면 live write는 fail closed한다.

---

## 구현 Phase

### Phase 0. V4 문서와 기준선 정리

목표:

- `PLAN_V4.md`를 추가한다.
- README와 AGENTS가 V4와 충돌하지 않게 정리한다.
- V3의 폐기된 기준을 명시한다.

수용 기준:

- `plans/PLAN_V4.md`가 존재한다.
- V4가 V3보다 우선한다는 문서 기준이 명확하다.
- 전체 verification이 통과한다.

### Phase 1. Config V4

목표:

- `kis.environment`를 추가한다.
- `storage.main_db_path` 필수화를 문서/코드/error에 반영한다.
- strategy TOML config를 추가한다.
- decision command provider config를 추가한다.
- live safety config를 fail-closed로 validate한다.

수용 기준:

- paper/live environment가 DB audit에 정확히 저장된다.
- missing live safety field는 actionable error로 실패한다.
- secret 값은 config 파일에 들어가지 않는다.

### Phase 2. Runtime Stabilization

목표:

- tick `started` 선기록.
- tick lease.
- previous tick running skip.
- tick timeout.
- graceful shutdown.
- market session TTL cache.
- closed-market dry-run audit with market-closed risk gating.

수용 기준:

- timeout tick은 status와 error를 DB에 남긴다.
- previous tick skip은 durable tick 또는 run summary에 남는다.
- graceful shutdown은 run terminal status를 남긴다.
- market closed는 실패가 아니며 snapshot/decision/risk audit을 남기고 write proposal은
  risk에서 차단한다.

### Phase 3. Risk Policy V4 Core

목표:

- runtime mode block reason 정합성.
- market session gate.
- unresolved journal gate.
- panic stop gate.

수용 기준:

- `read_only=true`면 `ReadOnlyMode`가 기록된다.
- unresolved journal은 live write를 차단한다.
- panic stop active이면 final risk check도 실패한다.
- 모든 block reason은 DB에 저장된다.

### Phase 4. Built-in Strategy Config

목표:

- strategy TOML config를 로드한다.
- `hold-only`, `sma-crossover-v1`를 config로 선택한다.
- SMA parameter를 config에서 받는다.

수용 기준:

- runtime에서 hard-coded default strategy 의존이 제거된다.
- invalid strategy id/parameter는 config error로 실패한다.
- signal generation은 deterministic test로 검증된다.

### Phase 5. Proposal Provider

목표:

- Built-in deterministic advisor fallback.
- `CommandAdvisorProvider`.
- Advisor request/output schema.
- Provider timeout/error handling.

수용 기준:

- provider stdin request에는 full account key와 secret이 없다.
- invalid provider output은 raw output과 rejection reason을 저장한다.
- timeout/non-zero exit은 주문 없이 failure로 기록된다.
- deterministic fallback tests와 command advisor contract tests가 통과한다.

### Phase 6. Risk Policy V4 Extended

목표:

- buying power gate.
- duplicate intent/open orders gate.
- exposure and daily limits.
- certification gate.

수용 기준:

- buying power 부족은 write를 차단한다.
- 기존 open order와 중복되는 intent는 차단된다.
- daily limits가 DB state 기반으로 동작한다.
- certification record가 없으면 live write가 차단된다.

### Phase 7. Execution MVP

목표:

- cash buy/sell place order workflow.
- execution request claim/read.
- client order id generation.
- journal reserve/link.
- final risk check.
- gateway submit.
- accepted/rejected/ambiguous state transition.

수용 기준:

- accepted order는 broker reference와 event를 저장한다.
- rejected order는 terminal rejected로 저장한다.
- ambiguous outcome은 reconciliation-needed로 저장한다.
- broker write 후 blind retry가 없다.
- read-only mode에서는 execution workflow가 submit하지 않는다.

### Phase 8. Reconciliation MVP

목표:

- pending unresolved place order reconciliation.
- startup block integration.
- execution state와 journal state 동시 갱신.

수용 기준:

- seeded unresolved command가 startup을 block한다.
- inquiry로 확인된 order/fill은 reconciled 상태로 갱신된다.
- still unknown은 unresolved로 남아 live write를 계속 block한다.
- reconciliation 결과 event가 DB에 저장된다.

### Phase 9. Panic Stop과 Certification Record

목표:

- panic stop state 저장.
- panic stop risk gate.
- certification record 저장.
- operator approval record 저장.

수용 기준:

- panic stop active 상태에서 write가 차단된다.
- panic stop clear가 unresolved journal을 삭제하지 않는다.
- certification record가 없는 live write는 실패한다.
- limited live enable은 explicit config와 approval record를 모두 요구한다.

### Phase 10. Cancel/Amend와 후속 운영 표면

목표:

- cancel/amend execution workflow.
- cancel/amend reconciliation.
- 필요 시 operator surface 재설계.

수용 기준:

- follow-up write는 KIS possible quantity preflight를 사용한다.
- ambiguous cancel/amend는 reconciliation-needed로 들어간다.
- operator surface는 DB를 source of truth로 사용한다.

---

## V4 MVP 완료 기준

V4 MVP는 다음을 만족할 때 완료로 본다.

- Main DB가 필수 system of record로 사용된다.
- Config에서 KIS environment를 명시한다.
- `doctor`가 DB migration, unresolved journal, ping, account snapshot, quote,
  market session을 확인한다.
- Dry-run loop가 tick lease, timeout, graceful shutdown, closed-market dry-run audit을
  감사한다.
- Snapshot 실패는 stale data 없이 fail-closed로 기록된다.
- Strategy는 TOML config로 선택된 Rust built-in strategy를 사용한다.
- External command advisor proposal provider가 동작한다.
- Provider failure는 주문 없이 DB에 기록된다.
- Risk policy가 read-only, live-disabled, market session, unresolved journal,
  panic stop, buying power, duplicate intent를 차단한다.
- Cash buy/sell place order execution workflow가 accepted/rejected/ambiguous를
  처리한다.
- Ambiguous place order는 reconciliation workflow로 복구된다.
- Panic stop과 certification gate가 live write를 차단할 수 있다.
- Full workspace verification이 통과한다.

필수 verification:

```sh
cargo check --workspace --no-default-features
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo doc --no-deps --all-features
```

---

## 안전 불변식

- LLM decision output은 order가 아니다.
- LLM provider는 broker write 권한을 갖지 않는다.
- Full account key는 LLM prompt에 포함하지 않는다.
- Secret, token, hashkey는 DB/log/report/prompt/fixture에 포함하지 않는다.
- Risk policy가 최종 write authority다.
- Live writes는 기본적으로 disabled다.
- Broker write 전에 execution request가 durable해야 한다.
- Broker write 전에 broker command journal record가 durable해야 한다.
- Submit 직전 final risk check를 통과해야 한다.
- 동일 live account에는 single writer만 허용한다.
- Unresolved journal record는 live write를 block한다.
- Panic stop은 clear될 때까지 live write를 block한다.
- Certification record 없이는 live write가 불가능하다.
- Ambiguous broker outcome은 blind retry하지 않는다.
- Backtest, LLM, strategy signal, confidence만으로 live write를 승인하지 않는다.
- Runtime state file과 DB는 commit하지 않는다.
