# 트레이딩 에이전트 계획 V3

## 상태

이 문서는 `plans/PLAN_V2.md`의 제품 방향과 직전 자동매매 런타임 계획을
정리해, 실제 구현할 다음 제품 crate 이름을 `trade-engine`으로 확정한 V3
계획이다.

- 기준일: 2026-04-25
- V3에서의 제품 런타임 crate 이름:
  - Cargo package: `trade-engine`
  - Rust library crate: `trade_engine`
  - repository 이름 `trading-agent`와 충돌하지 않는다.
- `agent`라는 이름은 crate 이름으로 쓰지 않는다. 너무 넓고, LLM 에이전트와
  제품 런타임의 책임이 섞인다.
- `trading-agent`라는 crate/package 이름도 쓰지 않는다. repository 이름과
  겹치기 때문이다.
- 현재 실제 workspace 기준선은 `kis-client`만 구현된 상태다. `trade-engine`
  crate는 아직 생성되지 않았다.
- `kis-client`는 library-only KIS broker adapter로 유지한다.
- `plans/INITIAL_PLAN.md`는 과거 계획 참고 자료다.
- `plans/PLAN_V2.md`는 V3 이전 제품 계획이며, V3와 충돌하면 V3가 우선한다.
- `plans/KIS_CLIENT_IMPLEMENTATION_PLAN.md`와
  `plans/KIS_CLIENT_IMPROVEMENT_PLAN.md`는 KIS client component 계획이다.

참고한 외부 기준:

- `koreainvestment/open-trading-api`
  - KIS 공식 샘플, `.kis.yaml`, strategy builder, backtester, MCP 참고.
- `koreainvestment/kis-ai-extensions`
  - 전략 설계, 백테스트, BUY/SELL/HOLD 신호 기반 주문, 보안 훅 참고.
- FINRA algorithmic trading guidance
  - 자동매매 개발, 테스트, 감독, 운영 통제 참고.
- SEC Market Access Rule FAQ
  - 자동화된 사전 주문 통제와 financial/regulatory risk control 관점 참고.
- QuantConnect Lean 문서
  - backtest, paper, live 사이의 차이와 stateful live deployment 참고.

---

## 이어갈 기존 제품 계획

V2의 핵심 원칙은 그대로 유지한다.

- AI/LLM 에이전트는 제안만 한다.
- 주문 실행과 기록은 결정론적 Rust 코드가 담당한다.
- 리스크와 정책 계층이 최종 권한을 가진다.
- broker response, fills, decisions, risk blocks, reconciliation 결과는
  감사 가능해야 한다.
- KIS write endpoint는 Python 샘플로 다시 만들지 않고, Rust `kis-client`를
  broker adapter foundation으로 유지한다.
- `.kis.yaml`은 first-class strategy artifact로 다룬다.
- 전략은 Backtest -> Paper -> Limited Live promotion gate를 통과해야 한다.
- MCP와 KIS AI extension은 research surface와 참고 구현으로만 사용한다.
- MCP, shell hook, AI plugin은 live 주문 권한을 직접 갖지 않는다.

V3에서 바뀌는 점은 제품 crate 이름과 현재 기준선 인식이다.

- V2의 `agent` crate 계획은 `trade-engine` crate 계획으로 이름을 바꾼다.
- `trade-engine` 안에서도 `agent/`라는 top-level module 이름은 피한다.
  대신 `decision/`, `advisor/`, `strategy/`, `risk/`, `execution/`처럼
  책임이 분명한 이름을 사용한다.
- 현재 repository에 product runtime crate가 이미 있다고 가정하지 않는다.
  `trade-engine` crate를 새로 만든다.

---

## 이름 결정

`trade-engine`을 선택한다.

이 crate가 맡을 책임은 LLM 에이전트 하나가 아니라 다음 전체 제품 런타임이다.

- 런타임 루프
- 시장/계좌 스냅샷
- 전략 신호
- AI 판단 제안
- 리스크 정책
- 실행
- 저장소/감사 기록
- 운영자 CLI/reporting

따라서 `agent`는 너무 좁고 애매하다. `trade-engine`은 전략, 리스크, 실행,
운영 런타임을 모두 묶는 이름으로 더 적합하다.

추천 명명 규칙:

```text
trade-engine/
  Cargo.toml          # package = "trade-engine"
  src/main.rs         # runtime binary entrypoint
  src/lib.rs          # crate = trade_engine
  src/runner.rs       # config/CLI request to runtime orchestration
```

`trade-engine` 내부에서 AI 판단 계층은 `decision` 또는 `advisor` module로 둔다.
문서에서는 개념적으로 "agent decision"이라고 부를 수 있지만, crate/module 이름은
구체적으로 유지한다.

---

## 제품 경계

### `kis-client`

소유 책임: KIS broker adapter.

책임:

