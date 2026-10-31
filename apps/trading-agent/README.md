# trading-agent

한국투자증권(KIS)을 우선 지원하고 Toss Invest Open API read-only 경로를
첫 증분으로 지원하는 Rust 기반 트레이딩 에이전트 인프라입니다.

목표는 단순 브로커 래퍼가 아닙니다. 브로커 연동, 시장/계좌 조회, 실행,
저장, 감사 로그는 결정적으로 처리하고, 에이전트는 구조화된 매수/매도/보유
제안만 생성합니다. 최종 실행 권한은 리스크/정책 계층이 가집니다.

현재 제품 방향의 기준 문서는 [plans/PLAN_V4.md](plans/PLAN_V4.md)입니다.
[plans/PLAN_V3.md](plans/PLAN_V3.md),
[plans/PLAN_V2.md](plans/PLAN_V2.md)와
[plans/INITIAL_PLAN.md](plans/INITIAL_PLAN.md)는 과거 계획입니다.
[plans/KIS_CLIENT_IMPLEMENTATION_PLAN.md](plans/KIS_CLIENT_IMPLEMENTATION_PLAN.md)는
KIS 어댑터 컴포넌트 계획으로만 사용합니다.

## 구조

- `trade-engine`: 제품 런타임 crate입니다. gateway, runtime, storage,
  strategy, decision, risk 흐름을 담당합니다.
- `kis-client`: KIS 브로커 클라이언트 crate입니다. 인증, 전송, KIS API
  계약, 매핑, 주문 안전장치, reconciliation을 담당합니다.
- `toss-client`: Toss Invest Open API 클라이언트 crate입니다. 현재는
  read-only 계좌/시세/시장일/상세 시장 데이터 조회와 주문 write fail-closed
  stub을 담당합니다.
- `configs/`: live dry-run 실행 예시 설정입니다.
- `plans/`: 제품 및 KIS 클라이언트 설계 문서입니다.

중요한 경계:

- 제품 런타임에서는 `broker`가 아니라 `gateway` 용어를 사용합니다.
- 전략, 에이전트 판단, 리스크 정책은 `kis-client`에 넣지 않습니다.
- 실행 계층은 구체 KIS 전송 세부가 아니라 `TradingGateway`에 의존해야 합니다.
- 에이전트는 주문을 직접 제출할 수 없습니다.

## 현재 상태

구현됨:

- Cargo workspace: `kis-client`, `toss-client`, `trade-engine`
- KIS 인증, 토큰 캐시, redaction, HTTP transport, request audit 기반
- 국내주식 quote, balance, cash, buying-power 조회
- 해외주식 quote, balance, buying-power 조회
- 국내/해외 주문 계약, 정정/취소, 조회, reconciliation 기반
- durable command journal 계약과 product-owned SQLite journal 구현
- 단일 writer guardrail, read-only/live 설정 guardrail
- `trade-engine doctor` startup doctor
- `trade-engine run --dry-run --once`
- `trade-engine run --dry-run`
- `trade-engine run --submit-orders` guarded order-submission loop
- `trade-engine backtest` stored daily OHLCV 기반 SMA baseline sidecar
- `trade-engine status`, `trade-engine review`, `trade-engine query --prompt`
  read-only feedback/review CLI
- V4 TOML config surface: KIS paper/live environment, main DB path,
  built-in strategy config, command advisor proposal config, fail-closed risk/live guard config
- Job-level domestic/overseas stock market selection for read-only dry-run configs
- SQLite 기반 dry-run snapshot 및 decision audit 일부
- 내장 `sma-crossover-v1` 전략을 이용한 write-free decision pipeline
- 일일 OHLCV bar 계약, deterministic technical feature 계산(SMA/EMA/ATR/RSI/ROC/volume),
  append-only `ohlcv_bars`/`technical_feature_audits` 감사 저장소, advisor/runtime
  audited technical feature context
- Codex CLI command provider를 이용한 market tick 내 advisory 감사 기록
- deterministic 수량 산정과 stop-loss/take-profit exit policy layer
- Toss Invest Open API read-only gateway/client: OAuth2 token issuance, accounts,
  holdings/cash/buying-power, quote, KR/US market calendar, open orders, stock
  info/warnings, candles/trades/orderbook/price limits

