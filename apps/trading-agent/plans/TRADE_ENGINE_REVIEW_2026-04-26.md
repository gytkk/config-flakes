# trade-engine 리뷰: PLAN_V3 대비 구현 갭과 개선 제안

검토일: 2026-04-26

검토 범위:

- `plans/PLAN_V3.md`
- `trade-engine/` 소스, migrations, tests
- `README.md`의 현재 상태 설명

검증 기준선:

- `cargo check --workspace --no-default-features`: 통과
- `cargo fmt --check`: 통과
- `cargo clippy --all-targets --all-features -- -D warnings`: 통과
- `cargo test --all-features`: 통과
- `cargo doc --no-deps --all-features`: 통과

## 요약

`trade-engine`은 PLAN_V3의 초기 제품 경계는 잘 잡고 있다. `TradingGateway`,
KIS read-only adapter, deterministic mock, SQLite migration, DB-backed broker
command journal, dry-run snapshot/decision/risk audit의 기초가 구현되어 있고
검증도 통과한다.

하지만 현재 구현은 아직 "실제 자동매매 에이전트"가 아니라 live read-only
dry-run과 내부 도메인/저장소 골격에 가깝다. PLAN_V3의 핵심인 startup doctor,
외부 strategy spec, decision provider boundary, full risk policy, execution
workflow, reconciliation workflow, 운영 CLI/reporting, certification gate는
대부분 미구현이다.

실거래 안전 관점에서 가장 먼저 고쳐야 할 부분은 다음이다.

1. Runtime run/tick lifecycle을 고쳐 한 loop가 하나의 `runtime_runs`를 갖고,
   tick status와 실패도 DB에 남기게 해야 한다.
2. `read_only=true`인데도 dry-run risk audit에는 `ReadOnlyMode`가 기록되지 않는
   현재 정책 연결을 바로잡아야 한다.
3. `TradingGateway`가 read-only trait라 execution workflow를 아직 구현할 수
   없다. write trait와 final risk/journal/submit/reconcile 상태 전이를 별도
   workflow로 만들어야 한다.
4. Live 계좌/환경 사용이 코드와 문서에서 섞여 있다. live read-only smoke를
   명시한 것인지, paper read-only MVP를 요구하는 것인지 PLAN_V3와 config를
   정리해야 한다.
5. Full account key가 broker journal DB에 평문 저장될 수 있다. 운영 DB 보안
   경계를 명확히 하고, 필요한 경우 암호화 또는 계좌 surrogate 저장으로 바꿔야
   한다.

## PLAN_V3 대비 구현 상태

| 영역 | 현재 상태 | 판단 |
| --- | --- | --- |
| Crate/workspace | `trade-engine` workspace member 존재, module skeleton 존재 | 구현됨 |
| Read-only run CLI | `trade-engine run --dry-run --once`, `run --dry-run` 존재 | 부분 구현 |
| KIS read-only 연결 | env credential, account key, live read-only KIS adapter 존재 | 부분 구현 |
| Main DB | migrations 1-5, 계획상 주요 table 대부분 존재 | 부분 구현 |
| DB-backed journal | `SqliteBrokerCommandJournal` 구현 및 state transition tests 존재 | 구현됨 |
| Gateway | read-only `TradingGateway`, `MockTradingGateway`, `KisGateway` 존재 | 부분 구현 |
| Startup doctor | 별도 `doctor` command, ping, recovery 순서 없음 | 미구현 |
| Snapshot collection | cash/positions/quotes 저장, open orders/fills는 빈 JSON | 부분 구현 |
| Strategy | hold-only와 hard-coded SMA crossover만 있음 | 부분 구현 |
| `.kis.yaml` strategy spec | parser/validator/import/examples 없음 | 미구현 |
| Backtest/promotion | tables만 있고 sidecar/gate 없음 | 미구현 |
| Decision layer | schema/domain validation은 있음, provider trait/LLM boundary 없음 | 부분 구현 |
| Risk engine | read-only/live-disabled/symbol/max order notional 일부만 있음 | 부분 구현 |
| Execution engine | domain object와 DB link helper만 있음, broker submit workflow 없음 | 미구현 |
| Reconciliation workflow | journal primitives만 있음, product workflow/CLI 없음 | 미구현 |
| Runtime loop | 반복 dry-run loop는 있음, tick lease/restart/graceful shutdown 없음 | 부분 구현 |
| Operator CLI/reporting | `run` 외 commands 없음 | 미구현 |
| Panic stop/resume-live | DB table은 있으나 workflow 없음 | 미구현 |
| Certification gate | checklist/workflow 없음 | 미구현 |