- `app_key`와 `app_secret` 기반 KIS 인증/token 처리.
- KIS REST transport, headers, TR ID, hashkey support, redaction.
- 국내주식 시세, 계좌, 주문가능금액, 주문, 정정/취소, 주문/체결 조회.
- Broker domain model과 `BrokerError`.
- Durable command journal contract.
- Broker 경계에 속하는 write guardrail.
- Single-writer lock 지원.
- Ambiguous outcome 분류와 reconciliation helper.
- KIS 공식 샘플 동작에 대한 contract test.

비책임:

- 전략 로직.
- AI prompt 구성.
- 포트폴리오 정책.
- 제품 DB 소유.
- 런타임 스케줄링.
- 운영자 CLI.
- Discord reporting.

### `trade-engine`

소유 책임: 제품 런타임.

책임:

- 제품 설정과 startup doctor.
- Main DB와 migrations.
- `kis-client::journal::BrokerCommandJournal`의 storage-backed 구현체.
- 제품-facing gateway trait와 KIS gateway adapter.
- 시장/계좌 스냅샷 수집.
- Strategy spec parsing과 deterministic signal generation.
- AI decision proposal schema와 validation.
- Risk/policy engine.
- `kis-client`를 호출하는 execution engine.
- 지속 실행되는 live dry-run/runtime loop.
- 운영자 CLI와 reporting.
- Backtest -> paper -> limited live promotion gate.

중요 경계:

- `trade-engine`은 `kis-client`를 호출할 수 있다.
- `kis-client`는 `trade-engine`에 의존하면 안 된다.
- 런타임, 전략, 리스크, reporting, CLI 코드는 KIS transport를 직접 호출하면
  안 된다. 이 코드는 `TradingGateway`에 의존해야 한다.

---

## 목표 아키텍처

```text
                 research / design
        +----------------------------------+
        | .kis.yaml StrategySpec           |
        | Backtest Adapter / Reports       |
        | AI Decision Proposal             |
        +----------------+-----------------+
                         |
                         v
+-------------+   +-------------+   +----------------+
| Market Data |-->| Strategy    |-->| Decision       |
| + Account   |   | Signals     |   | Proposal       |
+------+------+   +------+------+   +-------+--------+
       |                 |                  |
       +-----------------+------------------v
                                      +-------------+
                                      | Risk/Policy |
                                      | Final Gate  |
                                      +------+------+
                                             |
                                             v
                                      +-------------+
                                      | Execution   |
                                      | Engine      |
                                      +------+------+
                                             |
                                             v
                                      +-------------+
                                      | kis-client  |
                                      +------+------+
                                             |
                                             v
                                      +-------------+
                                      | KIS OpenAPI |
                                      +-------------+
```

거래 상태에 영향을 주는 모든 단계는 `trade-engine` main DB에 감사 기록을 남긴다.

---

## 현재 `kis-client` 기준선

현재 `kis-client`는 `trade-engine`이 필요로 하는 broker foundation을 이미
제공한다.

사용 가능한 기능:

- Public API는 crate root와 curated module만 노출한다. `auth`, `client`,
  `config`, `domain`, `error`, `journal`, `safety`, `transport`가 public
  surface이고, KIS raw `mapping`, `protocol`, `reconcile`은 기본적으로 내부
  module로 유지한다.
- `core/`는 broker trait, typed domain model, `BrokerError`, durable
  command journal contract를 소유한다.
- `auth/`는 `app_key`와 `app_secret` 기반 인증, access token 발급/cache,
  refresh lock, credential redaction을 소유한다. Access token은 public
  runtime 설정이 아니라 provider 내부 산출물이다.
- `transport/`는 base URL, endpoint metadata, request/response audit,
  KIS error classification, hashkey, rate limit, retry 정책을 소유한다.
- `protocol/`은 KIS 국내주식 endpoint metadata와 TR ID 규칙을 소유한다.
- `mapping/`은 KIS raw account/market/order response를 broker domain으로
  변환한다.
- `client/`는 `KisBrokerClient`와 `KisClientBuilder`를 제공하고 account,
  market, orders, recovery, guardrails를 파일 단위로 분리한다.
- `KisClientBuilder`는 environment, credential, token cache, runtime config,
  journal, single-writer lock, rate limit, retry policy로 `KisBrokerClient`를
  만든다. KIS 계좌 식별 세부는 `trade-engine` 공통 설정으로 끌어올리지 않고
  KIS adapter 조립 경계 안에 캡슐화한다.
- Public broker API는 `BrokerClient`로 노출된다.
- 계좌 read는 positions, cash balance, buying power를 포함한다.
- 시장 read는 quote와 market session lookup을 포함한다.
- 주문 write는 cash order, cancel, amend를 포함한다.
- 주문/체결 조회는 reconciliation을 지원한다.
- Live write는 runtime config, market session gate, journal, single-writer
  lock, unresolved reconciliation check로 보호된다.
- 애매한 write 실패는 blind retry 대신 reconciliation-needed 상태를 반환한다.

`trade-engine`이 제공해야 할 것:

- 운영 DB 기반 `BrokerCommandJournal`.
- 런타임 설정 출처.
- 시작 시점 복구 순서.
- Broker guardrail 위의 제품 수준 리스크 검사.
- 운영자가 사용할 수 있는 workflow.

---

## 우선 구현할 Read-only MVP

첫 번째 MVP는 자동매매가 아니라 관측용 smoke runtime이다.

목표:

- KIS 인증이 동작하는지 확인한다.
- 현재 계좌 잔고와 보유 종목을 가져온다.
- 설정한 몇 개 종목의 현재가를 가져온다.
- 결과를 structured log로 보여준다.
- Broker write는 전혀 수행하지 않는다.

이 단계에서는 아래 기능을 만들지 않는다.

- Main DB persistence.
- Strategy signal.
- AI decision proposal.
- Risk engine.
- Execution request.
- Broker order submit/cancel/amend.
- Reconciliation workflow.
- Discord reporting.

Read-only MVP 실행 흐름:

1. Config를 로드한다.
2. Env에서 `KIS_APP_KEY`, `KIS_APP_SECRET`을 읽는다.
3. `KisClientBuilder`로 paper 환경 KIS client를 만든다.
4. Access token을 발급하거나 token cache에서 재사용한다.
5. `get_cash_balance()`로 현금 잔고를 조회한다.
6. `get_positions()`로 보유 종목과 수량을 조회한다.
7. Config의 watchlist symbol마다 `get_quote()`를 호출한다.
8. 계좌번호는 suffix만 남기고 redaction한다.
9. 잔고, 보유 종목, watchlist 현재가를 로그로 출력한다.
10. 실패한 endpoint가 있으면 어떤 단계가 실패했는지 로그로 남기고 종료한다.

Read-only MVP 로그 예시:

```text
mode=paper account_suffix=1234 cash=1000000 positions=2
position symbol=005930 quantity=10 average_price=70000
quote symbol=005930 last_price=73000
quote symbol=000660 last_price=180000
```

Read-only MVP 수용 기준:

- `trade-engine run --dry-run --once`가 동작한다.
- KIS 인증 credential은 `KIS_APP_KEY`, `KIS_APP_SECRET` env var로만 받는다.
- App secret, access token, 전체 계좌번호는 로그에 나오지 않는다.
- Watchlist는 config에서 읽는다.
- Broker write 관련 코드는 호출되지 않는다.
- DB가 없어도 실행할 수 있다.
- Fake gateway test로 잔고/현재가 로그 출력을 검증한다.

`trade-engine`은 `account_key`를 공통 설정, 전략 입력, 리스크 입력으로 다루지
않는다. 계좌가 필요한 KIS read/write 호출의 계좌 식별 세부는 KIS client 또는
KIS gateway adapter 조립 경계 안에서 처리하고, `trade-engine` runtime과 로그에는
redacted account suffix 같은 표시용 값만 전달한다.

이 MVP가 통과한 뒤에만 Main DB, tick loop, strategy, decision, risk, execution을
차례대로 추가한다.

---

## 목표 저장소 구조

```text
trading-agent/
  Cargo.toml
  README.md
  plans/
    INITIAL_PLAN.md
    PLAN_V2.md
    PLAN_V3.md
    KIS_CLIENT_IMPLEMENTATION_PLAN.md
    KIS_CLIENT_IMPROVEMENT_PLAN.md
  configs/
    trade-engine.live-dry-run.toml
    prompts/
      decision.md
  strategies/
    examples/
      rsi_reversion.kis.yaml
      sma_crossover.kis.yaml
  trade-engine/
    Cargo.toml
    migrations/
      0001_command_journal.sql
      0002_runtime_audit.sql
      0003_strategy_and_decisions.sql
      0004_execution_events.sql
    src/
      main.rs
      lib.rs
      error.rs
      runner.rs
      config/
      gateway/
        traits.rs
        types.rs
        adapters/
          kis.rs
        testing/
          mock.rs
      storage/
        main_db.rs
        sqlite_journal.rs
      market/
      strategy/
      research/
      decision/
      risk/
      execution/
      runtime/
      reporting/
  kis-client/
    Cargo.toml
    README.md
    src/
    tests/
```

초기 workspace members:

```toml
[workspace]
members = ["kis-client", "trade-engine"]
resolver = "3"
```

---

## Main DB 범위

`trade-engine`이 제품 database를 소유한다.

최소 테이블:

- `broker_command_journal`
- `runtime_runs`
- `runtime_ticks`
- `market_snapshots`
- `portfolio_snapshots`
- `strategy_specs`
- `strategy_versions`
- `strategy_signals`
- `backtest_runs`
- `backtest_metrics`
- `decision_inputs`
- `decision_outputs`
- `decision_proposals`
- `risk_evaluations`
- `risk_blocks`
- `execution_requests`
- `execution_events`
- `fills`
- `operator_actions`
- `daily_reports`

규칙:

- DB migration history는 append-only다.
- 런타임 상태는 restart 후에도 복구 가능해야 한다.
- 거절된 모든 decision은 reason과 함께 저장한다.
- 모든 risk block은 durable해야 한다.
- 모든 broker write는 broker command journal record와 연결한다.
- Secret과 전체 계좌번호는 일반 감사 출력에 plain text로 저장하지 않는다.

---

## 런타임 설정

설정은 `kis-client`가 아니라 `trade-engine`이 로드한다.

이 절의 필수 설정은 Read-only MVP가 아니라 DB, strategy, risk, execution,
reporting이 모두 들어간 Full runtime 기준이다. Read-only MVP는 앞 절의 최소
설정만 요구하며, DB path, strategy file, risk limits, report targets를 필수로
요구하지 않는다.

Full runtime 필수 설정:

- KIS app key env var 이름
- KIS app secret env var 이름
- app-managed token cache
- main DB path
- single-writer lock directory
- read-only mode
- live enable flag
- confirmed account suffix
- allowed live accounts
- watchlist
- strategy file path
- risk limits
- polling interval
- report targets

KIS 계좌 식별자는 `trade-engine`의 공통 설정 항목으로 모델링하지 않는다. 계좌가
필요한 broker 호출은 KIS adapter 조립 경계에서 해결하고, 제품 runtime에는
표시/검증에 필요한 redacted suffix 또는 account label만 노출한다.

Live config는 live safety field가 하나라도 빠지면 fail closed해야 한다.

---

## 시작 순서

`trade-engine` startup은 결정론적이어야 한다. 다만 Read-only MVP와 Full
runtime은 필요한 부트스트랩 범위가 다르다.

Read-only MVP startup:

1. 최소 Config를 로드한다.
2. Environment와 KIS 인증 credential env var를 검증한다.
3. Read-only mode를 강제한다.
4. `KisClientBuilder`로 paper KIS client를 만든다.
5. Access token을 발급하거나 cache에서 재사용한다.
6. Broker `ping`을 실행한다.
7. Cash balance, positions, watchlist quote를 조회한다.
8. Secret, access token, 전체 계좌번호를 redaction한 structured log를 남긴다.
9. Broker write, DB migration, strategy load, risk/execution startup은 수행하지
   않는다.

Full runtime startup:

1. Config를 로드한다.
2. Environment와 required secrets를 검증한다.
3. Main DB를 연다.
4. Migrations를 적용하거나 검증한다.
5. `BrokerRuntimeConfig`를 만든다.
6. DB-backed `BrokerCommandJournal`를 만든다.
7. 필요하면 single-writer lock을 만들고 획득한다.
8. `KisClientBuilder`로 `KisBrokerClient`를 만든다.
9. Access token을 발급하거나 cache에서 재사용한다.
10. Broker `ping`을 실행한다.
11. `kis-client`를 통해 startup reconciliation을 실행한다.
12. Unresolved journal record가 남아 있으면 live write를 중단한다.
13. Strategy spec을 로드한다.
14. Doctor가 통과한 뒤에만 runtime loop를 시작한다.

시작 reconciliation이 끝나기 전에는 broker write를 허용하지 않는다.

---

## 런타임 Tick 흐름

각 runtime tick은 다음 순서를 따른다.

1. Panic stop과 shutdown signal을 확인한다.
2. Environment, read-only mode, live gate를 확인한다.
3. Market/account snapshot을 가져온다.
4. Snapshot을 저장한다.
5. Deterministic strategy signal을 생성한다.
6. Decision input summary를 만든다.
7. AI decision proposal을 요청하거나 deterministic fallback을 사용한다.
8. Decision schema를 검증한다.
9. Raw decision output과 parsed decision output을 저장한다.
10. Risk/policy evaluation을 실행한다.
11. Risk result를 저장한다.
12. Block되면 reporting 후 tick을 종료한다.
13. Approve되면 execution request를 만든다.
14. 중요한 상태 변화가 있으면 event를 남긴다.
15. Tick summary를 저장하고 tick을 종료한다.

Runtime은 다음 모드를 지원해야 한다.

- `run --dry-run --once`
- `run --dry-run`
- certification gate가 생긴 뒤 제한적인 `run --live`

---

## 엔진 작동 모델

`trade-engine`은 hybrid 구조로 동작한다.

- 대부분의 동작은 주기적인 tick loop에서 처리한다.
- 주문, 취소, 정정, 체결, 리스크 차단, reconciliation, panic stop 같은 중요한
  거래 상태 변화만 event로 남긴다.
- 반복적으로 발생하는 관측 데이터는 event가 아니라 snapshot과 tick summary로
  저장한다.
- Tick은 관측, 판단, risk approval, execution request 생성까지 담당한다.
- 실제 broker write는 execution workflow가 DB에 기록된 execution request를
  읽어 처리한다.

### Tick 주기

초기 기본값은 REST polling 기반으로 보수적으로 잡는다.