아직 미완성:

- runtime에 통합된 전체 market feature layer
- 외부 strategy spec parser/validator
- persisted backtest audit/promotion harness
- 실제 agent decision layer
- 완성된 risk/policy engine
- 완성된 operator/reconciliation surface
- Discord reporting과 완성된 운영 dashboard

## 안전 원칙

이 저장소는 아직 실거래에 안전하지 않습니다.

Live write는 최소한 다음 조건을 모두 만족해야 합니다.

- 명시적 live enablement
- `read_only = false`
- 계좌 suffix 확인
- live 계좌 allowlist 통과
- 브로커 write 전 durable journal 기록
- 같은 live account에 대한 단일 writer
- 시장 session/open-day 확인
- 애매한 write 결과 이후 blind retry 금지
- unresolved journal record reconciliation
- panic stop 통과

불확실할 때는 편의보다 중복 주문 방지를 우선합니다.

## 빠른 실행

전체 테스트:

```sh
cargo test --all-features
```

포맷 확인:

```sh
cargo fmt --check
```

Clippy:

```sh
cargo clippy --all-targets --all-features -- -D warnings
```

전체 검증 세트:

```sh
cargo check --workspace --no-default-features
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo doc --no-deps --all-features
```

KIS 연동 실행에는 앱 credential 환경 변수가 필요합니다. 계좌 키는 기본적으로
`KIS_ACCOUNT_KEY` 환경 변수에서 읽지만, 개인용 untracked config에서는
`[kis].account_key`로 직접 지정할 수 있습니다.

```sh
export KIS_APP_KEY=...
export KIS_APP_SECRET=...
export KIS_ACCOUNT_KEY=12345678-01
```

```toml
[kis]
account_key = "12345678-01"
```

Toss Open API read-only 실행에는 Toss credential과 계좌 일련번호를 환경 변수로
제공합니다. 공유 config에는 값 대신 env var 이름만 둡니다.

```sh
export TOSS_CLIENT_ID=...
export TOSS_CLIENT_SECRET=...
export TOSS_ACCOUNT_SEQ=7
```

위 환경 변수를 로컬 `.envrc`에서 `export`하는 경우에는 `direnv exec`로 실행합니다. Rust client는 전달받은 환경 변수를 읽으며 `.envrc` 자체를 실행하거나 파싱하지 않습니다.

```sh
direnv exec . cargo run -p trade-engine -- doctor --config configs/trade-engine.toss-dry-run.toml
```

`TOSS_ACCOUNT_SEQ`에는 `GET /api/v1/accounts` 응답에서 확인한 `accountSeq`를 설정합니다. Client ID와 secret만으로는 계좌 조회를 시작할 수 없습니다. `.envrc`는 Git에서 제외됩니다.

```toml
[broker]
provider = "toss"

[toss]
client_id_env = "TOSS_CLIENT_ID"
client_secret_env = "TOSS_CLIENT_SECRET"
account_seq_env = "TOSS_ACCOUNT_SEQ"
```

Config 파일에는 secret 값을 넣지 않습니다. 실제 계좌 키, Toss account sequence,
credential 값을 공개 문서, 공유 config, commit에 넣지 말고 개인용 로컬 config나
환경 변수에만 둡니다. `kis.environment`는 `paper` 또는
`live`로 명시하고, `market.kind`는 `domestic_stock` 또는 `overseas_stock`으로
반드시 job 단위 시장을 선택합니다. `watchlist` symbol도 선택한 시장 스코프와
맞아야 합니다. 해외주식 job은 `market.exchange_code`와 `market.currency`가
필요합니다. 해외주식 KIS 예수금 조회에 사용하는 기준 종목은 내부 기본값
`AAPL`을 사용합니다. 해외주식 live write의 주문 가능 세션은 기본적으로
`regular`만 허용하며, 프리마켓/애프터마켓/미국주간주문을 켜려면 `[market]`에
`allowed_order_sessions = ["regular", "premarket", "aftermarket", "daytime"]`처럼
명시합니다. 제공 해외주식 config는 이 네 세션을 모두 명시합니다.
`storage.main_db_path`는 `doctor`, `run --dry-run`,
`run --submit-orders`, `backtest` 모두에 필수입니다. 기본 config는 안정적인
기본값을 반복 기입하지 않습니다. 예를 들어 KIS env var 이름, deterministic
decision audit defaults, stdout/stderr caps, universe `max_symbols`/`refresh_seconds`, advisor
`recent_price_count`, dry-run sizing fraction, 기본 stop/take-profit pct는 코드
default를 사용하며, 운영상 바꿀 이유가 있을 때만 TOML에서 override합니다.

