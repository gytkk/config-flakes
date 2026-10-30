# KIS Client Library Implementation Plan (Rust)

## Purpose

이 문서는 `trading-agent` 프로젝트 안에서 사용할 **Rust 기반 한국투자증권(KIS) 클라이언트 라이브러리**의 실제 구현 계획을 정의한다.

`PLAN.md`가 전체 trading-agent 제품 계획의 source of truth이고, 이 문서는 그중 **broker adapter layer**에 해당하는 KIS client component 계획이다. 현재 Rust 구현은 workspace member crate인 `kis-client` 아래에 둔다.

> Current layout note: this plan was originally written with nested
> `broker/kis/*` module paths. The current implementation keeps the same
> responsibility boundaries under the flatter `kis-client/src/{auth,broker,
> client,domain,error,journal,mapping,models,reconcile,safety,transport}.rs`
> layout. The broker trait, domain models, `BrokerError`, and durable journal
> contract are owned by `kis-client`; SQLite persistence and migrations are
> owned by the product `agent` storage layer. There is no separate
> `broker-core` workspace crate.

목표는 단순한 샘플 코드 래핑이 아니라 아래 조건을 만족하는 **paper-first, live-ready** 브로커 계층을 만드는 것이다.

- 안정적인 인증 처리
- 일관된 내부 도메인 모델 제공
- 실전/모의 환경 분리
- 주문/체결/잔고/시세 흐름의 신뢰 가능한 추상화
- 재시도, 로깅, reconciliation을 포함한 운영 가능한 전송 계층
- 프로세스 재시작과 애매한 주문 결과를 견디는 durable order journal
- live 운용 전용 guardrail, 단일 writer 제약, 운영자 복구 절차
- TDD-first 구현 원칙과 회귀 테스트 기반 변경 관리
- 각 KIS REST API 스펙을 구현 전에 executable test case로 먼저 고정

이 문서에서 말하는 "live-ready"는 곧바로 무제한 실거래를 허용한다는 뜻이 아니다.

- `paper` 단계에서 검증한 구조를 `live`로 올릴 때 아키텍처 재작성 없이 확장 가능해야 한다.
- `live` write path는 durable persistence, startup reconciliation, operator control 없이는 활성화되지 않아야 한다.
- 애매한 네트워크 실패에서 "중복 주문을 피하는 것"을 최우선으로 설계해야 한다.

---

## Live-Ready Positioning

KIS client v1은 **paper에서 바로 쓸 수 있어야 하고**, **live로 전환할 때 필요한 안전장치가 이미 구조 안에 들어 있어야 한다**.

핵심 전제는 다음과 같다.

1. **single-writer**
   - 동일 `(environment, account)`에 대해 동시에 주문을 쓰는 프로세스는 하나만 허용한다.
   - active-active multi-host live trading은 v1 범위 밖이다.

2. **durable before write**
   - live에서 broker write 전에는 반드시 command가 durable store에 기록되어야 한다.
   - in-memory state만으로는 live write를 허용하지 않는다.

3. **reconcile before retry**
   - broker write path에서 결과가 애매하면 같은 command를 blind retry하지 않는다.
   - 먼저 기존 주문 또는 후속 command 반영 여부를 reconcile한다.

4. **operator-visible**
   - live 차단 이유, pending reconcile, 마지막 broker 응답, safety halt 상태를 사람이 확인할 수 있어야 한다.

5. **spec-tested before implemented**
   - KIS API endpoint 하나를 추가할 때마다 request shape, header, response mapping, error mapping, pagination/retry behavior 테스트를 먼저 작성한다.
   - 주문성 API는 journal/reconcile/safety 테스트가 없으면 구현을 시작하지 않는다.

---

## Design Goals

KIS client는 아래 특성을 가져야 한다.

1. **TDD-first**
   - 구현보다 테스트를 먼저 작성한다.
   - 특히 인증, transport retry, pagination, mapping, journal state transition, reconciliation, live guardrail은 실패 케이스 테스트 없이 구현하지 않는다.
   - broker write path는 테스트로 재현 가능한 safety invariant를 먼저 정의한 뒤 구현한다.
   - API 스펙 해석은 문서 요약이 아니라 테스트 fixture와 expected domain model로 남긴다.

2. **library-first**
   - runtime loop, reconciliation job, reporting에서 공통 사용 가능해야 한다.
   - operator CLI는 product runtime을 소유하는 `agent` crate 경계에서
     구현한다.

3. **deterministic**
   - agent 로직 없음
   - 브로커 연동, 상태 변환, 안전한 write orchestration에만 집중

4. **typed interface**
   - 외부 KIS 응답을 내부 타입으로 변환
   - 문자열 map 전파 금지
   - 금액은 `rust_decimal::Decimal`, 수량은 정수 기반 newtype으로 다룬다.

5. **environment-safe**
   - `mock`, `paper`, `live`를 명확히 분리
   - 잘못된 환경 혼선 방지
   - `live`는 별도 enable 절차 없이는 read-only여야 한다.

6. **operable**
   - retry, rate limit, request id, raw response metadata, pagination, reconciliation 고려

7. **durable-by-default**
   - live write path는 durable order journal 없이는 사용할 수 없어야 한다.
   - 프로세스 재시작 후에도 미결 주문 상태를 이어서 복구할 수 있어야 한다.

8. **single-writer safe**
   - 동일 계좌에 대한 중복 submit을 구조적으로 어렵게 만든다.

---

## Scope

### In scope for v1

- REST access token 발급 및 재사용
- 국내주식 현재가 조회
- 국내주식 잔고 조회
- 현금/주문가능금액 조회
- 현금 매수/매도 주문
- 주문 정정/취소
- 당일 주문 조회
- 미체결/정정취소가능주문 조회
- 당일 체결 조회
- 장운영/휴장일 확인
- KIS 응답을 내부 모델로 정규화
- continuation / pagination (`tr_cont`, `CTX_AREA_*`) 처리
- 공통 오류 체계
- 유량 제어 및 backpressure
- durable order journal contract
- startup recovery / reconciliation helper
- live guardrails 및 read-only mode
- mock-compatible broker interface
- KIS REST API별 spec test matrix와 fixture

### In scope for v1.1

- 해외주식에 대해 위의 국내주식 기능 대부분 구현
- 필요할 경우 WebSocket 실시간 체결/호가 별도 모듈 추가

### Out of scope for v1

- 국내선물옵션
- WebSocket 실시간 체결/호가
- 조건검색
- 다계좌 동시 운용
- 고빈도 주문 최적화
- 브로커 독립 서비스 분리
- 다중 호스트 active-active live write

---

## Architecture Boundaries

이 문서는 책임 경계를 아래처럼 둔다.

### KIS client가 책임지는 것

- KIS 인증, 헤더 조립, 엔드포인트 호출
- typed domain model 변환
- rate limit / continuation / pagination 처리
- durable order journal과 연동되는 write orchestration
- ambiguous write outcome 분류
- startup reconciliation helper