- MVP paper: 60초.
- 일반 paper: 30초.
- Limited live: 30초.
- 장 시작/종료 근처: 필요하면 10~15초.
- 장 외 시간: 5분.

초기 구현 권장값:

- 장중 tick interval: 30초.
- 장 외 tick interval: 300초.
- Tick 최대 실행 시간: 10초.
- 이전 tick이 아직 실행 중이면 다음 tick은 skip한다.
- 한 tick에서 생성할 execution request는 symbol당 최대 1개로 제한한다.
- 초기 자동매매에서는 전체 broker write도 tick당 최대 1개로 제한한다.

### Tick 내부 순서

Tick 하나는 다음 단계로 실행한다.

1. `runtime_ticks`에 `tick_id`, `run_id`, `started_at`, mode를 저장한다.
2. Tick lease를 획득한다. 이전 tick이 실행 중이면 skip 처리한다.
3. Runtime mode, panic stop, shutdown signal을 확인한다.
4. Market session을 확인한다. KIS holiday/session 조회는 매 tick마다 하지 않고
   5분 TTL cache를 사용한다.
5. 장이 닫혀 있으면 신규 주문 판단은 skip한다. 다만 open order/fill 조회와
   reconciliation은 설정에 따라 계속 허용할 수 있다.
6. Account snapshot을 수집한다.
   - cash
   - positions
   - open orders
   - today fills
   - buying power는 주문 후보가 있을 때만 조회할 수 있다.
7. Watchlist quote snapshot을 수집한다.
8. Snapshot 수집 실패 시 tick을 `snapshot_failed`로 저장하고 종료한다.
9. Snapshot을 DB에 저장한다.
10. Strategy state를 갱신하고 strategy signal을 생성한다.
11. Decision input summary를 만든다.
12. Decision provider를 호출한다.
    - MVP는 `DeterministicDecisionProvider`부터 구현한다.
    - `LlmDecisionProvider`는 같은 trait 뒤에 나중에 붙인다.
13. Decision output schema를 검증한다.
14. Invalid decision은 저장 후 `decision_rejected`로 종료한다.
15. Risk policy를 실행한다.
16. Risk block이면 `RiskBlocked` event를 남기고 `risk_blocked`로 종료한다.
17. `hold`이면 execution 없이 `completed`로 종료한다.
18. Risk approved이면 `execution_requests` row를 만든다.
19. `ExecutionRequested` event를 남긴다.
20. `completed_at`, status, duration을 저장하고 tick을 종료한다.

Tick status는 최소 다음 값을 가진다.

- `started`
- `snapshot_collected`
- `signal_generated`
- `decision_proposed`
- `decision_rejected`
- `risk_evaluated`
- `risk_blocked`
- `execution_requested`
- `skipped_market_closed`
- `snapshot_failed`
- `completed`
- `failed`

### Execution workflow

Execution workflow는 risk-approved execution request만 처리한다.

1. Approved execution request를 DB에서 읽는다.
2. `ClientOrderId`를 생성한다.
3. Broker command journal을 reserve한다.
4. Submit 직전 final risk check를 다시 실행한다.
5. `TradingGateway`/`kis-client`를 통해 broker write를 submit한다.
6. 결과에 따라 execution event와 상태를 저장한다.

결과별 처리:

- Accepted:
  - `OrderAccepted` event를 남긴다.
  - Broker order reference를 저장한다.
- Rejected:
  - `OrderRejected` event를 남긴다.
  - Execution request를 terminal rejected로 표시한다.
- Ambiguous timeout 또는 unknown:
  - `ReconciliationNeeded` event를 남긴다.
  - Journal record를 unresolved live-blocking 상태로 유지한다.
  - Blind retry하지 않는다.

### Reconciliation workflow

Reconciliation workflow는 startup, ambiguous write, 운영자 명령, 장 마감 정리에서
명시적으로 실행한다.

1. Unresolved broker command를 DB에서 읽는다.
2. KIS order/fill inquiry를 조회한다.
3. Broker id, client id, symbol/side/quantity/time window 순서로 매칭한다.
4. Command state와 execution state를 갱신한다.
5. 결과 event를 남긴다.

Reconciliation 결과 event:

- `ReconciliationSucceeded`
- `ReconciliationStillUnknown`
- `ManualInterventionRequired`
- `OrderFilled`
- `OrderCancelled`
- `OrderAmended`

`ReconciliationStillUnknown` 또는 `ManualInterventionRequired`가 남아 있으면 live write는
계속 차단한다.

### Event로 남길 거래 상태 변화

모든 tick 세부 로그를 event로 만들지 않는다. 다음처럼 거래 상태에 의미 있는
변화만 event로 남긴다.

- `RiskBlocked`
- `ExecutionRequested`
- `OrderSubmitted`
- `OrderAccepted`
- `OrderRejected`
- `OrderFilled`
- `OrderCancelled`
- `OrderAmended`
- `ReconciliationNeeded`
- `ReconciliationSucceeded`
- `ReconciliationStillUnknown`
- `ManualInterventionRequired`
- `PanicStopActivated`
- `LiveWriteBlocked`
- `LiveResumed`