제공 config는 다음 실행 목적에 맞춰 고릅니다.

| 목적 | Config |
| --- | --- |
| 국내 read-only 점검/드라이런 | `configs/trade-engine.live-dry-run.toml` |
| 국내 live 주문 제출 | `configs/trade-engine.live-submit-orders.toml` |
| 해외주식 read-only 점검/드라이런 | `configs/trade-engine.overseas-dry-run.toml` |
| 해외주식 live 주문 제출 | `configs/trade-engine.overseas-submit-orders.toml` |
| Toss 국내 read-only 점검/드라이런 | `configs/trade-engine.toss-dry-run.toml` |

### Domestic Market Alignment

Domestic market alignment is disabled by default. When enabled, the runtime can
compute domestic benchmark-relative context from stored daily technical feature
audits. In `advisor_context` mode, the context is included in the structured
advisor request and recorded in runtime tick detail JSON. It does not change
order sizing, risk blocks, or execution behavior.

```toml
[domestic_market_alignment]
enabled = true
mode = "advisor_context" # audit_only | advisor_context
benchmark_symbol = "KOSPI"
feature_set = "daily-core-v1"
relative_strength_lookback_days = 20
```

Live write는 `read_only = false`만으로 켜지지 않으며,
`risk.enable_live_trading`, resolved KIS account key 기반 account scope,
single-writer lock, panic stop 해제, unresolved broker command 없음 같은 별도
guard가 모두 필요합니다.
Toss provider는 현재 read-only 전용입니다. `run --submit-orders`는 config 검증에서
fail-closed로 차단되며, Toss 주문 생성/정정/취소 HTTP 호출은 활성화되어 있지 않습니다.

CLI stdout은 기본적으로 `--output pretty` 형식으로 runtime block을 표 형태로
렌더링하고, `Positions` 섹션은 `Decisions` 뒤 마지막에
symbol/quantity/price/return 컬럼으로 정리합니다.
색상은 `--color auto|always|never`로 제어합니다. 스크립트나 로그 파서처럼 기존 평문
block이 필요하면 전역 옵션 `--output plain`을 사용합니다.
예를 들어:

```sh
cargo run -p trade-engine -- --color auto run --dry-run --once --config configs/trade-engine.live-dry-run.toml
```

먼저 선택한 config로 startup doctor를 실행합니다.

```sh
cargo run -p trade-engine -- doctor --config configs/trade-engine.live-dry-run.toml
```

해외주식 config도 같은 방식으로 doctor를 실행합니다.

```sh
cargo run -p trade-engine -- doctor --config configs/trade-engine.overseas-dry-run.toml
```

Toss Open API read-only config도 같은 방식으로 doctor를 실행합니다.

```sh
cargo run -p trade-engine -- doctor --config configs/trade-engine.toss-dry-run.toml
```

선택적으로 universe refresh만 즉시 한 번 실행해 DB cache를 채울 수 있습니다.

```sh
cargo run -p trade-engine -- universe --config configs/trade-engine.live-dry-run.toml
cargo run -p trade-engine -- universe --config configs/trade-engine.overseas-dry-run.toml
```

저장된 daily OHLCV bar로 SMA crossover baseline backtest를 실행합니다. 기본 symbol은
config의 `watchlist`이며, `--symbols 005930,000660`처럼 좁힐 수 있습니다.