## 주요 갭

### 1. Read-only MVP 기준과 현재 구현이 어긋나 있다

PLAN_V3의 Read-only MVP는 "DB가 없어도 실행 가능"을 완료 기준으로 둔다.
반면 현재 `runner`는 `storage.main_db_path`가 없으면 `run`을 실패시킨다.

관련 코드:

- `trade-engine/src/runner.rs`: `validate_run_config`가 `main_db_path`를 필수로 요구한다.
- `plans/PLAN_V3.md`: Read-only MVP 완료 기준은 DB 없이 실행 가능하다고 적혀 있다.

현재 README는 DB-backed dry-run으로 설명하므로 실제 구현은 Phase 2 이후 방향에
가깝다. 둘 중 하나를 선택해야 한다.

- 옵션 A: PLAN_V3를 갱신해 현재 MVP를 "DB-backed live read-only dry-run"으로
  정의한다.
- 옵션 B: `--once` smoke는 `NoopAuditStore` 또는 memory store로 DB 없이도
  실행되게 하고, DB 저장은 별도 모드로 둔다.

실거래 관점에서는 옵션 A가 더 낫다. 거래 시스템은 관측 smoke도 감사 흔적이
남는 편이 운영에 유리하다. 다만 문서와 CLI 에러 메시지는 그 기준으로 맞춰야 한다.

### 2. KIS environment가 live로 고정되어 있다

`build_read_only_kis_gateway`는 `BrokerEnvironment::Live`를 사용한다. 설정 파일도
`live-dry-run`이고 README도 live read-only dry-run으로 설명한다. 반면 PLAN_V3의
초기 startup 흐름에는 paper KIS client를 만든다고 적힌 부분이 있다.

Live read-only smoke 자체는 필요한 단계지만, 현재는 환경 선택이 설정이 아니라
코드 상수에 가깝다. 다음이 필요하다.

- `kis.environment = "live_read_only" | "paper"` 같은 명시적 설정 또는 명령 모드
- live read-only 실행 시 별도 확인 문구와 account suffix 표시
- DB의 `runtime_runs.environment`가 실제 runtime environment에서 오도록 수정

현재 `storage`는 dry-run snapshot 저장 시 environment를 `"live"`로 하드코딩한다.
paper/fake gateway test도 DB에는 live로 남을 수 있어 audit 의미가 흐려진다.

### 3. Startup doctor와 ping/recovery 순서가 없다

PLAN_V3는 read-only startup에서도 broker `ping`을 요구하고, full runtime에서는
migration, journal, single-writer, startup reconciliation, unresolved command
block을 요구한다.

현재 `run_command`는 KIS gateway를 만들고 바로 tick을 실행한다. `TradingGateway::ping`
메서드는 있지만 runner startup에서 호출하지 않는다. 또한 unresolved journal을
확인하거나 startup recovery를 실행하는 product-level 단계도 없다.

우선순위:

1. `trade-engine doctor` command 추가
2. `run --dry-run --once` 시작 전에 config validate, DB migrate, credential check,
   `ping`, account suffix redaction check 수행
3. live-capable 모드가 생기기 전에 `SqliteBrokerCommandJournal::list_unreconciled`
   기반 startup block 추가

### 4. Runtime loop가 PLAN_V3의 run/tick 모델과 다르다

현재 `record_dry_run_tick`은 매 tick마다 새 `runtime_runs`와 새 `runtime_ticks`를
동시에 insert한다. PLAN_V3의 모델은 하나의 run 아래 여러 tick이 쌓이고, restart
후에도 resume 가능한 run/tick lifecycle을 전제로 한다.

또한 현재 loop는 다음을 아직 하지 않는다.

- tick lease 획득
- 이전 tick이 실행 중일 때 skip
- `max_tick_runtime_seconds` 강제
- `snapshot_failed`, `risk_blocked`, `execution_requested` 같은 tick status 저장
- 실패한 tick도 DB에 저장
- graceful shutdown
- market session 5분 TTL cache
- 장 마감 시 신규 주문 판단 skip

현재 `max_tick_runtime_seconds`와 `skip_if_previous_tick_running`은 로그에만 나오고
행동으로 이어지지 않는다. 실거래 전에는 이 둘을 실제 control flow로 연결해야 한다.

### 5. Snapshot 실패가 감사되지 않는다

`collect_dry_run_snapshot`에서 account snapshot이나 quote 하나가 실패하면 함수가
바로 error를 반환한다. 그 실패는 CLI error로만 나오고 `runtime_ticks`에는
`snapshot_failed` row가 남지 않는다.