Tick마다 저장할 반복 데이터:

- `runtime_tick`
- `market_snapshot`
- `portfolio_snapshot`
- `strategy_signal`
- `decision_input`
- `decision_output`
- `risk_evaluation`

최종 모델:

```text
Tick loop = 관측, 판단, risk 승인, execution request 생성
Execution workflow = 실제 broker write 처리
Reconciliation workflow = 애매한 broker 결과 복구
Event log = 중요한 거래 상태 변화 기록
Snapshot tables = 반복적인 tick 데이터 저장
```

---

## 판단 계층

AI decision layer는 proposal만 생성한다.

입력은 작고 결정론적인 context여야 한다.

- strategy id/version
- latest signal
- symbol
- quote
- position
- 현금/주문가능금액
- open orders
- recent fills
- risk hints
- current mode

출력 schema:

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

허용하는 `decision_type` 값:

- `buy`
- `sell`
- `hold`

규칙:

- Invalid JSON은 저장 후 reject한다.
- Unknown symbol은 reject한다.
- Unsupported order type은 reject한다.
- AI output은 KIS를 직접 호출할 수 없다.
- AI output은 risk policy를 우회할 수 없다.
- Confidence만으로 execution을 허용하지 않는다.

---

## 전략과 백테스트

`.kis.yaml`은 외부 strategy artifact로 받아들인다.

V1에서 지원할 strategy subset:

- hold-only baseline
- SMA crossover
- RSI threshold
- momentum

Backtest integration:

- 공식 repo 전체를 vendoring하기보다 sidecar adapter boundary를 우선한다.
- KIS 공식 backtester MCP는 research/backtest run에 사용할 수 있다.
- Backtest request, parameters, metrics, report references를 DB에 저장한다.
- Promotion gate는 live sidecar state가 아니라 저장된 metrics를 읽는다.

필수 promotion metrics:

- total return
- CAGR
- max drawdown
- Sharpe ratio when available
- win rate
- trade count
- data warnings

---

## 리스크 정책

리스크 계층은 execution 전 최종 권한이다.

최소 정책 검사:

- Read-only mode에서는 write를 차단한다.
- Live write는 기본적으로 비활성화한다.
- Strategy가 active이고 approved인지 확인한다.
- Strategy가 필요한 backtest evidence를 보유하는지 확인한다.
- Live 전 paper 관찰 기간을 통과했는지 확인한다.
- Symbol이 allowlist에 있는지 확인한다.
- Account가 live allowlist에 있는지 확인한다.
- Confirmed account suffix가 일치하는지 확인한다.
- Market session이 open인지 확인한다.
- Panic stop 상태가 아닌지 확인한다.
- Unresolved broker command가 없는지 확인한다.
- Single-writer lock을 보유하는지 확인한다.
- Max order notional을 초과하지 않는지 확인한다.
- Max position notional을 초과하지 않는지 확인한다.
- Max daily order count를 초과하지 않는지 확인한다.
- Max daily loss를 초과하지 않는지 확인한다.
- Min signal strength를 충족하는지 확인한다.
- 설정된 경우 min AI confidence를 충족하는지 확인한다.
- Buying power가 충분한지 확인한다.
- Duplicate intent를 방지한다.
- 주문 간 cooldown을 준수한다.

KIS extension의 "signal strength below 0.5 skips order" 규칙은 AI prompt rule이
아니라 configurable risk policy가 되어야 한다.

---

## 실행 엔진

Execution은 risk-approved proposal만 소비한다.

규칙:

- Broker submit 전에 `ClientOrderId`를 생성한다.
- Broker submit 전에 execution request를 저장한다.
- Broker write 전에 broker command journal을 reserve한다.
- KIS transport를 직접 쓰지 않고 `TradingGateway`를 사용한다.
- Accepted broker response는 reference와 status를 저장한다.
- Rejected response는 terminal rejection으로 저장한다.
- Ambiguous response는 reconciliation-needed state로 저장한다.
- Write가 KIS에 도달했을 수 있으면 절대 blind retry하지 않는다.
- Reconciliation은 명시적 workflow다.

초기 order support:

- cash buy
- cash sell
- cancel
- amend

Follow-up write support는 KIS revise/cancel possible-quantity preflight를
사용해야 한다.

---

## 운영자 표면

`kis-client`는 CLI를 제공하지 않는다.

제품 CLI는 `trade-engine`에 둔다.

초기 commands:

- `trade-engine doctor`
- `trade-engine config validate`
- `trade-engine strategy validate <file>`
- `trade-engine strategy import <file>`
- `trade-engine run --dry-run --once`
- `trade-engine run --dry-run`
- `trade-engine decisions list`
- `trade-engine risk blocks`
- `trade-engine executions list`
- `trade-engine reconcile pending`
- `trade-engine portfolio show`
- `trade-engine report daily`
- `trade-engine panic-stop`
- `trade-engine resume-live`