### 상위 execution/runtime이 책임지는 것

- 전략/agent 의사결정
- 리스크 정책
- 스케줄링
- panic stop 운영 정책
- 배포 환경의 secret 주입, process supervision, backup 운영

### 중요한 원칙

- KIS client는 live trading을 "쉽게" 만드는 것이 아니라 "안전하게" 만드는 방향으로 설계한다.
- 상위 계층은 broker client를 호출할 수 있지만, live write는 command journal과 single-writer lock이 없으면 실패해야 한다.

---

## Rust Stack Decision

v1은 **Rust 단일 crate**로 시작하되, 전체 trading-agent repository 안에서는 `kis-client` workspace member로 둔다. KIS client 내부를 여러 crate로 쪼개지 않는다. 파일 수와 모듈 수를 낮게 유지하면서, live-ready에 필요한 안전 로직은 타입/상태 전이/DB transaction으로 강제한다.

### Required crates

- `tokio`: async runtime, timer, task, signal handling
- `reqwest`: KIS REST HTTP client
- `serde`, `serde_json`: request/response serialization
- `sqlx` with `sqlite`, `runtime-tokio`, `macros`: SQLite persistence and migrations in `agent` storage
- `rust_decimal`: 금액/가격
- `time`: UTC/KST timestamp handling
- `thiserror`: library error enum
- `async-trait`: public async broker trait가 trait object로 필요할 때 사용
- `tracing`: structured logs
- `secrecy`: secret value accidental logging 방지
- `uuid`: request id / client order id component
- `governor` or custom limiter: endpoint group별 rate limiting

### Rust stance

- 모든 broker I/O는 async로 구현한다.
- public library API는 `Result<T, BrokerError>`를 반환한다.
- domain model은 `#[derive(Debug, Clone, Serialize, Deserialize)]`를 기본으로 하되 secret은 `Debug`에 노출하지 않는다.
- `f64`는 금액/가격/수량에 사용하지 않는다.
- 상태 전이는 enum과 storage implementation의 constraint 양쪽에서 보호한다.

### Why Rust here

- live-ready 코드에서 타입으로 막을 수 있는 실수를 컴파일 타임에 줄일 수 있다.
- async HTTP, SQLite transaction, structured logging을 하나의 runtime 안에서 일관되게 운용할 수 있다.
- 다만 Rust는 구현 학습/개발 비용이 높으므로 v1 범위를 좁게 유지하고, 단일 crate + 명확한 모듈 경계로 시작한다.

---

## SQLite Storage Decision

v1 persistence는 SQLite로 시작하되, concrete DB 구현과 migration은
`kis-client`가 아니라 product runtime을 소유하는 `agent` crate에 둔다.

이 프로젝트는 고빈도 주문 엔진이 아니라 단일 계좌, single-writer, command journal, reconciliation 중심 구조다. 따라서 live-ready 신뢰성의 핵심은 DB 종류보다 transaction boundary, unique constraint, startup recovery, 운영 절차에 있다.

### SQLite requirements

- WAL mode 사용
- `PRAGMA foreign_keys = ON`
- `busy_timeout` 설정
- command journal write는 transaction 안에서 처리
- `client_order_id` unique constraint 필수
- parent/child command 관계는 foreign key 또는 명시적 integrity check로 보호
- live write는 단일 프로세스/single-writer로 제한
- DB 파일은 로컬 디스크에 저장
- 네트워크 파일시스템 또는 동기화 폴더 위의 SQLite DB 금지
- DB backup은 live write와 충돌하지 않는 방식으로 수행

### SQLx rules

- `SqlitePool`은 v1에서 `max_connections(1)`로 시작한다.
- migration은 `agent/migrations/`에 두고 startup에서 적용 여부를 검증한다.
- query macro를 쓰는 경우 `cargo sqlx prepare`를 CI에 포함한다.
- migration 없는 implicit schema 생성은 금지한다.
- journal 상태 전이는 반드시 `Transaction<'_, Sqlite>` 안에서 수행한다.

---

## Package Layout

파일 수를 과도하게 늘리지 않기 위해 KIS client는 workspace 안의 단일 crate로 시작한다. 아래 경로는 repository root 기준이다.

```text
Cargo.toml
kis-client/
  Cargo.toml
  src/
    lib.rs
    config.rs
    error.rs
    logging.rs
    broker/
      mod.rs
      base.rs
      journal.rs
      mock.rs
      kis/
        mod.rs
        client.rs
        auth.rs
        transport.rs
        protocol/
          mod.rs
          domestic_stock/
        mapping/
          mod.rs
        safety.rs
        reconcile.rs
  tests/
    api_spec_contract.rs
    broker_journal.rs
    kis_auth.rs
    kis_mapping.rs
    kis_transport.rs
    kis_reconcile.rs
    live_guardrails.rs
    support/
      fake_kis_server.rs
      fixtures/
        auth/
        market/
        account/
        orders/
        inquiry/
        recovery/
```

### Module responsibilities

#### `broker/base.rs`

- 브로커 공통 trait와 domain model 정의
- 상위 계층은 이것만 직접 의존
- `BrokerClient`, `Quote`, `Position`, `OrderReference`, `OrderStatus` 포함

#### `broker/mock.rs`

- broker trait를 따르는 local fake broker
- KIS network 호출 없음
- unit/integration test에서 deterministic fixture 제공

#### `broker/journal.rs`

- 브로커 공통 write command model
- SQLite 기반 durable command journal
- place / cancel / amend command reserve / submit / ack / reconcile state transition 기록
- 특정 브로커 구현에 묶이지 않는 persistence 계약 보관

#### `broker/kis/client.rs`

- 공개 KIS client 진입점
- read API와 write orchestration 제공
- 내부적으로 auth, transport, mapping, safety, reconcile, command journal 조합

#### `broker/kis/auth.rs`

- access token 발급 및 재사용
- secure local token cache
- expiry 추적
- process/file lock 기반 중복 발급 억제
- paper/live 환경 분기

#### `broker/kis/transport.rs`

- KIS endpoint metadata
- `reqwest::Client` 기반 HTTP 전송 계층
- timeout, retry gate, connection pool, logging hook, tracing metadata
- endpoint별 헤더 조립
- `tr_cont`, `CTX_AREA_FK100`, `CTX_AREA_NK100` 같은 continuation 처리
- 환경별/엔드포인트 그룹별 유량 제어

#### `kis-client/src/protocol/`

- KIS raw payload 보조 모델
- KIS native identifier model
- endpoint별 lightweight response schema

#### `kis-client/src/mapping/`

- KIS raw response -> internal domain model 변환
- KIS status / body code / message -> 내부 오류 체계 정규화
- enum/status mapping
- `Decimal`, UTC/KST datetime, nullable field 정규화

#### `broker/kis/safety.rs`