```sh
cargo run -p trade-engine -- backtest --config configs/trade-engine.live-dry-run.toml --from 2026-01-01 --to 2026-03-31
```

Advisor, universe discovery, feedback의 command provider는 stdin 전달부터 프로세스 종료와 stdout/stderr 수집까지 하나의 timeout을 적용합니다. 입출력은 동시에 처리하고 설정된 크기까지만 메모리에 보관합니다. Timeout이나 호출 취소 시 자식 프로세스를 종료하며, Unix에서는 같은 프로세스 그룹의 자손도 정리합니다. 입력을 먼저 닫은 command는 종료 상태와 출력 검증으로 결과를 판단합니다.

### Feedback and review CLI

`trade-engine status` prints a read-only Main DB and runtime audit summary.

`trade-engine review --since-hours 24 --limit 20` prints recent universe,
advisor, decision, and risk audit context without invoking Codex.

`trade-engine query -p "..." --since-hours 24 --limit 20` builds the same
bounded audit context and sends it to `trade-engine/scripts/codex-feedback`,
which invokes `codex exec` in read-only sandbox mode. The result is a review
answer only: it cannot submit broker orders and it cannot apply policy/config
changes.

```sh
cargo run -p trade-engine -- status --config configs/trade-engine.live-dry-run.toml
cargo run -p trade-engine -- review --since-hours 24 --limit 20 --config configs/trade-engine.live-dry-run.toml
cargo run -p trade-engine -- query -p "summarize recent risk blocks" --config configs/trade-engine.live-dry-run.toml
```

국내 read-only dry-run을 한 tick만 실행합니다.

```sh
cargo run -p trade-engine -- run --dry-run --once --config configs/trade-engine.live-dry-run.toml
```

국내 read-only dry-run loop를 실행합니다.

```sh
cargo run -p trade-engine -- run --dry-run --config configs/trade-engine.live-dry-run.toml
```

해외주식 read-only dry-run을 한 tick만 실행합니다.

```sh
cargo run -p trade-engine -- run --dry-run --once --config configs/trade-engine.overseas-dry-run.toml
```

Toss read-only dry-run을 한 tick만 실행합니다.

```sh
cargo run -p trade-engine -- run --dry-run --once --config configs/trade-engine.toss-dry-run.toml
```

국내 live 주문 제출 전에 짧은 submit smoke loop를 먼저 실행합니다.

```sh
cargo run -p trade-engine -- run --submit-orders --config configs/trade-engine.live-submit-orders.toml --max-market-ticks 1
```

해외주식 live 주문 제출 전에도 별도 해외 config로 smoke loop를 먼저 실행합니다.

```sh
cargo run -p trade-engine -- run --submit-orders --config configs/trade-engine.overseas-submit-orders.toml --max-market-ticks 1
```

Smoke loop 결과를 확인한 뒤 국내 submit loop를 실행합니다.

```sh
cargo run -p trade-engine -- run --submit-orders --config configs/trade-engine.live-submit-orders.toml
```

해외주식 submit loop는 해외주식 config로 실행합니다.

```sh
cargo run -p trade-engine -- run --submit-orders --config configs/trade-engine.overseas-submit-orders.toml
```

기본 제공 submit config는 KIS live 환경용입니다. 공유 config에는 계정값을 쓰지
않습니다. Submit runtime은 resolved KIS account key에서 live account scope를 파생하고,
동일 account와 market scope에 대한 single-writer lock을 사용합니다. 국내주식과
해외주식 submit loop는 서로 다른 lock을 잡으므로 동시에 실행할 수 있고, 같은
market scope의 중복 writer는 계속 차단됩니다. KIS HTTP 호출은 account와 무관하게
동일 app key의 shared SQLite rate limiter를 통과하므로, 국내/해외 프로세스가 동시에
떠도 API 호출 slot과 throttle circuit을 공유합니다. Submit config는
`strategy.id = "hold-only"`로 SMA fallback 매매 신호를 끄고, `advisor` command
provider의 structured proposals만 active decision source로 사용합니다.

