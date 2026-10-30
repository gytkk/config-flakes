# KIS Client

`trading-agent` workspace 안에서 사용하는 한국투자증권(KIS)용 Rust broker
adapter crate입니다.

이 crate의 책임은 결정론적인 broker client 동작으로 제한합니다. KIS REST
contract, auth/token 처리, HTTP transport와 redaction, live write guardrail,
주문 journal contract, reconciliation을 다룹니다. 전략 로직, agent decision
layer, portfolio policy, operator CLI, Discord reporting은 포함하지 않습니다.

현재 실제 API broker adapter가 KIS뿐이므로 broker trait, typed domain model,
`BrokerError`, durable command journal trait도 이 crate가 함께 소유합니다.
다만 production DB 연결, migration, runtime orchestration은 이 crate의 책임이
아닙니다. 운영 runtime은 `BrokerCommandJournal` 구현체를 만들고
`KisBrokerClient`에 주입해야 합니다.

사용자-facing 인증 설정은 `app_key`와 `app_secret`입니다. REST 호출에 필요한
access token은 `KisTokenProvider`가 발급하고, 필요하면 `token_cache_path`에
owner-only 권한의 파일 cache로 저장합니다. `KisClientBuilder`로 만든 transport는
요청 직전에 provider에서 token을 다시 조회하므로 refresh margin 안에 들어온 token은
자동으로 갱신되고, KIS가 auth 실패를 반환하면 cache를 무효화한 뒤 한 번만 새 token으로
재시도합니다.

## 패키지

- Cargo package: `trading-agent-kis-client`
- Library crate: `trading_agent_kis_client`

## Features

- `default`: optional feature 없이 public library API만 빌드합니다.
- `internal-test-support`: KIS protocol/mapping/reconciliation internals를
  contract test 전용 `internal::*` module로 노출합니다.

## 공개 API

주요 public entrypoint는 crate root 또는 module 경로에서 사용할 수 있습니다:

- `KisBrokerClient`, `KisClientBuilder`
- `KisAuthConfig`, `KisCredential`, `KisTokenProvider`
- `BrokerClient`, `BrokerError`
- `domain::*`: `AccountKey`, `Symbol`, `CreateOrderRequest`,
  `OrderReference`, `TradingDay`, `OverseasStock`,
  `OverseasStockBalanceRequest`, `OverseasStockBuyingPowerRequest`,
  `OverseasStockOrderRequest`, `OverseasStockOrderHistoryRequest` 등 typed
  domain model
- `journal::*`: `BrokerCommandJournal`, `BrokerCommandRecord`,
  `BrokerCommandStatus` 등 durable command journal contract
- `transport::*`: `KisTransport`, `KisTransportConfig`, `KisEndpoint`,
  `KisRateLimitConfig`, shared rate limiter 등 low-level KIS HTTP 구성 요소
- `safety::*`: live single-writer lock

`mapping`, `protocol`, `reconcile`은 기본적으로 내부 모듈입니다. 테스트에서
contract를 직접 검증할 때만 `internal-test-support` feature로 접근합니다.

## 구조