PLAN_V3 기준으로는 실패한 endpoint와 단계가 감사 가능해야 한다. 특히 실제
트레이딩에서는 "데이터가 없어서 거래하지 않았다"와 "런타임이 죽어서 판단하지
못했다"를 구분해야 한다.

권장:

- tick row를 `started`로 먼저 insert
- account/quote 단계별 실패 시 `snapshot_failed`와 stage/error 저장
- 부분 quote 실패 정책을 명시: 전체 tick 실패, 해당 symbol skip, stale quote 사용 금지

### 6. Strategy spec과 runtime strategy 선택이 없다

현재 runtime은 `SmaCrossoverStrategy::default_v1()`를 직접 생성한다. 외부
`.kis.yaml` parser, strategy import, strategy version, active/approved 상태,
promotion evidence 연결이 없다.

PLAN_V3의 strategy/backtest/promotion 방향을 살리려면 다음 순서가 좋다.

1. `StrategySpec` domain과 `.kis.yaml` parser 추가
2. `strategy validate <file>` CLI 추가
3. `strategy_specs`/`strategy_versions` 저장 구현
4. runtime은 hard-coded `default_v1`이 아니라 저장된 active strategy version을 읽음
5. risk policy가 strategy approval/backtest evidence를 검사

### 7. Decision provider boundary가 없다

`DecisionProposal` schema와 validation은 잘 시작되어 있다. 다만 현재 decision은
`decision_from_signal` 내부 함수가 strategy signal을 그대로 proposal로 바꾸는
형태다. PLAN_V3의 `DeterministicDecisionProvider`/`LlmDecisionProvider` trait
경계는 아직 없다.

추가로 LLM output을 받을 계획이면 다음을 미리 넣는 것이 좋다.

- `#[serde(deny_unknown_fields)]` on decision request/response structs
- raw invalid JSON 저장 후 rejected output 저장
- reason/risk_notes 길이 제한
- provider timeout, retry 금지 또는 idempotent retry 정책
- symbol/order type/quantity/price validation 결과를 DB에 reason과 함께 저장

### 8. RiskPolicy가 실제 live gate로는 부족하다

현재 `RiskPolicy`는 다음만 검사한다.

- read-only mode
- live trading enabled
- symbol allowlist
- market order reference price
- max order notional

PLAN_V3의 최소 정책 중 다음은 아직 없다.

- strategy active/approved
- backtest evidence
- paper observation period
- live account allowlist
- confirmed account suffix
- market session open
- panic stop
- unresolved broker command
- single-writer lock
- max position notional
- max daily order count
- max daily loss
- min signal strength
- min AI confidence
- buying power
- duplicate intent
- order cooldown

중요한 현재 연결 문제도 있다. `run_dry_run_tick`은 `RiskPolicy::new(false, false, ...)`
로 정책을 만들기 때문에 실제 config가 `read_only=true`여도 risk audit에는
`ReadOnlyMode`가 기록되지 않는다. buy signal은 `LiveTradingDisabled`로 block되지만
read-only 차단 사유가 빠진다.

실거래 안전 관점에서는 runtime mode를 risk input으로 반드시 전달하고, block reason은
"왜 주문이 나가지 않았는지"를 운영자가 그대로 이해할 수 있어야 한다.

### 9. Execution workflow는 아직 없다

`execution` module은 approved risk와 hold rejection 같은 domain rule을 가진다.
`storage`에는 `execution_requests` insert와 journal link helper가 있다. 그러나
PLAN_V3가 요구하는 실제 workflow는 없다.

빠진 단계:

- approved execution request를 DB에서 읽기
- `ClientOrderId` 생성
- broker command journal reserve
- submit 직전 final risk 재검사
- `TradingGateway`를 통한 broker write submit
- accepted/rejected/ambiguous 결과별 status/event 저장
- ambiguous outcome을 reconciliation-needed로 연결

또한 `TradingGateway` trait 자체가 read-only 메서드만 갖고 있어 execution engine이
gateway를 통해 주문을 낼 수 없다. read/write trait를 분리하는 편이 안전하다.

권장 구조:

- `TradingReadGateway`: 현재 read methods
- `TradingWriteGateway`: place/cancel/amend methods
- execution workflow는 live-capable 모드에서만 `TradingWriteGateway`를 받음
- dry-run/read-only runtime은 타입 수준에서도 write gateway를 받지 않음

### 10. Execution status domain과 DB status가 다르다

DB status는 `requested`, `journal_reserved`, `accepted`, `rejected`,
`reconciliation_needed`, `failed`를 허용한다. `ExecutionRequestStatus` enum은
`Requested`, `Accepted`, `Rejected`, `ReconciliationNeeded`만 가진다.