Dry-run은 선택한 broker의 read-only gateway로 계좌/시세를 읽고, 내장 전략으로
signal/decision/risk row를 저장하지만 브로커 주문은 제출하지 않습니다.
해외주식은 별도 config/job으로 실행하며, KIS 해외주식 시세/잔고/매수가능금액/
미체결/체결/지정가 주문/정정/취소/reconciliation 경로를 사용합니다. Live
해외 broker write는 코드 기본값으로 `NASD`/`NYSE`/`AMEX` 정규장만 허용합니다.
`allowed_order_sessions`에 명시한 경우 프리마켓, 애프터마켓, 미국주간주문도
KIS `countries-holiday` 조회와 세션별 시간 gate가 통과해야 broker write로
넘어갑니다. 명시된 해외주식 주문 세션이 열려 있으면 runtime loop도
`runtime.market_tick_seconds`를 사용합니다. 명시되지 않은 세션과 그 외 해외
거래소 live write는 계속 fail-closed입니다.
Toss read-only gateway/client는 OAuth2 token issuance, account snapshot,
holdings, cash/buying-power, single-symbol quote, KR/US market calendar, open
orders를 조회합니다. `toss-client`는 추가로 stock info/warnings,
candles/trades/orderbook/price limits 조회 메서드를 제공합니다. 이 상세 시장
데이터는 아직 runtime decision loop에 자동 연결하지 않았습니다. Toss 주문 write와
reconciliation은 safety halt로 닫혀 있고, fills는 첫 증분에서 빈 결과로
처리합니다.
`backtest`는 저장된 `ohlcv_bars`만 읽는 sidecar입니다. Startup doctor, KIS gateway,
계좌/시세 조회, broker write 경로를 호출하지 않습니다. 현재는 `sma-crossover-v1`,
daily interval, 같은 날 종가 체결, 수수료/슬리피지/세금/부분체결 없음이라는 보수적으로
문서화된 첫 baseline만 지원하며 결과는 stdout에 표시하고 `backtest_runs` 같은 별도
audit table에는 아직 저장하지 않습니다.

`run --dry-run`은 runtime tick/loop를 시작하기 전에 startup doctor를 실행해
config, main DB migration, unresolved broker command journal, broker ping,
계좌 snapshot, watchlist quote, market session을 확인합니다. 현재 V4 기준 CLI는
`run`, `doctor`, `universe`, `backtest`를 유지합니다. Runtime loop는 market session을 TTL 동안
캐시합니다. 장이 닫힌 tick은 command advisor를 호출하지 않고 market-closed
advisory audit을 남기며, write 의도는 market-closed risk gate로 차단합니다.
KIS rate limit 같은 retryable gateway read 실패는 해당 tick을 `snapshot_failed`로
감사한 뒤 다음 tick에서 자동 재시도합니다.
각 tick 출력의 `Positions` 섹션은 보유 수량, 평균가, 현재가, 미실현 수익률을 표시합니다.
보유 포지션은 watchlist 밖에 있어도 이 표시와 audit snapshot을 위해 별도 quote를 조회합니다.
Ctrl-C로 종료하면 현재 tick 경계에서 run을 `interrupted`로 기록합니다.

`run --submit-orders`는 같은 tick pipeline을 실행한 뒤, 현재 runtime run에서 새로
생성된 risk-approved execution request만 submit 대상으로 삼습니다. Broker write
전에는 DB execution request와 broker command journal record를 먼저 durable하게
남기고, submit 직전 final risk check를 다시 실행합니다. KIS write-enabled gateway는
DB-backed journal과 single-writer lock을 주입받으며, live 환경에서는 confirmed account,
allowlist, unresolved journal, panic stop guard가 통과해야 합니다.
Daily loss risk는 오늘 sell fill의 원가를 현재 포지션에서 먼저 찾고, 전량 매도되어
현재 포지션에서 사라진 심볼은 Main DB의 과거 portfolio snapshot 평균단가로 보강해
계산합니다. 원가 근거가 어디에도 없으면 계속 fail-closed로 차단합니다.
해외주식 limit proposal은 KIS가 거절하지 않도록 1달러 이상 가격을 cent 단위로
정규화합니다. 매수 가격은 올림, 매도 가격은 내림으로 처리해 limit 의도를 보존합니다.