규칙:

- CLI는 가능하면 DB에서 읽는다.
- CLI output은 secret과 전체 계좌번호를 redaction해야 한다.
- CLI는 숨은 source of truth가 아니다.
- Discord/reporting은 runtime private memory가 아니라 DB에서 읽는다.

---

## 구현 단계

### Phase 0. 기준선과 문서

목표:

- `PLAN_V3.md`를 채택한다.
- 구현 시작 후 README/AGENTS를 갱신한다.
- `kis-client`는 library-only로 유지한다.
- 현재 workspace에 `trade-engine`이 아직 없음을 명확히 한다.

수용 기준:

- `plans/PLAN_V3.md`가 존재한다.
- `cargo check --workspace --no-default-features`가 통과한다.
- 기존 `kis-client` tests가 계속 통과한다.

### Phase 1. `trade-engine` Crate 생성

목표:

- Workspace member `trade-engine`을 추가한다.
- 제품 오류 타입을 추가한다.
- Read-only MVP에 필요한 최소 module skeleton을 추가한다.
- `run --dry-run --once` CLI를 추가한다.
- Fake gateway 기반 잔고/현재가 로그 출력 테스트를 추가한다.

수용 기준:

- `cargo test -p trade-engine`이 통과한다.
- `cargo check --workspace --no-default-features`가 통과한다.
- KIS credentials가 필요하지 않다.
- Fake gateway로 cash, positions, watchlist quote가 structured log로 출력된다.
- 전체 계좌번호와 secret은 로그에 나오지 않는다.

### Phase 2. KIS Read-only 연결

목표:

- Env에서 KIS 인증 credential을 로드한다.
- `KisClientBuilder`로 paper KIS client를 만든다.
- 현재 잔고, 보유 종목, watchlist 현재가를 조회한다.
- 결과를 로그로 출력한다.

수용 기준:

- `trade-engine run --dry-run --once`가 DB-backed audit storage로 실행된다.
- Broker write method는 호출되지 않는다.
- Token cache path를 설정할 수 있다.
- Endpoint 실패는 단계별 context가 있는 error로 남긴다.

### Phase 3. Main DB와 Journal

목표:

- SQLite main DB를 추가한다.
- Migrations를 추가한다.
- `trade-engine`에 `BrokerCommandJournal` 구현체를 추가한다.

수용 기준:

- Migration contract tests가 통과한다.
- Duplicate `client_order_id`가 reject된다.
- Parent/child command integrity가 강제된다.
- Invalid state transition은 mutation 전에 실패한다.

### Phase 4. Trading Gateway

목표:

- `TradingGateway`를 정의한다.
- 결정론적인 `MockGateway`를 추가한다.
- `kis-client::BrokerClient` 위에 `KisGateway` adapter를 추가한다.

수용 기준:

- Gateway tests가 read, write, error mapping을 커버한다.
- 런타임 module은 `TradingGateway`에 의존한다.
- 런타임 module은 KIS transport를 직접 호출하지 않는다.

### Phase 5. Config와 시작 Doctor

목표:

- TOML에서 config를 로드한다.
- Env에서 secret을 로드한다.
- Config로 KIS client를 만든다.
- 시작 doctor를 실행한다.

수용 기준:

- 누락된 secret은 조치 가능한 error를 반환한다.
- Live config는 safety field가 빠졌을 때 fail closed한다.
- 시작 recovery는 unresolved command가 있으면 live write를 차단한다.

### Phase 6. Snapshot Collection

목표:

- Read-only market/account snapshot collection을 구현한다.
- Snapshots를 저장한다.

수용 기준:

- `run --dry-run --once`가 quote, cash, positions를 저장한다.
- Open orders와 fills 저장은 execution/reconciliation 단계에서 확장한다.
- Fake gateway tests는 deterministic하다.
- Read-only mode에서는 broker write가 발생하지 않는다.

### Phase 7. Strategy Spec과 Signals

목표:

- `.kis.yaml`을 parse한다.
- Internal `StrategySpec`으로 normalize한다.
- 초기 deterministic signals를 생성한다.

수용 기준:

- Example YAML이 validate된다.
- Invalid YAML은 구체적 error로 실패한다.
- Signal generation은 저장된 snapshots에서 replay 가능하다.

### Phase 8. Decision Proposal Layer

목표:

- Decision input builder를 추가한다.
- Decision output schema를 추가한다.
- Test용 fake decision provider를 추가한다.
- Hard dependency 없이 optional LLM/provider boundary를 추가한다.

수용 기준:

- Malformed output은 저장 후 reject된다.
- `hold`는 항상 valid하다.
- `buy`/`sell`은 risk approval 없이는 executable이 될 수 없다.

### Phase 9. Risk Engine

목표:

- Configurable risk policy를 추가한다.
- Risk evaluations를 저장한다.