- live enable flag, account allowlist, market session check, single-writer lock
- order lifecycle state machine

#### `broker/kis/reconcile.rs`

- ambiguous order recovery
- startup recovery
- order/fill/balance 기반 상태 재구성

### Dependency rules

- 상위 runtime은 `trading-agent-kis-client` crate의 `broker::base`와 공개 `broker::kis::KisBrokerClient`만 사용한다.
- `broker/kis/transport.rs`는 domain model을 알지 않고 raw KIS response만 반환한다.
- `kis-client/src/mapping/`은 network 호출을 하지 않는다.
- `broker/kis/safety.rs`와 `broker/kis/reconcile.rs`는 write/recovery 정책만 담당하고 HTTP 세부 구현을 숨긴다.
- `broker/journal.rs`는 KIS 전용 TR id나 raw payload를 직접 알지 않는다.

---

## Public Interface Proposal

Rust trait는 object-safety와 async ergonomics를 고려해 v1에서는 `async-trait` 사용을 허용한다. trait object가 필요 없어지면 concrete type API로 단순화할 수 있다.

```rust
#[async_trait::async_trait]
pub trait BrokerClient: Send + Sync {
    async fn get_quote(&self, symbol: &Symbol) -> Result<Quote, BrokerError>;
    async fn get_positions(&self) -> Result<Vec<Position>, BrokerError>;
    async fn get_cash_balance(&self) -> Result<CashBalance, BrokerError>;
    async fn get_buying_power(
        &self,
        symbol: Option<&Symbol>,
    ) -> Result<BuyingPower, BrokerError>;
    async fn place_order(
        &self,
        order: CreateOrderRequest,
    ) -> Result<OrderSubmission, BrokerError>;
    async fn cancel_order(
        &self,
        request: CancelOrderRequest,
    ) -> Result<CancelResult, BrokerError>;
    async fn amend_order(
        &self,
        request: AmendOrderRequest,
    ) -> Result<AmendResult, BrokerError>;
    async fn get_order(
        &self,
        query: OrderQuery,
    ) -> Result<OrderStatus, BrokerError>;
    async fn list_open_orders(&self) -> Result<Vec<OrderStatus>, BrokerError>;
    async fn list_orders(&self, day: TradingDay) -> Result<Vec<OrderStatus>, BrokerError>;
    async fn list_fills(&self, day: TradingDay) -> Result<Vec<Fill>, BrokerError>;
    async fn reconcile_order(
        &self,
        client_order_id: &ClientOrderId,
        lookback_days: u16,
    ) -> Result<ReconciliationResult, BrokerError>;
    async fn ping(&self) -> Result<BrokerHealth, BrokerError>;
}
```

### Why `OrderReference` is required

- `cancel_order(order_id: String)`는 live-ready 설계로 부족하다.
- KIS 정정/취소는 원주문 식별에 broker-native 필드가 더 필요할 수 있다.
- 따라서 후속 write는 `OrderReference` 또는 그것을 포함한 request model을 기준으로 수행한다.

### Broker command journal trait

```rust
#[async_trait::async_trait]
pub trait BrokerCommandJournal: Send + Sync {
    async fn reserve_command(
        &self,
        command: BrokerCommandRecord,
    ) -> Result<(), BrokerError>;
    async fn mark_submitting(
        &self,
        client_order_id: &ClientOrderId,
        request_id: &RequestId,
    ) -> Result<(), BrokerError>;
    async fn mark_ack(
        &self,
        client_order_id: &ClientOrderId,
        ack: BrokerCommandAck,
    ) -> Result<(), BrokerError>;
    async fn mark_reconcile_needed(
        &self,
        client_order_id: &ClientOrderId,
        reason: &str,
    ) -> Result<(), BrokerError>;
    async fn mark_terminal(
        &self,
        client_order_id: &ClientOrderId,
        status: BrokerCommandStatus,
    ) -> Result<(), BrokerError>;
    async fn list_unreconciled(&self) -> Result<Vec<BrokerCommandRecord>, BrokerError>;
}
```

### Live rule

- `live`에서 `place_order`, `cancel_order`, `amend_order`는 `BrokerCommandJournal`과 single-writer lock 없이는 실패해야 한다.

---

## Internal Domain Models Expected

KIS client는 최소 아래 내부 모델에 맞춰 데이터를 반환해야 한다.

- `Quote`
- `Position`
- `CashBalance`
- `BuyingPower`
- `CreateOrderRequest`
- `OrderSubmission`
- `OrderReference`
- `OrderStatus`
- `CancelOrderRequest`
- `CancelResult`
- `AmendOrderRequest`
- `AmendResult`
- `Fill`
- `BrokerHealth`
- `ReconciliationResult`
- `BrokerCommandRecord`
- `BrokerCommandAck`
- `BrokerCommandStatus`

핵심 원칙:

- KIS 필드명은 client 내부에만 머무른다.
- 상위 계층은 내부 도메인 모델만 본다.
- 정정/취소/조회에 필요한 broker-native 식별자는 `OrderReference`라는 typed wrapper 안에 캡슐화한다.
- `String` 원시 타입을 남발하지 않고 `Symbol`, `AccountKey`, `ClientOrderId`, `RequestId` 같은 newtype을 사용한다.

---

## Time and Calendar Model

금융 거래에서는 시간 해석이 곧 동작 해석이다.

### Rules

- 내부 저장 시각은 UTC로 저장한다.
- 한국 시장 세션 판단은 `Asia/Seoul` 기준으로 한다.
- 로그에는 UTC timestamp와 KST market timestamp를 모두 남길 수 있어야 한다.
- 상대 시간 표현 대신 절대 시간 기록을 기본으로 한다.
- `time::OffsetDateTime`과 explicit timezone conversion helper를 사용한다.

### Guardrails

- 시스템 clock drift가 과도하면 `ping()` 또는 startup doctor에서 경고 또는 실패 처리한다.
- write path는 장운영정보/휴장일 확인 없이 진행하지 않는다.

---

## Environment Model

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrokerEnvironment {
    Mock,
    Paper,
    Live,
}
```

### Rules

- `mock`: local fake broker, no KIS network
- `paper`: KIS 모의투자 credentials + paper base URL
- `live`: KIS 실전투자 credentials + live base URL

### Additional runtime flags

- `read_only`: read API만 허용
- `enable_live_trading`: live write path 활성화 플래그
- `confirmed_account_suffix`: 운영자가 의도한 계좌 suffix 확인용 문자열

### Guardrails

- `live`에서 write path 실행 시 명시적 enable flag 필요
- `paper`/`live` account metadata mismatch 시 즉시 실패
- startup log에 environment, account suffix, mode, host, pid 출력
- `(environment, account)` 단위 single-writer lock 획득 실패 시 live write 비활성화
- unresolved broker command가 남아 있으면 startup에서 reconcile 완료 전까지 live write 차단

---

## Authentication Plan

### Requirements

- 토큰 발급 로직 분리
- 토큰 만료 전 재사용 및 재발급
- concurrent access 시 중복 발급 최소화
- paper/live 토큰 캐시 분리
- 프로세스 재시작 후에도 토큰 재사용 가능
- KIS 발급 정책 변경 시 하드코딩된 TTL 가정 없이 적응 가능

### Proposed API

```rust
pub struct KisTokenProvider {
    client: reqwest::Client,
    cache: TokenCache,
    config: KisAuthConfig,
}