`tick_advisory`는 각 market tick 안에서 실행됩니다. `[advisor]` command provider가
켜진 경우 `runtime`과 `current_state.current_tick.kis_api` JSON을 외부 command에
전달하고, 결과를 `operator_actions`에 `tick_advisory_completed` 또는
`tick_advisory_failed`로 남깁니다. 입력의 quote에는 KIS에서 확인한 종목 full name과
recent prices가 포함됩니다. 기본 live submit-orders config는
`sh trade-engine/scripts/codex-advisor`를 호출하며, 이 wrapper는 `codex exec`를
read-only sandbox와 JSON schema 출력으로 실행합니다. Advisor 출력은
`summary`/`observations`/`risks`/`next_actions`, audit용 `rationale`/`evidence`,
그리고 risk gate로 넘길 structured `proposals`를 포함합니다. `run` 출력에는
audit 위치와 `summary`/`risks`/`next_actions` 요약만 표시하고, 전체
request/output/evidence/proposals는 DB audit에 남깁니다. Advisor는 broker write를
직접 만들지 않고, 제안은 deterministic validation과 risk gate를 거칩니다. Codex
advisor를 쓰는 live config는 외부 command 호출 시간을 포함하도록
`max_tick_runtime_seconds`를 더 길게 둡니다. Config validation은 command advisor
timeout보다 tick runtime이 최소 30초 이상 길어야 통과합니다.
제공 config는 command advisor timeout을 180초, tick runtime timeout을 240초로
명시해 이 조건을 만족합니다.

`[sizing]`을 켜면 advisor의 buy proposal 수량을 엔진이 다시 산정합니다.
현재 계좌 현금과 quote로 평가한 보유 포지션을 더해 account value를 추정하고,
`target_account_fraction`, `risk.max_order_notional`, 남은
`risk.max_position_notional`, 가용 현금 중 가장 작은 notional을 기준으로 정수
주식 수량을 내림합니다. `[exit_policy]`를 켜면 보유 종목의 현재가가
`average_price * (1 - stop_loss_pct)` 이하이거나
`average_price * (1 + take_profit_pct)` 이상일 때 전량 지정가 sell proposal을
엔진이 추가합니다. 같은 tick에서 exit가 발생한 symbol의 advisor proposal은
충돌 주문을 막기 위해 제외되고, 생성된 exit proposal도 기존 validation/risk/final
execution gate를 그대로 통과해야 합니다. 기본 dry-run config는
`target_account_fraction = "0.10"`을 사용하고, live submit-orders config는
`target_account_fraction = "0.40"`을 사용합니다. 기본 exit policy는
`stop_loss_pct = "0.05"`, `take_profit_pct = "0.10"`입니다. Notional 값은 해당
job의 계좌 통화 기준이며, 해외주식 USD config에서는 USD 기준입니다.