`journal_reserved`와 `failed`가 domain enum에 없으면 상태 전이 로직이 DB 문자열에
분산될 가능성이 높다. execution workflow 구현 전에 enum과 DB status를 맞추는 것이
좋다.

### 11. Product-level reconciliation workflow가 없다

`SqliteBrokerCommandJournal`은 unresolved command를 list하고 상태를 바꾸는
primitive를 제공한다. `kis-client`도 reconciliation helper를 갖고 있다. 그러나
`trade-engine`에는 다음이 없다.

- startup reconciliation orchestration
- pending reconciliation CLI
- execution state와 journal state를 함께 갱신하는 workflow
- `ReconciliationStillUnknown`, `ManualInterventionRequired` product event
- unresolved record가 live write를 block한다는 product-level test

이는 live write 전 필수 블로커다.

### 12. Operator CLI와 reporting이 거의 없다

현재 CLI subcommand는 `run`뿐이다. PLAN_V3의 초기 commands 중 `doctor`,
`config validate`, `strategy validate/import`, `decisions list`, `risk blocks`,
`executions list`, `reconcile pending`, `portfolio show`, `report daily`,
`panic-stop`, `resume-live`는 없다.

운영 관점에서는 execution보다 먼저 read-only 조회 CLI를 추가하는 편이 좋다.
DB에서 읽는 CLI가 생겨야 system of record가 실제로 쓸모 있어진다.

## Rust best practice 관점 개선점

### 1. Sync SQLite를 async runtime에서 직접 실행한다

현재 `rusqlite::Connection`을 async path에서 직접 사용한다. 테스트와 작은 smoke에는
문제가 작지만, 지속 loop와 reporting/query CLI가 커지면 tokio worker thread를
막을 수 있다.

선택지:

- DB 작업을 dedicated blocking boundary로 감싸기
- `tokio_rusqlite` 같은 dedicated thread 모델 사용
- 장기적으로 SQLx async pool로 이동

현재 규모에서는 즉시 바꿀 필요는 없지만, execution workflow 전에 DB boundary를
명확히 하는 것이 좋다.

### 2. Migrations가 부분 실패에 취약하다

`apply_migrations`는 migration SQL batch를 실행한 뒤 schema_migrations row를
기록한다. migration 중간에 실패하면 일부 `ALTER TABLE`만 적용되고 version row는
없는 상태가 될 수 있다. 다음 실행에서는 같은 `ALTER TABLE ADD COLUMN`이 실패할 수
있다.

운영 DB에서는 migration을 transaction으로 감싸고, SQLite의 DDL 특성을 감안해
부분 적용 복구 정책을 테스트해야 한다. 최소한 migration failure simulation test를
추가하는 것이 좋다.

### 3. SQL 문자열 생성은 내부 whitelist로만 유지해야 한다

`count_table`과 `column_exists`는 table/column 이름을 format string으로 넣는다.
현재는 내부 상수/테스트에서만 호출되어 괜찮다. 향후 CLI 입력이 이 경로에 들어오면
안 된다. table enum 또는 whitelist helper를 두면 확장 시 안전하다.

### 4. 상태 문자열을 domain enum으로 모으는 편이 좋다

DB insert/update에는 `"approved"`, `"requested"`, `"broker_command_reserved"` 같은
문자열이 흩어져 있다. 지금은 tests가 막아주지만 execution/reconciliation이 커지면
상태 전이 버그가 생기기 쉽다.

권장:

- `ExecutionStatus`, `RiskStatus`, `TickStatus`, `EventType`에 `as_str`/`parse`
  구현
- DB CHECK values와 Rust enum values를 test로 동기화
- storage API는 문자열 대신 enum을 받도록 제한

### 5. Runtime output은 structured logging으로 옮겨야 한다

현재 출력은 `String`에 `writeln!`으로 누적한다. MVP test에는 단순하고 좋지만,
운영에서는 stdout text가 감사 로그의 대체물이 되면 안 된다.

권장:

- CLI human output과 machine audit event를 분리
- `tracing` + JSON formatter 사용
- secret/account redaction을 formatter 또는 domain type에서 강제

### 6. Test helper의 `Box::leak`는 피할 수 있다

`strategy_decision_risk.rs`는 test helper에서 `Box::leak`로 lifetime을 맞춘다.
테스트 프로세스 수명에서는 큰 문제는 아니지만 불필요하다. helper struct로
`AccountSnapshot`과 `Quote`를 소유하고 `StrategyInput`을 만드는 방식이 더 깔끔하다.

## 실제 트레이딩 에이전트 관점 개선점