impl KisTokenProvider {
    pub async fn access_token(
        &self,
        force_refresh: bool,
    ) -> Result<AccessToken, BrokerError>;

    pub async fn invalidate(&self, reason: &str) -> Result<(), BrokerError>;

    pub async fn snapshot(&self) -> Result<TokenSnapshot, BrokerError>;
}
```

### Token behavior

- cache key는 `environment + account + appkey fingerprint` 기준으로 분리
- in-memory cache + secure local file cache를 함께 사용
- server response의 expiry 값을 기준으로 `expires_at` 추적
- 만료 전 선제 갱신 margin은 설정값으로 관리
- file lock 또는 inter-process lock으로 동시 갱신 충돌 방지
- 토큰 발급 실패 시 즉시 무한 재시도하지 않고 throttle/backoff 적용
- `401` 또는 인증 오류 시 1회 invalidate 후 재시도
- write path에서 인증 오류가 submit 시점과 경계가 모호하면 reconcile-needed로 전환

### Local token cache requirements

- 파일 권한은 owner read/write only (`0600`)로 제한
- atomic write 사용
- cache payload에 schema version, environment, account key fingerprint, expiry 포함
- cache corruption 또는 fingerprint mismatch 시 폐기 후 재발급
- shared volume 또는 repo 내부 경로에 저장 금지
- 가능하면 OS keyring 또는 encrypted local secret store를 우선 사용

### Live rules

- `live`에서는 디스크 캐시 없는 in-memory-only token provider를 허용하지 않는다.
- current KIS 정책상 토큰 재발급 제한이 있을 수 있으므로 발급 빈도를 적극적으로 억제한다.
- 운영자 명령으로만 `revoke` 또는 force refresh를 수행한다.

### Non-goal for v1

- 다중 호스트 간 토큰 공유 서비스
- `live`와 `paper` 간 자동 fallback

---

## HTTP Transport Plan

`reqwest::Client` 기반 async 구현으로 시작한다. 런타임은 `tokio` 하나로 통일한다.

### Why async first in Rust

- `reqwest` async client가 Tokio 기반 생태계와 가장 자연스럽다.
- SQLite persistence도 `sqlx` async transaction으로 맞출 수 있다.
- auth, rate limit, timeout, reconcile job을 같은 runtime에서 다룰 수 있다.
- WebSocket을 v1.1에 추가할 때 runtime 전환이 필요 없다.

### Transport responsibilities

- base URL 선택
- timeout class 적용
- connection pool 재사용
- safe retry 정책
- request/response metadata 로깅
- redaction
- request id 생성
- continuation 헤더/토큰 전달
- endpoint가 요구할 때 hashkey 헤더 부착 가능

### Endpoint classes

- `auth`: 별도 throttling, 짧은 retry budget
- `read_light`: quote, holiday, health
- `read_heavy`: balance, orders, fills, pagination 포함 조회
- `write_order`: place / amend / cancel

### Retry rules

자동 재시도 허용:

- DNS/connect timeout
- read timeout on idempotent read API
- 일시적 5xx
- 네트워크 연결 오류
- throttle 응답 이후 backoff가 가능한 read API

자동 재시도 제한:

- auth 발급 호출은 짧은 budget로 제한
- pagination 중간 page 실패는 전체 실패로 보고 caller가 재수행 여부 판단

자동 재시도 금지:

- 주문 POST가 이미 접수되었을 가능성이 있는 애매한 상태
- validation error
- auth credential error
- operator confirmation이 필요한 safety halt 상태

### Key write rule

- write path에서 socket write 시작 이후 결과가 불명확하면 blind retry하지 않는다.
- 해당 intent를 `reconciliation_needed`로 기록하고 recovery path로 넘긴다.

---

## Rate Limit and Backpressure Plan

공식 포털의 유량 공지는 수시로 바뀔 수 있으므로 rate limit은 코드 상수가 아니라 설정값이어야 한다.

### Strategy

- 환경별 quota 설정 (`paper`, `live`)
- endpoint group별 secondary quota 설정 (`auth`, `read_light`, `read_heavy`, `write_order`)
- 동일 `(environment, account)` 기준 공유 limiter 사용
- throttle code 또는 유량 초과 메시지 감지 시 jittered backoff
- repeated throttle 발생 시 temporary circuit을 열고 `BrokerRateLimit` 반환

### Rules

- background reconcile job이 write path quota를 고갈시키면 안 된다.
- pagination은 한 개의 논리 작업으로 보되, 내부 page 호출도 limiter를 통과한다.
- `paper` 한도가 더 낮을 수 있으므로 `paper`용 기본값을 더 보수적으로 둔다.

### Observability

- rate-limit sleep duration
- throttle count
- circuit open/close 이벤트
- per-operation effective quota class

---

## Continuation / Pagination Plan

잔고/주문/체결 조회는 단일 응답만으로 끝난다고 가정하면 안 된다.

### Requirements

- `tr_cont` header 처리
- `CTX_AREA_FK100`, `CTX_AREA_NK100` 등 continuation token 처리
- page aggregation의 순서/중복/종료 조건 검증
- endpoint별 max-depth 설정

### Design

- continuation은 각 endpoint 함수가 직접 구현하지 않고 `broker/kis/transport.rs`에서 공통 처리
- read API는 "단일 page raw 응답"과 "전체 aggregate 결과"를 모두 지원할 수 있게 설계
- reconcile에 쓰이는 조회는 항상 전체 aggregate 결과를 기준으로 판단

### Failure policy

- continuation token이 비정상 반복되면 `BrokerError::Invariant`
- pagination 중 일부 page만 성공한 결과를 조용히 반환하지 않는다.

---

## Error Taxonomy

최소 오류 체계:

```rust
#[derive(Debug, thiserror::Error)]
pub enum BrokerError {
    #[error("authentication failed: {message}")]
    Auth { message: String },
    #[error("permission denied: {message}")]
    Permission { message: String },
    #[error("rate limited: {message}")]
    RateLimit { message: String },
    #[error("validation failed: {message}")]
    Validation { message: String },
    #[error("temporary broker error: {message}")]
    Temporary { message: String },
    #[error("transport error: {message}")]
    Transport { message: String },
    #[error("market is closed: {message}")]
    MarketClosed { message: String },
    #[error("duplicate intent: {client_order_id}")]
    DuplicateIntent { client_order_id: ClientOrderId },
    #[error("order rejected: {message}")]
    OrderRejected { message: String },
    #[error("reconciliation is required: {client_order_id}")]
    ReconciliationNeeded { client_order_id: ClientOrderId },
    #[error("safety halt: {reason}")]
    SafetyHalt { reason: String },
    #[error("broker invariant violated: {message}")]
    Invariant { message: String },
}
```

### Mapping policy

- KIS HTTP status + body `rt_cd` + `msg_cd` + `msg1` + relevant headers를 함께 보고 분류한다.
- raw payload는 audit 저장 가능하도록 유지하되 민감정보는 redaction한다.
- user-facing 메시지는 내부 표준 메시지 사용한다.
- ambiguous write outcome는 항상 `BrokerError::ReconciliationNeeded` 또는 reconcile state로 승격한다.

---

## API Spec TDD Workflow

정확한 path / `tr_id` / domain은 구현 시 **당시 공식 KIS 문서**로 확정한다. 단, 문서를 읽고 바로 구현하지 않는다. 모든 KIS REST API는 아래 순서를 따른다.

1. 공식 문서에서 method, path, base URL, `tr_id`, required headers, request fields, response fields, pagination token, success/error body shape를 정리한다.
2. 해당 스펙을 `ApiSpecCase` 형태의 fixture로 만든다.
3. request builder test를 먼저 작성해 path, query/body, header, hashkey 필요 여부, redaction metadata를 검증한다.
4. response mapping test를 먼저 작성해 KIS raw payload가 내부 domain model로 변환되는지 검증한다.
5. error mapping test를 먼저 작성해 HTTP status, `rt_cd`, `msg_cd`, `msg1` 조합이 `BrokerError`로 분류되는지 검증한다.
6. pagination API라면 continuation 종료 조건, token 반복, 중간 page 실패 테스트를 먼저 작성한다.
7. write API라면 journal reserve-before-send, duplicate intent, ambiguous timeout, no blind retry, reconcile-needed 테스트를 먼저 작성한다.
8. 위 테스트가 실패하는 것을 확인한 뒤 최소 구현을 추가한다.
9. fake KIS server 기반 integration test를 통과시킨 뒤에만 `paper` smoke test를 실행한다.

### API spec fixture shape

```rust
pub struct ApiSpecCase {
    pub name: &'static str,
    pub method: &'static str,
    pub path: &'static str,
    pub tr_id: &'static str,
    pub endpoint_group: EndpointGroup,
    pub requires_hashkey: bool,
    pub request_fixture: &'static str,
    pub response_fixture: &'static str,
    pub expected_domain_fixture: &'static str,
}
```

이 구조체는 실제 구현 타입일 필요는 없지만, 테스트 fixture는 이 정보를 모두 표현해야 한다. API 스펙 변경이 생기면 fixture와 테스트가 먼저 바뀌어야 한다.

---

## Endpoint Groups and TDD Acceptance

### Group 1. Auth / Security

구현 대상:

- access token 발급
- 필요 시 token revoke
- 필요 시 hashkey 발급

구현 전 필수 테스트:

- token request의 URL, method, body, header가 스펙과 일치
- token response의 expiry가 `TokenSnapshot`으로 정확히 변환
- expiry margin 이전에는 cached token 재사용
- cache corruption, permission mismatch, fingerprint mismatch 시 재발급
- concurrent refresh에서 중복 발급 억제
- `401` 이후 1회 invalidate + retry
- auth 발급 throttle/backoff
- secret 값이 `Debug`/로그/audit에 노출되지 않음

### Group 2. Market data / Session

구현 대상:

- 국내주식 현재가 조회
- 장운영 / 휴장일 조회
- 필요 시 일봉/분봉은 v1.1

구현 전 필수 테스트:

- quote request path/query/header/`tr_id`가 스펙과 일치
- KIS numeric string이 `Decimal` 또는 수량 newtype으로 변환
- 빈 문자열/null-ish 필드 normalization
- 시장 상태/휴장일 응답이 `MarketSession`으로 변환
- read timeout, 일시적 5xx, throttle 응답이 idempotent retry 정책을 따름
- market closed 상태가 write gate에 반영

### Group 3. Account

구현 대상:

- 잔고 조회
- 예수금 / 주문가능금액 조회

구현 전 필수 테스트:

- balance/cash request의 account field masking과 redaction metadata 검증
- 보유 종목이 없을 때 empty position 처리
- multi-page balance response aggregation
- account summary와 position detail의 금액/수량 mapping
- 일부 page 실패 시 partial result를 반환하지 않음
- account metadata가 configured environment/account와 다르면 실패

### Group 4. Orders

구현 대상:

- 현금 매수
- 현금 매도
- 정정
- 취소

구현 전 필수 테스트:

- buy/sell/amend/cancel request body, header, hashkey, `tr_id`가 스펙과 일치
- `client_order_id`가 journal에 먼저 기록되지 않으면 broker submit 불가
- single-writer lock 실패 시 submit 불가
- live enable flag, read-only flag, account suffix mismatch 시 submit 불가
- socket write 이후 timeout/connection reset/5xx에서 blind retry 금지
- ambiguous outcome이 `reconcile_required`로 durable 기록
- duplicate `client_order_id`는 `BrokerError::DuplicateIntent`
- order accepted/rejected response가 `OrderSubmission` 또는 `BrokerError::OrderRejected`로 변환
- cancel/amend는 불완전한 `OrderReference`로 실행 불가

### Group 5. Order inquiry

구현 대상:

- 당일 주문 조회
- 미체결 조회
- 정정취소가능주문 조회
- 당일 체결 조회

구현 전 필수 테스트:

- order inquiry request의 path/query/header/`tr_id`가 스펙과 일치
- broker-native order id/orgno/exchange code가 `OrderReference`로 조립
- 주문 상태, 체결 상태, 부분 체결 상태 mapping
- continuation이 있는 주문/체결 조회의 전체 aggregation
- 같은 주문이 여러 page에 반복될 때 deduplication 또는 invariant error 처리
- raw status code를 해석하지 못하면 `Unknown(String)` variant로 보존

### Group 6. Recovery helpers

구현 대상:

- startup reconcile에 필요한 order/fill/account 조회 조합
- ambiguous place/cancel/amend recovery
- cross-day observation window recovery

구현 전 필수 테스트:

- journal unresolved record를 조회 API 결과와 broker-native id로 매칭
- `client_order_id` 기반 매칭
- 심볼/사이드/수량/시각 window 기반 보조 매칭
- match가 하나로 확정되지 않으면 `manual_intervention_required`
- 장 마감 직전 command 후 다음 영업일 startup reconcile
- unresolved command가 남아 있으면 live write block 유지

---

## Mapping Strategy

`kis-client/src/mapping/`은 가장 중요한 영역 중 하나다.

### Responsibilities

- KIS raw numeric strings -> `Decimal` / 정수 변환
- empty string / null-ish value normalization
- order state mapping
- side / order_type normalization
- money / quantity normalization
- UTC/KST datetime normalization
- broker-native id를 `OrderReference`로 조립

### Example mapping concerns

- 수량/가격이 문자열일 수 있음
- 일부 필드가 공백 문자열일 수 있음
- 환경/상품별 응답 shape 차이가 있을 수 있음
- 같은 의미의 상태가 환경별로 다른 코드로 표현될 수 있음

### Rule

- mapper는 관대하게 읽고
- 상위 도메인 모델은 엄격하게 반환
- 의미를 해석하지 못한 enum은 `Unknown(String)` variant로 승격하고 raw code를 남긴다.

---

## Order Reference Model

KIS의 후속 주문 조작은 단순 `order_id` 하나로 표현하기 어려울 수 있다.

```rust
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OrderReference {
    pub client_order_id: ClientOrderId,
    pub broker_order_id: Option<BrokerOrderId>,
    pub broker_order_orgno: Option<BrokerOrderOrgNo>,
    pub exchange_code: Option<ExchangeCode>,
    pub account_key: AccountKey,
    pub environment: BrokerEnvironment,
}
```

### Rules

- `place_order()`는 후속 cancel/amend에 필요한 broker-native 식별자를 최대한 담은 `OrderSubmission`을 반환한다.
- `cancel_order()`와 `amend_order()`는 반드시 `OrderReference`를 받는다.
- `OrderReference`가 불완전하면 write path 전에 reconcile 또는 조회를 먼저 수행한다.

---

## Order Lifecycle Handling

주문 관련 내부 상태를 분리한다.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrokerCommandStatus {
    DecisionCreated,
    CommandRecorded,
    ReadyToSend,
    Submitting,
    SubmittedAckUnknown,
    Accepted,
    PartiallyFilled,
    Filled,
    CancelRequested,
    Cancelled,
    Rejected,
    Failed,
    ReconcileFailed,
    SafetyHalted,
}
```