`[universe_discovery]` command provider를 켜면 tick 초반에 사용자 `watchlist`를
seed로 외부 command를 호출해 추가로 관찰할 후보 symbol만 받습니다. 기본 config는
`sh trade-engine/scripts/codex-universe`를 사용합니다. Submit-orders loop는 시작할 때
universe refresh를 best-effort로 한 번 실행하고, 이후 background refresh task가
`universe_snapshots` DB cache를 갱신합니다. Runtime tick 자체는 `codex-universe`를
직접 실행하지 않고 매 tick마다 DB의 유효한 snapshot을 읽어
`watchlist + cached discovered_universe`를 effective candidates로 합칩니다. 같은 후보
집합은 advisor/risk/final execution gate에도 전달됩니다. 유효한 snapshot이 없거나
refresh가 실패하면 tick은 실패하지 않고 기존 watchlist만 사용해 계속 진행합니다.
`codex-universe` wrapper는 `watchlist`를 경계가 아니라 seed로 취급합니다. Runtime
request에는 시장 스코프가 포함되며, 한국 시장에서는 KRX 6자리 코드를, 해외주식
시장에서는 설정된 거래소/통화에 맞는 미국 ticker를 우선합니다. 해외주식
universe refresh는 KRX 6자리 후보를 걸러내므로 해외 quote 단계에 국내 코드가
섞이지 않습니다. `liquidity`, `valuation`,
`short_term_momentum`, `catalyst`, `watchlist_peer`, `defensive_rotation`,
`etf_or_proxy` bucket을 독립적으로 훑은 뒤 audit 가능한 후보를 8-20개 수준으로
압축하도록 prompt되어 있습니다. 후보에는 `bucket`, 구체적인 `signals`, 짧은
`evidence`(`metric`/`value`/`as_of`/`source`), 그리고 `risk_flags`가 함께
저장됩니다. 최신 거래량, PER, 단기 모멘텀 근거가 불확실한 경우에는 낮은
confidence와 `stale_data`, `unverified_source`, `news_unverified`,
`data_unavailable` 같은 flag를 남겨야 하며, universe discovery 자체는 여전히
매수/매도/수량/가격을 제안하지 않습니다. Advisor request에는 기존
`runtime.discovered_universe`와 `runtime.effective_candidates` symbol 배열을 유지하면서
`runtime.market`의 시장/거래소/통화 context와
`runtime.universe_discovery.discovered_candidates`의 bucket, signal, evidence, risk flag가
함께 전달됩니다. `runtime.universe_discovery.source`에는 cache snapshot id, 원 provider,
observed/expires timestamp, summary, 실패 사유 같은 freshness context가 들어갑니다.
Advisor input은 같은 symbol 배열을 nested discovery context에 반복하지 않는 compact
contract를 사용합니다.
Advisor는 이 discovery metadata를 관찰 우선순위와 근거로 사용할 수 있지만, 주문 제안은
여전히 현재 tick의 KIS quote/account/order/fill 관측값과 risk gate를 통과해야 합니다.
차트 패턴, breakout, support/resistance, 이동평균, oscillator, ATR/RSI/MACD/ROC,
volume confirmation 같은 기술적 분석 주장은 deterministic technical feature output이나
직접 관측된 quote/recent-price 근거를 인용할 때만 확정적으로 다룰 수 있습니다.
저장된 feature audit이 있으면 runtime은 최신 `daily-core-v1` daily evidence를
`current_state.current_tick.technical_analysis`에 포함하고, `run` 출력의
`Technical analysis` 섹션에 `audit_id`, `interval`, `feature_set`, 핵심 feature 값을
요약합니다.
근거가 없으면 advisor/universe output은 이를 hypothesis 또는 unsupported claim으로 표시해야
하며, 매수/매도 제안은 계속 deterministic validation과 risk gate를 통과해야 합니다.
`[universe_discovery].refresh_seconds`는 snapshot TTL과 목표 refresh 주기입니다.
기본값과 제공 config는 10800초로, universe command를 저빈도로 호출하고 한 번의 refresh에서
더 깊게 검토한 후보 cache를 여러 tick에서 재사용하는 쪽을 기본 정책으로 둡니다.
제공 config는 `[universe_discovery].auto_refresh = true`로 두어 `run`/`submit-orders`
loop가 background universe refresh task를 함께 시작하게 합니다. 이 task는 loop 시작 직후
첫 refresh를 비동기로 실행하고, 이후 기존 cache가 만료되기 전에
`timeout_seconds + 30초` 여유를 두고 다음 refresh를 시작해 command 실행 시간으로 인한
cache 공백을 줄입니다. Runtime tick은 universe command 완료를 기다리지 않고 저장된 universe snapshot
cache를 읽으며, cache가 없거나 refresh가 실패하면 user watchlist로만 fail-closed
fallback합니다. 외부 universe command 실행을 main loop에서 분리하고 싶으면
`auto_refresh = false`로 두고 `universe --loop`나 one-shot `universe` command를 별도로
운용할 수 있습니다. `universe --loop`는 market pre-open window를 사용하고, one-shot
`universe` command는 수동 점검용으로 즉시 한 번 실행합니다.

```sh
cargo run -p trade-engine -- universe --loop --config configs/trade-engine.live-dry-run.toml
```

