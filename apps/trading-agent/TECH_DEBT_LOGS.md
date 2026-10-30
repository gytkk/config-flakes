# Technical Debt Logs

## 2026-05-09 Devil's Advocate Review

Reviewed HEAD: `d56c468` (`fix: refresh universe cache before expiry`)

Verdict: `reject`  
Confidence: `8/10`  
Mode: `full`

Summary: 라이브 주문 안전성 관점에서 ship-blocker가 있다.

### Top Concerns

1. **critical** - `trade-engine/src/storage/sqlite_journal.rs:267`

   `submitting` 상태의 broker command가 미해결로 잡히지 않는다.
   `mark_submitting()`은 broker write 직전 `Submitting`을 durable하게 남기지만,
   `list_unreconciled()`는 `submitted_ack_unknown`/`reconcile_failed`만 조회한다.
   프로세스가 이 사이에 죽으면 startup doctor와 live write guard가 새 주문을
   허용할 수 있다.

   Recommendation: `submitting` 또는 stale in-flight 상태를 unresolved set에
   포함하거나 startup에서 `submitted_ack_unknown`으로 승격한 뒤 reconciliation을
   강제한다.

2. **major** - `trade-engine/src/risk/mod.rs:73`

   sell proposal이 보유 수량을 초과하는지 risk gate가 확인하지 않는다.
   `RiskEvaluationContext`에는 held quantity가 없고, runtime은 projected notional만
   넘긴다. 외부 advisor가 미보유 종목 매도나 초과 매도를 제안하면 deterministic
   risk가 막지 못하고 broker reject에 의존한다.

   Recommendation: `SellQuantityExceedsPosition` 같은 risk block reason을 추가하고,
   runtime/final execution risk context에 current position quantity를 포함한다.

3. **major** - `trade-engine/src/runtime/mod.rs:3175`

   `DecisionConfig`/`CommandDecisionProvider`는 config와 테스트가 있지만 runtime
   decision path에서 사용되지 않는다. `settings.decision.validate()`만 하고 실제
   provider 선택은 `advisor.provider` 기준이다. 운영자는 `[decision] provider =
   "command"`가 실행된다고 오해할 수 있다.

   Recommendation: V4 기준 provider boundary를 하나로 정리하거나 runtime에
   `DecisionProvider`를 실제 연결한다. 사용하지 않을 config surface는 제거하거나
   명확히 deprecated 처리한다.

4. **major** - `trade-engine/src/storage/sqlite_journal.rs:21`

   async journal 구현이 `rusqlite::Connection`을 `tokio::sync::Mutex` 뒤에서 직접
   호출한다. `BlockingMainDb`는 `spawn_blocking` 경계를 쓰는데, live order journal
   path는 async worker를 blocking I/O에 노출한다.

   Recommendation: `SqliteBrokerCommandJournal`도 `BlockingMainDb`와 같은 blocking
   boundary로 통일한다.

5. **minor** - `trade-engine/src/advisor.rs:435`

   command provider stdout이 unbounded이다. `wait_with_output()` 뒤 전체 stdout을
   `String`으로 만들고 audit에 저장한다. stderr cap은 있지만 stdout cap이 없어
   잘못된 sidecar가 메모리와 DB를 압박할 수 있다.

   Recommendation: external command stdout cap을 config화하고 초과 시
   rejected/failed audit으로 남긴다. 같은 패턴이 `universe`와 legacy
   `decision` provider에도 있는지 함께 정리한다.

### Pass Results

#### Architecture

- **critical** - `submitting` journal recovery gap은 live invariant 위반이다.
- **major** - decision/advisor provider 경계가 중복되고 한쪽은 dead path다.

#### Maintainability

- **major** - SQLite journal만 `BlockingMainDb` 패턴에서 벗어난다.
- **minor** - external command stdout cap이 없어 sidecar 실패 모드가 과하게 넓다.

#### Edge Cases

- **major** - sell quantity와 position availability가 타입이나 risk context로
  강제되지 않는다.

### Verification

Passed:

- `cargo check --workspace --no-default-features`
- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all-features`
- `cargo doc --no-deps --all-features`

Repository state at review time: clean.

### Not Verified

- 실제 KIS live 환경 동작
- 장중 네트워크 단절/프로세스 kill 재현 테스트
- 장시간 sidecar stdout 폭주 부하 테스트
- SQL migration downgrade/legacy DB 변형 케이스