### Key rules

- `CommandRecorded` 전에는 write network call 금지
- `Submitting` 이후 프로세스가 죽어도 startup reconcile로 복구 가능해야 함
- `SubmittedAckUnknown` 상태에서는 동일 command 재전송 금지
- 정정/취소 전에는 정정취소가능수량 또는 원주문 상태 확인을 수행

---

## Durable Command Journal and Idempotency Plan

KIS 자체 idempotency만으로는 충분하지 않을 수 있으므로 앱 레벨에서 보강한다.

### Strategy

- 각 broker write command에 `client_order_id` 생성
- command type은 `place`, `cancel`, `amend`로 구분
- live에서는 DB에 write command를 먼저 기록
- `client_order_id` unique constraint 보장
- 브로커 제출 전후 상태를 durable state transition으로 저장
- 응답 불명확 시 reconciliation 수행

### Required transition order

1. command reserve
2. single-writer validation
3. preflight checks
4. mark `submitting`
5. broker submit
6. ack 기록 또는 `reconcile_needed` 기록

### Stored fields

- `client_order_id`
- `parent_client_order_id`
- `command_type`
- `decision_id`
- `environment`
- `account_key`
- `symbol`
- `side`
- `quantity`
- `limit_price`
- `order_type`
- `requested_at`
- `request_id`
- `transport_send_started_at`
- `broker_order_id`
- `broker_order_orgno`
- `target_broker_order_id`
- `target_broker_order_orgno`
- `cancel_or_amend_quantity`
- `amend_price`
- `exchange_code`
- `last_known_state`
- `raw_ack_code`
- `reconcile_required`
- `last_reconciled_at`
- `trading_day`
- `broker_observation_window_start`
- `broker_observation_window_end`