```text
kis-client/
  Cargo.toml
  README.md
  src/
    lib.rs
    config.rs                  # live write runtime guardrail config
    reconcile.rs               # journal command과 broker snapshot reconciliation
    safety.rs                  # live single-writer lock
    auth/
      mod.rs                   # public auth re-export
      credential.rs            # KIS app key/secret wrapper와 redaction
      config.rs                # token provider 설정
      request.rs               # OAuth token issue request contract
      token.rs                 # access token, token response, snapshot
      cache.rs                 # owner-only token file cache
      lock.rs                  # cross-provider token refresh file lock
      provider.rs              # token issue/cache/refresh orchestration
    client/
      mod.rs                   # KisBrokerClient struct, constructors, BrokerClient impl
      builder.rs               # credential에서 token/transport/client 조립
      account.rs               # balance, cash, buying power reads
      market.rs                # quote, holiday/session, health read
      orders.rs                # order inquiry, place/cancel/amend writes
      overseas.rs              # overseas quote/balance reads, order writes, order/fill inquiry
      recovery.rs              # ambiguous write reconciliation and startup recovery
      guardrails.rs            # write preflight, live gate, ambiguous write marking
      helpers.rs               # KIS account/date/body helper functions
    core/
      mod.rs
      contract.rs              # broker trait
      domain.rs                # typed broker domain model
      error.rs                 # BrokerError
      journal.rs               # durable command journal trait and records
    mapping/
      mod.rs
      common.rs
      account.rs               # KIS account/balance response -> domain
      market.rs                # KIS quote/session response -> domain
      orders.rs                # KIS order/fill/write response -> domain
      overseas_stock.rs        # KIS overseas response -> domain
    protocol/
      mod.rs
      domestic_stock/
        mod.rs
        account.rs             # account endpoint metadata
        market.rs              # market endpoint metadata
        orders.rs              # order endpoint metadata and tr_id rules
      overseas_stock/
        mod.rs
        account.rs             # overseas account read endpoint metadata
        market.rs              # overseas market read endpoint metadata
        orders.rs              # overseas order endpoint metadata and tr_id rules
    transport/
      mod.rs                   # KisTransport execution flow
      config.rs                # base URL, timeout, retry, rate-limit config
      endpoint.rs              # endpoint metadata and retry class
      request.rs               # prepared request and request audit record
      response.rs              # raw response and response audit record
      rate_limit.rs            # endpoint-group limiter and throttle circuit
      error.rs                 # KIS HTTP/body error classification
      audit.rs                 # header/query/body redaction helpers
      hashkey.rs               # write payload hashkey wrapper
  tests/
    support/
    config_guardrails.rs
    domain.rs
    kis_account_read.rs
    kis_api_contract.rs
    kis_auth.rs
    kis_balance.rs
    kis_market_session.rs
    kis_order_inquiry.rs
    kis_order_write.rs
    kis_overseas_read.rs
    kis_quote.rs
    kis_reconcile.rs
    kis_transport.rs
    live_guardrails.rs
    public_api.rs
```

## 안전 불변식

live write는 관련 guardrail을 모두 통과하지 않으면 차단됩니다:

- live trading이 명시적으로 활성화되어야 합니다.
- read-only mode가 꺼져 있어야 합니다.
- account suffix 확인과 allowlist 검사를 통과해야 합니다.
- broker submit 전에 command intent가 durable하게 기록되어야 합니다.
- 획득한 single-writer lock이 write 대상 `(environment, account, market_scope)`와
  일치해야 합니다.
- 여러 process가 같은 KIS app key를 사용하면 shared rate limiter를 통해 request
  slot과 throttle circuit을 공유해야 합니다.
- unresolved journal record가 reconciliation으로 정리되어 있어야 합니다.
- 국내 live write는 KIS open-day gate와 KRX regular-session time gate를 통과해야
  합니다. 해외 live write는 별도 해외 market-session gate가 구현되기 전까지
  enforced mode에서 차단됩니다.
- ambiguous write outcome은 blind retry 대신 reconciliation 경로로 이동해야
  합니다.

secret, access token, account number, hashkey는 `Debug`, log, fixture, audit
output에 노출되면 안 됩니다.

## 검증

repository root에서 실행합니다:

```sh
cargo fmt --check
git diff --check
cargo check --workspace --no-default-features
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo doc --no-deps --all-features
```

특정 crate test file만 실행할 수도 있습니다:

```sh
cargo test -p trading-agent-kis-client --features internal-test-support --test kis_order_write
cargo test -p trading-agent-kis-client --features internal-test-support --test kis_overseas_order_write
cargo test -p trading-agent-kis-client --features internal-test-support --test kis_overseas_order_inquiry
```

## 사용 경계

이 crate는 library-only입니다. 운영자 CLI, reporting, agent decision runtime은
상위 runtime에서 구현하고, 이 library에는 typed request와 journal 구현체만
주입해야 합니다. agent decision layer가 직접 order write를 호출하지 않도록,
risk/policy gate가 `BrokerClient` 앞단에서 최종 권한을 가져야 합니다.