### 1. Live write 전 필수 게이트를 별도 checklist로 고정해야 한다

코드에 기능을 추가하는 것과 live readiness는 다르다. PLAN_V3의 인증 순서처럼
다음 단계가 독립적으로 증명되어야 한다.

- paper read-only smoke
- write 없는 dry-run loop
- controlled paper order
- paper cancel/amend drill
- seeded unresolved command startup recovery
- live read-only smoke
- limited live write approval

이 checklist는 docs만이 아니라 DB에 operator action으로 남겨야 한다.

### 2. Buying power와 open orders가 risk input에 들어가야 한다

현재 snapshot은 cash/positions/quotes 중심이다. 실제 주문 전에는 buying power,
open orders, today fills가 필요하다. 특히 중복 의도 방지와 cooldown은 open orders
없이 구현하기 어렵다.

### 3. 장 상태는 once가 아니라 tick-time source여야 한다

현재 loop 시작 전에 market hours를 한 번만 구한다. 장중/장마감 경계를 지나가는
long-running loop에서는 부정확해진다. PLAN_V3처럼 5분 TTL cache 또는 다음 session
boundary 기반 refresh가 필요하다.

### 4. 실패 정책을 명시해야 한다

실제 자동매매에서는 "데이터 실패 시 hold", "이전 가격 재사용", "symbol별 skip" 같은
정책이 거래 결과를 바꾼다. 현재는 quote 하나 실패 시 전체 tick이 error로 끝난다.
이 정책은 config와 risk/audit에 명시되어야 한다. 초기 live에서는 stale data를
사용하지 않고 해당 tick을 block하는 보수적 정책이 적합하다.

### 5. Full account key 저장 정책을 정해야 한다

broker command journal은 `account_key`를 저장한다. reconciliation과 정확한 broker
호출에는 계좌 식별이 필요할 수 있지만, PLAN_V3는 secret과 전체 계좌번호가 일반
감사 출력에 평문 저장되지 않아야 한다고 둔다.

권장:

- DB에는 account suffix 또는 account alias를 기본 저장
- full account key가 필요한 경우 OS keyring/secret manager/config에서 resolve
- 불가피하게 DB 저장이 필요하면 암호화와 접근권한, dump redaction 정책 추가

### 6. Panic stop은 execution보다 먼저 들어가야 한다

`operator_actions` table은 있지만 panic stop workflow는 없다. live write path를
만들기 전에 다음이 필요하다.

- panic stop activate command
- panic stop clear/resume command
- risk policy가 panic stop을 매번 읽고 block
- clear는 unresolved journal을 지우지 않음

## 추천 작업 순서

1. PLAN_V3와 README를 현재 방향에 맞게 정리한다. 특히 "DB 없이 실행 가능",
   "paper vs live read-only", "token cache configurable" 기준을 확정한다.
2. `doctor`와 startup preflight를 만든다. `ping`, DB migration, config validation,
   account suffix redaction, unresolved journal check를 포함한다.
3. run/tick lifecycle을 고친다. 하나의 run 아래 여러 tick을 저장하고, 실패 tick도
   status와 error로 남긴다.
4. risk input을 runtime config와 snapshot으로 확장한다. 우선 read-only, live disabled,
   market closed, unresolved command, single-writer, buying power, duplicate intent를
   넣는다.
5. strategy spec parser와 `strategy validate/import`를 추가하고 hard-coded
   `default_v1` runtime 의존을 제거한다.
6. decision provider trait와 deterministic provider를 추가한다. malformed output
   저장/reject path를 먼저 테스트한다.
7. write-capable gateway trait를 read gateway와 분리하고 execution workflow를
   TDD로 구현한다.
8. product-level reconciliation workflow와 `reconcile pending` CLI를 구현한다.
9. DB-backed operator CLI/reporting을 추가한다.
10. certification gate와 panic stop/resume-live workflow를 live write 전에 고정한다.

## 결론

현재 `trade-engine`은 PLAN_V3 방향의 좋은 기반이다. 특히 KIS 세부를 `kis-client`에
두고 product runtime은 gateway와 DB 감사 모델로 분리한 점, broker command journal을
product-owned SQLite 구현으로 둔 점은 계획과 잘 맞는다.

다만 실거래 에이전트로 보기에는 아직 핵심 안전 경로가 비어 있다. 다음 단계는
새로운 전략을 붙이는 것이 아니라 startup doctor, tick audit lifecycle, full risk
input, execution/reconciliation workflow를 먼저 닫는 것이다. 이 순서가 중복 주문,
blind retry, 감사 누락 같은 실제 운영 리스크를 가장 빨리 줄인다.