### Live rules

- live write path는 journal 없이는 실패
- process crash 이후 `reconcile_required` 레코드가 남아 있으면 startup 시 우선 복구
- 동일 `client_order_id`를 새 주문으로 재사용하지 않는다.
- 취소/정정 command도 주문 제출 command와 동일하게 durable transition을 거친다.
- 원주문과 후속 취소/정정 command는 `parent_client_order_id`로 연결한다.
- journal 상태 전이는 SQLite transaction 안에서 atomic하게 반영한다.

---

## Reconciliation Plan

`reconciliation`은 예외 처리 부가 기능이 아니라 live write path의 핵심이다.

### Trigger cases

- 주문 submit 후 timeout / connection reset / 5xx 등으로 결과 불명확
- cancel/amend submit 후 timeout / connection reset / 5xx 등으로 결과 불명확
- process crash 후 재시작
- journal state와 broker 조회 상태 불일치
- cancel/amend 후 terminal state 불명확
- operator 수동 실행

### Sources of truth

- 당일 주문 조회
- 최근 N영업일 주문 조회
- 미체결/정정취소가능주문 조회
- 당일/최근 N영업일 체결 조회
- 잔고/현금 변화

### Matching rules

- 1순위: broker-native id로 직접 매칭
- 2순위: `client_order_id` 기반 내부 매칭
- 3순위: 심볼/사이드/수량/시각 window 기반 보조 매칭
- 취소/정정 command는 원주문 reference와 target broker id를 함께 사용해 매칭

### Lookback policy

- 기본 reconcile window는 최근 3영업일
- command별 `trading_day`와 broker observation window를 journal에 저장
- 장 마감 직전 write 후 다음 영업일 재시작하는 경우를 필수 복구 케이스로 다룬다.

### Result categories

- `confirmed_submitted`
- `confirmed_amended`
- `confirmed_rejected`
- `confirmed_cancelled`
- `confirmed_filled`
- `still_unknown`
- `manual_intervention_required`

### Hard rules

- `still_unknown` 상태에서 자동 재주문 금지
- startup reconcile 후에도 `manual_intervention_required`가 남으면 live write 차단

---

## Startup Recovery Sequence

live-ready client는 재시작 직후 바로 주문부터 보내면 안 된다.

### Startup order

1. 설정 로드
2. environment/account 검증
3. single-writer lock 획득
4. SQLite schema/migration 상태 검증
5. token health check
6. unresolved journal records 로드
7. reconcile 수행
8. unresolved 상태가 남아 있으면 live write 비활성화
9. market session/holiday 확인
10. 새 write 허용 여부 판단
11. 이후에만 normal operation 진입

### Recovery rule

- reconcile read path는 장외/휴장일에도 실행 가능해야 한다.
- market session/holiday check는 새 write를 허용할지 판단하는 gate이지, 기존 미확정 command 복구를 막는 gate가 아니다.

### Failure policy

- startup reconcile 실패는 경고로 넘기지 않는다.
- live에서는 unresolved 상태가 남으면 `BrokerError::SafetyHalt`를 반환한다.

---

## Live Safety Controls

### Required controls

