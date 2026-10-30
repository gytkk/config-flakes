# trade-engine 아키텍처 리뷰: 레이어 분리와 Rust 구현 관점

검토일: 2026-04-28

검토 범위:

- `trade-engine/src/market.rs`
- `trade-engine/src/runtime/mod.rs`
- `trade-engine/src/execution/workflow.rs`
- `trade-engine/src/runner.rs`
- `trade-engine/src/doctor.rs`
- `trade-engine/src/config/mod.rs`
- `trade-engine/src/gateway/adapters/kis.rs`
- `kis-client/src/client/`

## 요약

전체 구조는 현재 방향이 맞다. `kis-client`는 KIS protocol, mapping, transport,
guardrail을 담당하고, `trade-engine`은 gateway adapter, runtime, decision, risk,
execution, storage를 나누어 가진다. Product runtime 대부분은 `TradingGateway`와
journal abstraction 뒤에서 동작하며, product DB-backed journal이 `kis-client`로
새지 않는 점도 좋다.

Rust 구현 관점에서도 큰 문제는 보이지 않는다. typed domain model, `Decimal` 사용,
명시적인 error type, async gateway/journal boundary, live write 전 durable journal
flow가 잘 잡혀 있다.

다만 실거래 운용과 장기 유지보수를 생각하면 몇몇 경계가 조금씩 섞이기 시작했다.
우선순위는 final execution risk의 local hard-stop 순서 정리와 runtime module의
production DB/journal 의존성 분리다.

## Findings

### 1. Final execution risk가 local hard-stop 전에 broker read를 먼저 수행한다

심각도: Medium

관련 코드:

- `trade-engine/src/execution/workflow.rs:859`
- `trade-engine/src/execution/workflow.rs:887`

`build_policy_final_execution_risk_gate`는 buying power, account snapshot, market
session, fills를 먼저 조회한 뒤 unresolved broker command와 panic stop 같은 DB 상태를
`RiskEvaluationContext`에 넣는다.

주문 제출 전에는 final risk가 막기 때문에 money-moving 버그는 아니다. 하지만 panic
stop이나 unresolved journal이 있는 상태에서도 KIS read가 발생한다. Live 운용에서는
로컬 hard stop을 먼저 확인하고, 그 이후에 필요한 broker read를 수행하는 편이 더
안전하고 조용하다.

권장:

- `build_policy_final_execution_risk_gate` 초반에 local DB hard-stop state를 먼저 읽는다.
- panic stop이나 unresolved command만으로 차단이 확정되면 broker read를 생략한다.
- broker read가 필요한 risk rule은 local hard-stop 통과 이후에만 평가한다.

처리 상태: 완료

- 수정: `build_policy_final_execution_risk_gate`가 unresolved broker command와 panic
  stop을 먼저 조회하고, 둘 중 하나가 active이면 broker read 없이 final risk gate를
  구성한다.
- 테스트: unresolved journal과 panic stop 상태에서 buying power, account snapshot,
  market session, fills, place order 호출이 모두 0회임을 검증했다.
- 셀프 리뷰: 변경 범위는 final execution risk gate 생성 순서와 해당 테스트에 한정했다.
  Daily loss, buying power, market session처럼 broker state가 필요한 risk rule은 기존
  경로를 유지한다.

### 2. Runtime core 안에 production DB/journal 구성이 들어와 있다

심각도: Medium

관련 코드:

- `trade-engine/src/runtime/mod.rs:560`
- `trade-engine/src/runtime/mod.rs:608`

`RuntimeTickExecutionDriver` 추상화는 좋다. 하지만 `DbRuntimeExecutionDriver`가
`MainDb`와 `SqliteBrokerCommandJournal`을 직접 열면서 runtime module이 concrete
storage 구현을 알게 된다.

현재 동작상 문제는 아니지만, runtime은 tick orchestration과 gateway-driven workflow에
집중하고 production wiring은 composition root나 execution adapter 쪽에 두는 편이
의존성 방향이 더 명확하다.

권장:

- `RuntimeTickExecutionDriver` trait는 `runtime`에 유지한다.
- `DbRuntimeExecutionDriver` 구현은 `runner` 하위 wiring module이나 `execution`
  adapter module로 이동한다.
- Runtime core는 `TradingGateway`, audit store, execution driver abstraction만 알도록
  유지한다.

처리 상태: 완료

- 수정: `RuntimeTickExecutionDriver` trait와 `NoopRuntimeExecutionDriver`는 runtime
  core에 유지하고, concrete `DbRuntimeExecutionDriver` 구현은
  `trade-engine/src/runner/execution_driver.rs`로 이동했다.