Overseas read-only universe refresh:

```sh
cargo run -p trade-engine -- universe --config configs/trade-engine.overseas-dry-run.toml
```

`[advisor].recent_price_count`는 command advisor에 전달할 symbol별 과거 가격
개수입니다. 기본값은 20이며, `strategy.id = "hold-only"`로 deterministic SMA 전략을
꺼도 Codex advisor에는 이전 tick 가격들이 함께 전달됩니다. Runtime은 이 값과 전략에
필요한 가격 개수 중 큰 값을 사용해
`current_state.current_tick.kis_api.quotes[].recent_prices`를 채웁니다.
제공 submit-orders config는 advisor 입력 크기를 줄이기 위해 `recent_price_count = 16`을
명시합니다.
`[advisor].candidate_limit`는 command advisor에 전달할 effective candidate 수의 상한입니다.
기본값은 8이고, 제공 submit-orders config는 10입니다. Runtime은 user watchlist를 우선 유지한 뒤 quote가 있는 discovered
candidate를 risk flag 수와 confidence 기준으로 정렬해 advisor 입력만 줄입니다. 전체
snapshot, universe audit, risk/final execution gate의 후보 범위는 이 필터로 축소하지 않습니다.
`[advisor].min_command_interval_seconds`는 command advisor 재호출의 기본 최소 간격이며
기본값은 600초입니다. Runtime tick은 계속 30초 기본값으로 돌 수 있고, advisor는 후보 목록,
계좌/주문/체결 상태, 또는 `[advisor].price_change_threshold_bps`(기본 75bp) 이상의 가격
변화가 있을 때 이 간격 안에서도 다시 호출됩니다. 제공 submit-orders config는 더 보수적으로
900초/100bp를 사용합니다. 간격 안에서 상태가 안정적이면 runtime이 `advisor_policy_skip`
audit과 hold proposal을 남기고 외부 advisor command는 실행하지 않습니다.

## 개발 규칙

- TDD를 기본으로 합니다. 실패하는 테스트를 먼저 만들고, 예상한 이유로 실패하는지
  확인한 뒤 구현합니다.
- KIS endpoint 작업은 공식 문서 또는 공식 샘플 동작을 먼저 확인합니다.
- endpoint별 method, path, header, `tr_id`, request/response, error mapping
  contract test를 둡니다.
- 주문 write endpoint는 journal, safety, ambiguous outcome, reconciliation
  테스트를 먼저 둡니다.
- 공개 경계에서는 `Symbol`, `AccountKey`, `ClientOrderId`, `RequestId` 같은
  typed wrapper를 사용합니다.
- 돈, 가격, 수량에는 `f64`를 쓰지 않고 `rust_decimal::Decimal`을 사용합니다.
- secret, token cache, account file, local DB, runtime log는 커밋하지 않습니다.

## 로컬 상태와 자격증명

현재 테스트 suite는 KIS 또는 Toss 자격증명을 요구하지 않습니다.

KIS smoke 실행에는 환경 변수와 앱이 관리하는 로컬 token cache가 필요합니다.
Runtime에서 사용하는 KIS client는 요청 직전에 token cache를 확인하므로 만료 margin에
들어온 access token은 자동 갱신됩니다.
`.gitignore`는 일반적인 secret, SQLite DB, log, Rust build output을 제외합니다.
그래도 커밋 전 diff에서 credential, token, 계좌 파일, 로컬 DB가 포함되지
않았는지 확인하세요.

## 참고 문서

- [AGENTS.md](AGENTS.md): 이 repo에서 Codex/agent가 따라야 하는 작업 규칙
- [plans/PLAN_V4.md](plans/PLAN_V4.md): 현재 제품 계획
- [plans/PLAN_V3.md](plans/PLAN_V3.md): 이전 제품 계획
- [plans/KIS_CLIENT_IMPLEMENTATION_PLAN.md](plans/KIS_CLIENT_IMPLEMENTATION_PLAN.md):
  KIS client 구현 계획
- [kis-client/README.md](kis-client/README.md): KIS client crate 설명