수용 기준:

- 모든 block은 durable reason을 가진다.
- Live writes는 기본적으로 block된다.
- Unresolved broker command는 새 live write를 block한다.
- 리스크 테스트는 주문 크기, 노출, 장 상태, 중복 의도, 주문 간 대기 시간,
  주문가능금액을 커버한다.

### Phase 10. 실행 엔진

목표:

- Approved risk result를 broker command로 변환한다.
- `TradingGateway`를 통해 submit한다.
- Execution event를 저장한다.

수용 기준:

- Accepted, rejected, ambiguous outcome이 테스트된다.
- Ambiguous write는 reconciliation path로 들어간다.
- Broker write가 도달했을 수 있는 이후에는 blind retry가 없다.

### Phase 11. Paper 런타임 루프

목표:

- 지속 실행되는 paper loop를 구현한다.
- Graceful shutdown을 추가한다.
- Tick audit을 추가한다.

수용 기준:

- Paper loop는 fake gateway로 반복 실행될 수 있다.
- Dry-run/read-only modes는 write를 submit하지 않는다.
- 런타임 재시작은 DB state에서 resume한다.

### Phase 12. 운영자 CLI와 Reporting

목표:

- 운영 command와 daily report를 추가한다.

수용 기준:

- CLI output은 secret을 redaction한다.
- `panic-stop`은 writes를 block한다.
- `resume-live`는 unresolved journal records를 clear하지 않는다.
- Reports는 DB state에서 생성된다.

### Phase 13. 인증 Gate

목표:

- Paper와 limited-live 인증 workflow를 추가한다.

수용 기준:

- Paper read-only smoke checklist가 존재한다.
- Paper controlled-order checklist가 존재한다.
- Live read-only smoke checklist가 존재한다.
- Live write는 explicit promotion과 operator approval을 요구한다.

---

## 인증 순서

Fake-server 완료는 live readiness가 아니다.

필수 순서:

1. Unit test와 integration test.
2. Fake gateway runtime loop test.
3. KIS fake-server adapter test.
4. Paper read-only smoke.
5. Write 없는 dry-run loop.
6. Explicit approval이 있는 작은 controlled paper order.
7. Paper cancel/amend와 reconciliation drill.
8. Live read-only quote/balance smoke.
9. Seeded unresolved command가 있는 live startup recovery drill.
10. Promotion gate approval 이후 limited live write.

---

## 안전 불변식

- AI decision output은 그 자체로 order가 아니다.
- 제품 리스크 정책이 최종 권한을 가진다.
- Live writes는 기본적으로 disabled다.
- Live broker write 전에 broker command가 durable해야 한다.
- 동일 live account에는 single writer만 존재한다.
- Unresolved reconciliation은 새 live write를 block한다.
- Ambiguous broker write outcome은 blind retry하지 않는다.
- Secret은 logs, fixtures, DB audit output, CLI, Discord, reports에 노출되지
  않는다.
- 전체 계좌번호는 approved suffix display를 제외하고 redaction한다.
- Backtest success만으로 live trading을 승인하지 않는다.
- MCP와 AI plugins는 `trade-engine` execution path를 우회할 수 없다.
- Panic stop은 명시적으로 clear될 때까지 writes를 block한다.

---

## Read-only MVP 완료 기준

Read-only MVP는 다음 조건을 만족할 때 완료로 본다.

- `trade-engine`이 workspace member로 존재한다.
- `trade-engine run --dry-run --once`가 존재한다.
- Live 환경 KIS 인증을 수행할 수 있다.
- 현재 현금 잔고를 가져와 로그로 출력한다.
- 현재 보유 종목과 수량을 가져와 로그로 출력한다.
- 설정한 watchlist 종목들의 현재가를 가져와 로그로 출력한다.
- Secret, access token, 전체 계좌번호는 로그에 나오지 않는다.
- DB 없이 실행 가능하다.
- Broker write method는 호출되지 않는다.
- Fake gateway 기반 테스트가 통과한다.

## V3 자동매매 MVP 완료 기준

V3 자동매매 MVP는 다음 조건을 만족할 때 완료로 본다.

- `trade-engine`이 workspace member로 존재한다.
- `kis-client`는 library-only로 유지된다.
- `trade-engine`이 SQLite migrations와 DB-backed journal implementation을
  소유한다.
- `TradingGateway`, `MockGateway`, `KisGateway`가 존재한다.
- `run --dry-run --once`가 market/account snapshots를 저장한다.
- Strategy signal, decision proposal, risk evaluation, execution request가
  모두 저장된다.
- Dry-run loop가 fake gateway와 KIS read endpoints에서 동작한다.
- Ambiguous broker outcome은 reconciliation workflow로 들어간다.
- CLI가 decisions, risk blocks, executions, pending reconciliation을 조회할
  수 있다.
- 전체 workspace verification이 통과한다.

```sh
cargo check --workspace --no-default-features
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo doc --no-deps --all-features
```