- 테스트: submit-orders runtime write loop 테스트가 기존 accepted/stale request 동작을
  그대로 검증한다.
- 셀프 리뷰: runtime core에서 `MainDb`, `SqliteBrokerCommandJournal`,
  execution workflow concrete imports를 제거했다. Production storage wiring은 runner
  composition 경계에서만 보인다.

### 3. `runner.rs`가 composition root 이상의 책임을 가지기 시작했다

심각도: Medium/Low

관련 코드:

- `trade-engine/src/runner.rs:147`
- `trade-engine/src/runner.rs:239`
- `trade-engine/src/runner.rs:477`

`runner.rs`는 CLI command 처리, gateway 구성, doctor, runtime wiring, submit-orders,
universe loop, background task 구성을 모두 담당한다. Composition root 역할로는
이해할 수 있지만, 기능이 늘어나면서 변경 지점이 한 파일에 몰리고 있다.

권장:

- CLI command parsing과 command handler를 분리한다.
- Production dependency wiring을 별도 module로 뺀다.
- Auto universe background task는 독립 module로 옮긴다.
- 예: `runner/commands.rs`, `runner/wiring.rs`, `runner/universe_task.rs`

처리 상태: 완료

- 수정: CLI parsing 타입과 clap annotation을 `trade-engine/src/runner/args.rs`로
  분리했다. Production execution wiring은 2번 항목에서
  `trade-engine/src/runner/execution_driver.rs`로 이동했다.
- 테스트: runner args parsing/rejection 테스트가 기존 command surface를 검증한다.
- 셀프 리뷰: command handler 자체를 한 번에 이동하면 doctor, gateway build,
  universe task helper가 같이 얽히므로 이번 항목은 parsing/wiring 분리에 한정했다.
  Auto universe background task는 4번 항목에서 독립 scheduler/task 레이어로 이어서
  처리한다.

### 4. Universe refresh의 clock 정책이 경로별로 다르다

심각도: Low

관련 코드:

- `trade-engine/src/runner.rs:239`
- `trade-engine/src/runner.rs:370`

Submit-orders의 auto universe refresh는 `MarketProfile`의 pre-open window를 사용한다.
반면 manual `universe` command loop는 별도 refresh interval에 따라 직접
`refresh_universe_once`를 호출한다.

Operator command라서 의도적으로 수동 실행을 허용하는 것이라면 괜찮다. 다만 사용자가
동일한 market-window 정책을 기대한다면 두 경로의 차이가 운영상 혼란이 될 수 있다.

권장:

- 의도된 차이라면 README 또는 command help에 명시한다.
- 동일 정책이 필요하면 `UniverseRefreshScheduler` 같은 작은 레이어를 만들어 runtime
  auto refresh와 manual loop가 공유한다.
- Manual command에 market window를 무시하는 명시 옵션을 둘 수도 있다.

처리 상태: 완료

- 수정: `UniverseRefreshScheduler`를 `trade-engine/src/runner/universe_task.rs`에 추가하고,
  submit-orders auto refresh와 `universe --loop`가 같은 market pre-open window/delay
  계산을 공유하게 했다.
- 정책: one-shot `universe` command는 수동 점검용으로 즉시 한 번 refresh한다.
  반복 운용인 `universe --loop`만 market window에 맞춰 대기한다.
- 테스트: scheduler window/delay 단위 테스트와 기존 auto universe refresh 테스트를
  통과시켰다.
- 셀프 리뷰: refresh 실행 자체는 기존 `refresh_universe_once`를 그대로 사용하고,
  scheduling 책임만 runner task 모듈로 분리했다. Provider output, DB snapshot 저장,
  candidate filtering 동작은 바꾸지 않았다.

### 5. `MarketProfile`은 좋은 분리지만 아직 code-defined calendar다

심각도: Low

관련 코드:

- `trade-engine/src/market.rs:105`
- `trade-engine/src/market.rs:192`

국내/미국 정규장, pre-open, DST 계산이 `MarketProfile`로 모인 것은 좋은 변화다. Runtime,
doctor, runner가 같은 market clock abstraction을 쓰기 시작했다.

다만 market hours와 DST 계산이 코드에 박혀 있고 holiday calendar는 표현되어 있지
않다. 현재 KRX와 US equities 범위에서는 충분하지만, 시장이 늘어나거나 휴장일 기반
pre-open 스케줄이 필요해지면 한계가 생긴다.

권장:

- 당장은 유지한다.
- 해외 시장이 늘어나면 `MarketCalendar`를 별도 trait/data source로 분리한다.
- Holiday source를 도입할 때 runtime clock access는 계속 명시적으로 주입한다.

처리 상태: 완료

- 결정: 현재 범위에서는 code-defined `MarketProfile`을 유지한다. 지금 지원 범위는 KRX와
  US equities이고, 휴장일/영업일 여부는 broker/KIS market session 조회 결과를 runtime
  market-hours 판단에 계속 사용한다.
- 이유: 별도 holiday source나 config-driven calendar를 지금 추가하면 실제 데이터
  provider 없이 speculative abstraction이 된다. 현재 문제는 clock 로직 위치였고, 그
  부분은 이미 `MarketProfile`과 `UniverseRefreshScheduler`로 정리되어 있다.
- 후속 조건: KRX/US 외 시장 추가, 공식 holiday source 도입, 휴장일 기반 pre-open
  universe scheduling이 필요해지는 시점에 `MarketCalendar` trait/data source를 추가한다.
- 셀프 리뷰: 코드 변경은 하지 않았다. 현재 runtime clock access는 명시적이고, live
  market open/closed 판단은 KIS market session 결과를 함께 사용한다.

### 6. Overseas cash balance 조회가 watchlist 첫 종목에 의존한다

심각도: Low

관련 코드:

- `trade-engine/src/gateway/adapters/kis.rs:159`
- `trade-engine/src/gateway/adapters/kis.rs:547`

KIS overseas account endpoint 제약 때문에 cash probe symbol이 필요한 것으로 보인다.
현재는 gateway 구성 시 watchlist 첫 종목을 probe symbol로 사용한다.

동작은 가능하지만 설정 의존성이 암묵적이다. Watchlist가 비어 있거나, watchlist 첫
종목의 exchange가 의도한 probe와 다르면 원인 파악이 어렵다.

권장:

- `market.cash_probe_symbol` 같은 명시 config로 분리한다.
- 또는 order/symbol context가 있는 call path에서는 해당 symbol을 probe로 사용한다.
- Config validation에서 overseas account read에 필요한 probe 조건을 명확히 검사한다.

처리 상태: 완료

- 수정: 해외 주식 market config에 `market.cash_probe_symbol`을 추가하고
  `overseas_stock`에서는 필수로 검증한다. 기본 overseas config는 `AAPL`을 probe
  symbol로 명시한다.
- 구현: `KisOverseasStockGateway`의 `cash_probe_symbol`을 `Option<Symbol>`에서
  `Symbol`로 바꿔 gateway 내부에서 빈 probe 상태가 생기지 않게 했다.
- 테스트: overseas config가 exchange, currency, cash probe symbol을 모두 요구하는지와
  기본 overseas config가 해당 값을 읽는지 검증했다.
- 셀프 리뷰: watchlist 첫 종목 암묵 의존성을 제거했고, 기존 account snapshot,
  buying power, quote/order mapping 경로는 유지했다.

## 잘 지켜지고 있는 부분

- `kis-client`와 `trade-engine`의 책임 경계가 비교적 명확하다.
- `trade-engine`의 runtime, risk, execution은 대부분 `TradingGateway` abstraction에
  의존한다.
- Product DB-backed journal 구현이 `kis-client` 내부로 들어가지 않고 주입된다.
- Broker write 전 durable command journal record를 만드는 흐름이 있다.
- Ambiguous broker outcome을 blind retry하지 않고 reconciliation-needed 상태로 다룬다.
- `Decimal`과 typed domain wrappers를 사용해 money/quantity/string primitive 남용을
  줄이고 있다.
- `TradeEngineError`와 `BrokerError`로 product error와 broker error가 분리되어 있다.
- `MarketProfile` 도입으로 runtime/doctor/runner의 market clock 처리가 한 단계
  정리되었다.

## 권장 후속 순서

1. 완료: Final execution risk에서 local hard-stop을 broker read보다 먼저 평가한다.
2. 완료: `DbRuntimeExecutionDriver`를 runtime core 밖으로 옮긴다.
3. 완료: `runner.rs`에서 CLI parsing과 production execution wiring을 분리한다.
4. 완료: Universe refresh scheduler를 공유하고 manual one-shot 정책을 문서화한다.
5. 완료: Overseas cash probe symbol을 명시 config로 뺀다.
6. 완료: 현재는 `MarketProfile`을 유지하고 `MarketCalendar`는 후속 조건 발생 시 추가한다.