- `enable_live_trading=true` 없이는 write 불가
- `read_only=true`이면 write 강제 차단
- `confirmed_account_suffix` mismatch 시 차단
- live 계좌 allowlist 지원
- operator panic stop flag는 product runtime(`agent`) 책임으로 분리
- safety halt 상태는 프로세스 재시작만으로 자동 해제하지 않는다.

### Product operator commands

`kis-client`는 CLI를 제공하지 않는다. 운영자 명령은 main DB와 product runtime을
소유하는 `agent` crate에서 제공하고, 필요한 broker 상태는 `TradingGateway`와
`KisGateway`를 통해 조회한다.

---

## Logging and Audit Hooks

KIS client는 다음 정보를 남길 수 있어야 한다.

### For every request

- internal request id
- operation name
- environment
- account key suffix
- endpoint group
- KIS `tr_id`
- request timestamp
- latency
- HTTP status
- broker result code
- retry count
- rate-limit sleep
- sanitized request metadata

### For pagination

- `tr_cont`
- continuation token hash 또는 존재 여부
- page index
- aggregated page count

### For command-specific events

- `client_order_id`
- command type
- symbol
- side
- quantity
- order_type
- broker_order_id if available
- broker_order_orgno if available
- lifecycle state transition
- reconcile attempt number

### For sensitive data

절대 로그 금지 또는 마스킹:

- app key
- app secret
- access token
- full account number
- raw continuation token 원문
- webhook URL

### Audit storage rule

- raw request/response body를 남길 수는 있으나 민감정보 redaction 후 저장
- broker write command의 state transition은 journal과 별개로 audit trail에서 추적 가능해야 함

---

## Testing Plan

TDD는 이 프로젝트의 최우선 구현 원칙이다. 각 기능은 먼저 실패하는 테스트를 만들고, 그 테스트를 통과시키는 최소 구현을 추가한 뒤, live-ready invariant를 깨지 않는 범위에서 리팩터링한다.

### TDD rules

- production module을 만들기 전에 해당 behavior의 unit/integration/recovery test 위치를 먼저 정한다.
- 정상 케이스보다 실패 케이스를 먼저 작성한다.
- KIS API 하나를 구현할 때는 `api_spec_contract` 테스트를 먼저 작성한다.
- broker write path는 ambiguous outcome, crash recovery, duplicate intent, live write block 테스트가 없으면 구현 완료로 보지 않는다.
- KIS 실제 호출이 필요한 테스트는 기본 test suite에서 제외하고, feature flag 또는 명시적 environment variable이 있을 때만 실행한다.
- 버그를 발견하면 먼저 재현 테스트를 추가한 뒤 수정한다.

### API spec contract tests

- 모든 endpoint는 request builder contract test를 가져야 한다.
- 모든 endpoint는 success response fixture와 domain model expected fixture를 가져야 한다.
- 모든 endpoint는 KIS error response fixture를 최소 1개 이상 가져야 한다.
- pagination endpoint는 continuation fixture를 최소 2 page 이상 가져야 한다.
- write endpoint는 ambiguous transport failure fixture와 journal transition expected fixture를 가져야 한다.
- live-only guardrail은 fake server 호출 전 단계에서 차단되는 테스트를 가져야 한다.
- 공식 KIS 문서가 바뀌면 구현보다 spec fixture와 contract test를 먼저 갱신한다.

### Unit tests

- token expiry / local cache / lock behavior
- local token cache permission / corruption handling
- header builder
- rate limiter / backoff
- pagination aggregator
- response mapper
- error mapper
- order state mapping
- order reference mapping
- command journal transition mapping
- live guardrail checks

### Integration tests

- KIS `paper` 환경 smoke tests
- 현재가 조회
- 잔고 조회
- multi-page balance/order query
- 소량 `paper` order
- order inquiry and cancel
- ambiguous write fault injection 후 reconcile

### Recovery tests

- submit 직후 process crash 시뮬레이션
- cancel/amend 직후 process crash 시뮬레이션
- 장 마감 직전 command 후 다음 영업일 startup reconcile
- startup reconcile 후 state recovery
- unresolved command가 남았을 때 live write block 검증

### Fixture strategy

- 공식 샘플 응답 기반 JSON fixture 저장
- 실환경에서 수집한 sanitized payload도 fixture로 축적
- continuation / throttle / rejection / partial fill 케이스를 fixture로 별도 보관

### Rust verification commands

- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all-features`
- `cargo sqlx prepare --check` if SQLx query macros are used

### Operational test note

- KIS 포털의 testbed는 실제 요청이 접수될 수 있으므로 live credential로 안전하지 않은 수동 테스트를 기본 경로로 삼지 않는다.

---

## Delivery Phases

Status marker:

- `[x]`: implemented and covered by local tests
- `[~]`: partially implemented; remaining scope is still open
- `[ ]`: not implemented yet

### Phase 1. Foundations

- [x] TDD 규칙과 test harness 확정
- [x] fake KIS server와 API spec fixture loader 확정
- [x] workspace `Cargo.toml`, `kis-client/Cargo.toml`, `kis-client/src/lib.rs` skeleton 생성
- [x] `broker::base` trait와 domain model 테스트 작성 후 정의
- [x] `broker::journal` SQLite schema와 journal trait 테스트 작성 후 정의
- [x] `OrderReference`, `OrderSubmission`, `CancelOrderRequest` 테스트 작성 후 확정
- [x] environment / live flags / single-writer model 테스트 작성 후 정의

### Phase 2. Auth + Transport + Limiter

- [x] auth API spec fixture와 contract test 작성
- [x] token cache, expiry, corruption, refresh race 테스트 작성 후 `broker/kis/auth.rs` 구현
- [x] transport request builder, bounded read retry, redaction, timeout 테스트 작성 후 `broker/kis/transport.rs` 구현
- [x] base URL / timeout / retry / request id 정책 테스트 작성 후 구현
- [x] `ping()` 또는 lightweight health check 테스트 작성 후 구현

### Phase 3. Read APIs + Pagination

- [x] 현재가 조회 spec fixture와 mapper test 작성 후 구현
- [x] 잔고 조회 spec fixture와 mapper test 작성 후 구현
- [x] 주문가능금액 조회 spec fixture와 mapper test 작성 후 구현
- [x] 장운영/휴장일 조회 spec fixture와 mapper test 작성 후 구현
- [x] pagination success/failure/loop-detection 테스트 작성 후 `broker/kis/transport.rs`에 구현
- [x] mapper 실패 케이스 테스트를 먼저 작성하고 구현

### Phase 4. Durable Command Journal + Reconciliation

- [x] migration contract test 작성 후 `agent/migrations/0001_command_journal.sql` 작성
- [x] state transition, unique constraint, parent/child integrity 테스트 작성 후 `agent::storage::SqliteCommandJournal` 구현
- [x] ambiguous order handling 테스트 작성 후 구현
- [x] broker-native id / client id / business-day lookback 테스트 작성 후 `broker/kis/reconcile.rs` 구현
- [x] startup recovery success/halt 테스트 작성 후 flow 구현

### Phase 5. Order APIs

- [x] 현금 매수/매도 spec fixture, journal-before-submit, ambiguous timeout 테스트 작성 후 구현
- [x] 주문 조회 spec fixture와 `OrderReference` mapping 테스트 작성 후 구현
- [x] 취소/정정 spec fixture와 incomplete reference/preflight 테스트 작성 후 구현
- [x] order lifecycle mapping 테스트 작성 후 구현
- [x] cancel/amend preflight check 테스트 작성 후 구현

### Phase 6. Live Guardrails

- [x] live enable flag / account allowlist / read-only mode 테스트 작성 후 `broker/kis/safety.rs` 구현
- [x] startup halt / live guardrail 테스트 작성 후 보강
- [x] guardrail 실패 시 fake server가 호출되지 않는 테스트 추가

### Phase 7. Runtime Integration

- [x] library configuration, broker ping, market quote, portfolio/account read
      tests 작성 후 연결
- [x] doctor/reconcile/journal 운영 책임을 product runtime CLI 경계로 이동
- [x] redaction/audit logging 테스트 작성 후 연결

### Phase 8. Live Certification

아래 항목은 fake server/local test로 완료 처리하지 않는다. 실제 KIS paper/live
자격증명, 계좌, 운영자 절차, 그리고 실거래 전환 승인 흐름이 있어야만 `[x]`로
바꿀 수 있다.

- [ ] `paper` E2E 검증 완료
- [ ] read-only live smoke 검증
- [ ] operator recovery drill 수행
- [ ] live enable checklist 통과
- [ ] live 전환 전 모든 API spec contract test와 recovery test 통과

---

## Definition of Done for v1

아래가 되면 KIS client v1은 **paper usable + live-ready** 상태로 본다.

현재 로컬 fake-server 기반으로 검증 가능한 구현 항목은 완료되었다. 남은 unchecked
항목은 실제 KIS paper 인증/주문과 live certification을 수행해야 완료된다.

1. [ ] `paper` 환경에서 인증 성공
2. [x] 현재가 조회 가능
3. [x] 잔고 및 현금 조회 가능
4. [x] continuation이 있는 조회를 완전 수집 가능
5. [ ] `paper` 매수/매도 주문 가능
6. [x] 주문 조회 및 취소/정정 가능
7. [x] 후속 write가 `OrderReference` 기반으로 동작
8. [x] 내부 도메인 모델로 일관되게 반환
9. [x] 주요 오류가 내부 오류 enum으로 정규화됨
10. [x] 네트워크 애매 상태에서 재주문 대신 reconcile 경로로 진입
11. [x] durable command journal 기반 state transition 보유
12. [x] place/cancel/amend 모두 journal 기반으로 복구 가능
13. [x] startup 시 unreconciled command를 자동 복구 시도
14. [x] 최근 N영업일 기반 cross-day reconcile 가능
15. [x] unresolved 상태가 남으면 live write 차단
16. [x] rate limit / backpressure 정책이 구현됨
17. [x] live enable flag / read-only mode / account guardrail 구현
18. [x] 핵심 기능이 TDD로 구현되어 regression test를 보유
19. [x] 구현된 모든 KIS endpoint가 API spec contract test를 보유
20. [x] 구현된 모든 write endpoint가 journal/reconcile/safety 실패 케이스 테스트를 보유
21. [x] 최소 smoke/integration/recovery test 보유
22. [x] `kis-client`는 library-only이고, DB-backed doctor/reconcile/journal
        운영 명령은 main DB를 소유하는 product runtime 경계에서 제공한다.
23. [x] `cargo fmt`, `cargo clippy`, `cargo test`가 통과

---

## Live Enable Checklist

실제 `live` write를 열기 전 아래를 만족해야 한다.

1. `paper` E2E가 안정적으로 통과
2. command journal persistence가 SQLite durable storage에 연결됨
3. single-writer lock이 실제 배포 환경에서 검증됨
4. startup recovery drill을 최소 1회 수행
5. cancel/amend ambiguous recovery drill을 최소 1회 수행
6. cross-day recovery drill을 최소 1회 수행
7. operator가 reconcile 명령 사용법을 숙지
8. read-only live smoke로 계좌/세션/장운영 정보 확인
9. panic stop과 resume 절차 검증
10. 로그 redaction과 감사 저장 위치 점검
11. SQLite backup/restore 절차를 실제 DB 파일로 검증
12. 모든 live 관련 write API의 ambiguous outcome/reconcile 테스트 통과
13. 모든 구현 API의 spec fixture가 최신 공식 문서 기준으로 검토됨

---

## Recommended Immediate Next Steps

1. [x] workspace `Cargo.toml`, `kis-client/src/lib.rs`, `kis-client/tests/` skeleton과 TDD test harness를 먼저 만든다.
2. [x] fixture 경로와 fake KIS server 전략을 확정한다.
3. [x] `kis-client/src/broker/base.rs`에 브로커 공통 trait와 domain model 테스트 먼저 작성
4. [x] `kis-client/src/config.rs`에 KIS credential / environment / live guardrail schema 테스트 먼저 작성
5. [x] `agent/migrations/0001_command_journal.sql`과 durable journal 계약 테스트 먼저 작성
6. [x] KIS 공식 샘플 기준으로 auth + quote + balance + order + continuation spec fixture 작성
7. [x] auth API spec contract test부터 작성하고 `kis-client/src/broker/kis/auth.rs` 구현
8. [x] transport request/read retry/redaction contract test부터 작성하고 `kis-client/src/broker/kis/transport.rs` 구현
9. [x] read-only API와 pagination 테스트를 먼저 작성한 뒤 연결
10. [x] 그 다음 command journal + reconcile 테스트를 먼저 작성한 뒤 구현
11. [x] 마지막에 order API와 live guardrail 테스트를 먼저 작성한 뒤 연결

---

## Recommended Build Order

가장 좋은 순서는 이거다.

1. [x] test harness / fixtures / fake KIS server
2. [x] domain model / config / environment model tests, then implementation
3. [x] SQLite migration / journal schema tests, then implementation
4. [x] auth tests, then implementation
5. [x] transport + limiter/read retry tests, then implementation
6. [x] quote tests, then implementation
7. [x] positions / cash / market session tests, then implementation
8. [x] pagination-aware list orders / fills tests, then implementation
9. [x] command journal tests, then implementation
10. [x] place order tests, then implementation
11. [x] cancel / amend tests, then implementation
12. [x] reconciliation tests, then implementation
13. [x] live guardrail tests, then implementation
14. WebSocket, if needed after v1

이 순서를 지키면 읽기 전용 경로에서 KIS shape를 테스트로 충분히 고정한 뒤 durable write path로 넘어갈 수 있다. 어떤 API도 spec fixture, request builder test, response mapper test, error mapper test 없이 구현하지 않는다.
