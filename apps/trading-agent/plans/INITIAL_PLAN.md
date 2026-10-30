# Trading Agent Plan

## Goal

국내 증권사 API를 통해 실제 거래가 가능하되, 전체 시스템은 다음 원칙을 따르는 트레이딩 에이전트를 만든다.

- 실행과 기록은 구조화된 스크립트가 담당한다.
- 매매 판단은 agent가 담당한다.
- agent 출력은 자유 텍스트가 아니라 구조화된 데이터로 제한한다.
- 모든 주문은 리스크 정책과 검증 계층을 통과해야 한다.
- 거래 기록과 현재 상태를 투명하게 볼 수 있어야 한다.
- 최소 운영 도구는 CLI와 Discord report다.

---

## Core Product Shape

이 프로젝트는 "완전 자율 agent"가 아니라 아래 형태를 목표로 한다.

1. **Deterministic system**
   - 브로커 연동
   - 시세/잔고/주문/체결 조회
   - 리스크 검증
   - 주문 실행
   - 로깅
   - 스케줄링

2. **Agent reasoning layer**
   - 시장 상태, 포지션, 최근 거래, 규칙 요약을 입력으로 받는다.
   - `buy / sell / hold` 같은 구조화된 결정을 반환한다.
   - 실제 주문 권한은 직접 가지지 않는다.

3. **Transparency layer**
   - CLI로 현재 상태, 최근 판단, 체결, 손익을 확인한다.
   - Discord로 주요 이벤트와 정기 리포트를 받는다.

---

## Language and Framework Decision

### KIS SDK Situation

현재 기준으로 KIS는 전형적인 의미의 "공식 범용 SDK"보다는,
**Open API 포털 + Python 샘플 코드 + GitHub 예제 저장소** 중심에 가깝다.

즉, 시작점은 SDK 의존 설계보다 아래처럼 보는 것이 맞다.

- 브로커는 **REST API + 필요 시 WebSocket** 기반으로 직접 래핑
- 공식 샘플은 참고 자료로 사용
- 애플리케이션은 자체 `KIS client` 계층을 만든다

이 프로젝트에서는 공식 샘플을 그대로 앱 구조의 중심에 두지 말고,
**우리 쪽의 안정적인 브로커 추상화 계층**을 먼저 만드는 것이 중요하다.

---

### Python vs Rust

결론부터 말하면, **1차 MVP는 Python을 추천**한다.

#### Python을 추천하는 이유

1. **KIS 샘플/자료와 궁합이 가장 좋다**
   - 공식 GitHub 예제가 Python 중심이다.
   - 초기 인증, 주문, 잔고, 시세 검증 속도가 빠르다.

2. **agent integration이 훨씬 쉽다**
   - OpenAI/Codex 연동, JSON schema validation, prompt 실험, CLI 제작이 빠르다.

3. **운영 도구를 붙이기 쉽다**
   - SQLite, Typer, Rich, APScheduler, Pydantic 조합이 빠르고 안정적이다.

4. **paper trading 단계까지는 성능 병목이 거의 아니다**
   - 이 프로젝트는 HFT가 아니라 판단/검증/기록/리포트 중심 구조다.
   - 병목은 네트워크와 브로커 응답이지 Python 실행속도가 아니다.

#### Rust를 고려할 이유

Rust는 아래 상황에서 2차 선택지로 좋다.

- 장기적으로 서비스 안정성과 정적 타입 안정성을 극도로 중시할 때
- 실행 바이너리 단일 배포가 중요할 때
- 고빈도, 고동시성, 낮은 런타임 오류 확률이 중요할 때
- 브로커 client를 별도 service로 떼고 싶을 때

#### 지금 Rust를 메인으로 잡지 않는 이유

- KIS 검증 속도가 느려질 가능성이 높다.
- 샘플/문서/커뮤니티 자산 활용성이 떨어진다.
- agent 연동과 운영 도구 개발 속도가 낮아진다.
- MVP 목표가 "판단 가능한 안전한 자동매매 운영 구조"이기 때문에 Rust의 장점이 아직 덜 중요하다.

---

### Recommended Stack for MVP

**언어:** Python 3.11+

**핵심 라이브러리 제안:**
- CLI: `typer`
- 데이터 모델/검증: `pydantic`
- HTTP: `httpx`
- DB: `sqlalchemy` + SQLite
- 리치 CLI 출력: `rich`
- 스케줄링: 우선 `cron` 또는 systemd timer, 필요 시 `APScheduler`
- 설정 관리: `pydantic-settings` 또는 YAML + env 조합
- 테스트: `pytest`

보조 선택:
- WebSocket: `websockets` 또는 `httpx/ws` 계열
- Discord report: webhook 우선, 이후 bot 확장 가능

---

### Framework Stance

웹 프레임워크는 초기 MVP에 **필수 아님**.

우선순위:
1. 라이브러리형 core
2. CLI
3. 반복 실행 루프
4. cron/systemd 기반 정기 작업
5. 필요 시 나중에 web dashboard

즉 처음부터 FastAPI 서버를 중심에 두기보다는,
**"import 가능한 코어 + 얇은 CLI/잡 실행기"** 구조가 더 적합하다.

FastAPI는 아래 조건이 생기면 도입 검토:
- 웹 대시보드가 필요해질 때
- 외부 제어 API가 필요할 때
- 여러 프로세스/서비스가 상태를 공유할 때

---

## Recommended Architecture

### 1. Broker Adapter Layer

역할:
- 국내 증권사 API 래핑
- 인증 처리
- 시세 조회
- 계좌/잔고 조회
- 주문 제출
- 주문 정정/취소
- 체결/미체결 조회

원칙:
- agent 없음
- 완전히 결정적이고 테스트 가능한 코드
- 브로커별 차이는 adapter로 흡수

예상 인터페이스:
- `get_quote(symbol)`
- `get_positions()`
- `get_cash_balance()`
- `place_order(order)`
- `cancel_order(order_id)`
- `get_order_status(order_id)`
- `list_fills(date)`

---

### 2. Market Data / Feature Layer

역할:
- 관심 종목 데이터 수집
- OHLCV, 호가, 거래대금 등 필요한 데이터 정리
- 전략 입력용 feature 계산

예시:
- 전일 대비 수익률
- 이동평균
- 변동성
- 거래량 이상치
- 포지션 비중
- 당일 손익

원칙:
- agent prompt에 바로 넣기 쉬운 요약 형태로 가공
- 원본 데이터와 요약 데이터 둘 다 저장 가능하게 설계

---

### 3. Strategy Runner

역할:
- 정해진 주기마다 실행
- 데이터 수집 → feature 생성 → agent 호출 → 검증 → 주문 실행 또는 보류

실행 단위:
- `run_once`
- 장중 주기 실행
- 장 시작 전 브리핑
- 장 마감 후 리포트 생성

원칙:
- 한 번 실행될 때마다 run id를 부여
- 입력과 출력 전부 감사 가능해야 함

---

### 4. Agent Decision Layer

역할:
- 시장과 계좌 상태를 보고 매매 판단 제안

입력 예시:
- 현재 시각, 장 상태
- 계좌 요약
- 현 포지션
- 관심 종목 요약
- 최근 판단/주문/체결 요약
- 명시적 전략 규칙
- 리스크 제한 요약

출력 형식:
```json
{
  "action": "buy",
  "symbol": "005930",
  "size": 10,
  "order_type": "market",
  "confidence": 0.74,
  "reason": "단기 추세 회복과 거래량 증가",
  "risk_notes": ["변동성 확대 구간"],
  "thesis": "...",
  "time_horizon": "intraday"
}
```

원칙:
- 반드시 JSON schema validation
- 허용되지 않은 필드는 무시 또는 실패 처리
- 자연어 설명은 남기되, 실행은 구조화 필드만 사용

---

### 5. Risk / Policy Engine

역할:
- agent 판단을 실제 주문 가능 여부로 필터링

정책 예시:
- 1회 주문 최대 금액
- 종목당 최대 비중
- 일간 최대 손실 한도
- 연속 손실 시 거래 중단
- 장 시간 외 주문 금지
- 중복 주문 금지
- 최근 N분 내 동일 종목 재진입 제한
- 금지 종목/경고 종목 차단
- 현금 부족 시 거절

원칙:
- agent보다 상위 권한
- 차단 사유를 반드시 기록
- 정책은 코드 + 설정 파일로 관리

---

### 6. Execution Engine

역할:
- 실제 주문 제출
- 주문 상태 추적
- 체결 반영
- 재시도 / 실패 처리

필수 특성:
- idempotency key
- retry-safe
- partial fill 대응
- timeout 및 네트워크 오류 처리
- 실제 주문 전 dry-run 가능

원칙:
- 실행 결과는 항상 저장
- "의사결정"과 "실행 결과"를 분리해 기록

---

### 7. Audit / Storage Layer

최소 저장 대상:
- run metadata
- market snapshot
- feature snapshot
- agent prompt input 요약
- agent raw output
- parsed decision
- risk evaluation result
- order request
- broker response
- fill events
- portfolio snapshots
- daily PnL snapshots

초기 저장소 추천:
- **SQLite** 로 시작
- 필요 시 PostgreSQL로 확장

테이블 후보:
- `runs`
- `market_snapshots`
- `agent_decisions`
- `risk_checks`
- `orders`
- `fills`
- `positions_snapshots`
- `daily_reports`

---

## User-Facing Tools

### CLI 최소 명세

목표:
- 현재 상태와 과거 기록을 사람이 투명하게 확인 가능해야 함

초기 명령 제안:
- `trading-agent run-once`
- `trading-agent market snapshot`
- `trading-agent portfolio show`
- `trading-agent orders list`
- `trading-agent fills list`
- `trading-agent decisions list`
- `trading-agent pnl today`
- `trading-agent report daily`
- `trading-agent doctor`
- `trading-agent config validate`

표시 예시:
- 계좌 현금
- 보유 종목 / 평균단가 / 현재가 / 손익
- 오늘 주문 내역
- 최근 10개 agent 판단
- 정책 차단 내역
- 최근 에러

---

### Discord Reporting 최소 명세

목표:
- 개발자/운영자가 Discord에서 상태를 빠르게 확인 가능해야 함

이벤트성 알림:
- agent가 매수/매도 판단 생성
- 리스크 엔진이 차단
- 주문 접수
- 체결 완료
- 주문 실패
- 거래 중단 트리거 발동

정기 리포트:
- 장 시작 전 요약
- 장중 중간 상태 요약
- 장 마감 요약
- 일간 손익 요약

리포트 항목 예시:
- 현재 현금 / 총 평가금액 / 실현손익 / 미실현손익
- 주요 포지션 상위 N개
- 오늘의 판단 수 / 주문 수 / 차단 수
- 주요 리스크 이벤트
- 마지막 에러

원칙:
- Discord는 알림 채널
- 진실의 원천은 DB와 CLI

---

## Suggested Repository Structure

레포는 크게 네 층으로 나누는 게 좋다.

1. **core library**
   - KIS client
   - domain model
   - risk / execution / storage

2. **operator tools**
   - CLI 명령
   - 상태 조회
   - 수동 reconcile / panic stop / doctor

3. **runtime jobs**
   - 반복 루프
   - 장 시작/장 마감 잡
   - 리포트 생성

4. **infra/config**
   - 설정
   - cron/systemd 예시
   - env template
   - SQL schema/migration

추천 구조:

```text
trading-agent/
  PLAN.md
  README.md
  pyproject.toml
  .env.example
  configs/
    app.yaml
    broker.yaml
    risk.yaml
    universe.yaml
    schedules.yaml
    prompts/
      decision.md
  docs/
    architecture.md
    operations.md
    kis-api-notes.md
    adr/
      001-python-first.md
      002-kis-broker-abstraction.md
  src/trading_agent/
    __init__.py
    settings.py
    logging.py
    constants.py

    domain/
      enums.py
      models.py
      schemas.py
      events.py

    broker/
      base.py
      mock.py
      kis.py
      kis_auth.py
      kis_mapper.py
      kis_errors.py
      kis_ws.py

    market/
      collector.py
      features.py
      universe.py
      sessions.py

    agent/
      client.py
      prompts.py
      decision.py
      parser.py
      guardrails.py

    risk/
      engine.py
      rules.py
      policies.py

    execution/
      engine.py
      orders.py
      reconciliation.py
      idempotency.py

    storage/
      db.py
      models.py
      repositories.py
      migrations/

    reporting/
      discord.py
      daily.py
      renderers.py
      snapshots.py

    runtime/
      run_once.py
      loop.py
      market_open.py
      market_close.py
      healthcheck.py

    cli/
      __init__.py
      main.py
      broker.py
      market.py
      portfolio.py
      orders.py
      reports.py
      admin.py

    utils/
      clock.py
      ids.py
      json.py
      retry.py

  tests/
    broker/
      test_kis_mapper.py
      test_mock_broker.py
    risk/
      test_risk_engine.py
    execution/
      test_reconciliation.py
    agent/
      test_parser.py
    runtime/
      test_run_once.py

  scripts/
    seed_mock_data.py
    run_paper_loop.py
    send_daily_report.py

  ops/
    cron/
      trading-agent-paper.cron
      trading-agent-daily-report.cron
    systemd/
      trading-agent-paper.service
      trading-agent-paper.timer
      trading-agent-daily-report.service
      trading-agent-daily-report.timer
```

---

### Current Repository Note

현재 레포에는 전체 MVP Python package가 아직 없고, 먼저 구현을 시작한 Rust 기반 KIS client component가 `kis-client/` 아래에 있다.

이 Rust crate는 위 구조의 `broker/` 계층에 해당하는 broker adapter 후보로 본다. 즉, 전체 제품의 중심은 여전히 `PLAN.md`의 trading-agent 구조이고, KIS client는 그중 브로커 연동 컴포넌트다.

앞으로 Python MVP package를 만들 경우, `src/trading_agent/...` 구조로 core/runtime/CLI/reporting을 구성하고 `kis-client`는 필요에 따라 FFI, 별도 프로세스, 또는 Rust service 형태로 연결할지 다시 결정한다.

---

### Why This Repository Shape Works

#### `broker/`
- KIS 의존성이 가장 강한 영역
- 나중에 다른 증권사를 붙여도 여기서 격리 가능

#### `runtime/`
- 반복 실행 루프와 장 시작/마감 잡을 분리
- 매번 CLI를 억지로 재사용하지 않아도 됨

#### `cli/`
- 운영자가 보는 진입점
- core 기능을 재조립하는 얇은 계층으로 유지

#### `reporting/`
- Discord, 일간 요약, 렌더링 로직 분리
- 나중에 웹 대시보드가 생겨도 재사용 가능

#### `storage/`
- 판단, 주문, 체결, 스냅샷을 일관되게 기록
- 감사 가능성의 중심

#### `docs/adr/`
- 왜 Python을 골랐는지
- 왜 KIS abstraction을 두는지
- 왜 live보다 paper 우선인지
- 이런 결정들을 남겨두면 나중에 흔들리지 않음

---

### Additional Pieces Worth Adding Early

질문에 적어주신 것 외에, 초기에 같이 넣는 게 좋은 것들:

1. **domain models**
   - 시스템 전체의 공용 언어
   - 브로커 응답을 바로 쓰지 않게 해줌

2. **risk/policy layer**
   - 이 프로젝트에서 사실상 핵심 안전장치

3. **storage/audit layer**
   - 나중에 제일 아쉬워지는 부분이라 초기에 넣는 게 좋음

4. **reconciliation job**
   - 주문 상태 꼬임 복구용
   - 실제 운영에서 굉장히 중요

5. **admin/ops commands**
   - `panic stop`
   - `doctor`
   - `config validate`
   - `orders reconcile`

6. **session/market-time utilities**
   - 장중인지, 동시호가인지, 점검시간인지 판단
   - 자동매매에서 의외로 자주 필요

---

### Monorepo or Single Package?

초기에는 **단일 Python package + 명확한 내부 모듈 분리**가 가장 좋다.

즉:
- 지금은 별도 microservice로 쪼개지 말고
- `src/trading_agent/...` 아래에 모아두되
- 모듈 경계를 분명히 유지

나중에 분리 검토 가능한 대상:
- `broker`를 독립 서비스로 분리
- `reporting`을 별도 worker로 분리
- `dashboard API`를 FastAPI로 별도 추가

하지만 MVP는 분리보다 **관찰 가능하고 디버깅 쉬운 단일 프로세스 구조**가 낫다.

---

### Concrete Recommendation

현재 단계의 추천은 이렇게 고정하는 게 좋아 보인다.

- **언어:** Python
- **KIS 접근 방식:** 공식 샘플 참고 + 자체 REST client 작성
- **초기 프레임워크:** 웹 프레임워크 없음, library-first
- **운영 인터페이스:** CLI + cron/systemd + Discord webhook
- **레포 구조:** single package with strong module boundaries

즉 한 줄로 요약하면:

**"Python으로 KIS REST client를 직접 감싼 라이브러리 중심 구조를 만들고, 그 위에 CLI, 반복 루프, 리포트 잡을 얹는 형태"**

---

## Korea Investment Securities Specific Plan

### Why KIS First

이 프로젝트의 첫 브로커는 한국투자증권(KIS Developers)로 고정한다.

선정 이유:
- 공식 Open API 포털이 비교적 명확하다.
- REST + WebSocket 구조가 분리되어 있어 시스템 설계가 깔끔하다.
- Python 샘플과 GitHub 예제가 풍부하다.
- LLM/자동화 사용 사례를 공식 자료에서도 적극적으로 다루고 있다.
- 초기 MVP에서 필요한 기능인 인증, 시세 조회, 주문, 잔고 조회, 체결 관련 흐름을 비교적 표준적인 방식으로 구성하기 좋다.

공식 참고:
- `https://apiportal.koreainvestment.com/intro`
- `https://github.com/koreainvestment/open-trading-api`

---

### KIS Integration Scope for MVP

초기 MVP 범위는 너무 넓히지 않고 아래만 먼저 지원한다.

1. 국내주식
2. 현금 계좌 1개
3. 단순 현금 매수/매도
4. `market` 또는 단순 `limit` 주문 한 가지부터 시작
5. 단일 계정, 단일 전략, 단일 실행 루프

초기 MVP에서 제외 권장:
- 신용/미수
- 다계좌 동시 운용
- 해외주식
- 조건검색 연동
- 복수 전략 동시 경쟁 실행
- 실시간 호가 기반 초단타

---

### KIS Runtime Model

KIS 연동은 다음 2채널 구조로 설계한다.

1. **REST channel**
   - 접근토큰 발급
   - 시세 조회
   - 잔고 조회
   - 주문 제출
   - 주문 정정/취소
   - 주문/체결 조회

2. **WebSocket channel**
   - 실시간 시세 수신
   - 실시간 체결/주문 이벤트 수신 가능 시 연결
   - 장중 리포트와 상태 반영 속도 개선

초기 구현 전략:
- **Phase 1~3은 REST 우선**
- WebSocket은 **투명성 개선과 반응속도 향상용으로 Phase 4 이후 도입**

이유:
- REST만으로도 run-once, paper trading, 일간 리포트, 주문 흐름 검증이 가능하다.
- 초기 장애 원인을 줄이기 좋다.

---

### KIS Environments

환경은 최소 3개로 나눈다.

1. `mock`
   - 로컬 가짜 브로커
   - 가장 빠른 테스트용

2. `paper`
   - KIS 모의투자 앱키/시크릿 사용
   - 실제 API 연결 흐름 검증용

3. `live`
   - KIS 실전투자 앱키/시크릿 사용
   - 제한된 소액 실거래용

원칙:
- `paper`와 `live`는 설정 파일과 비밀정보가 분리되어야 한다.
- 런타임 시작 시 현재 환경을 명시적으로 출력한다.
- `live`에서는 추가 가드가 활성화되어야 한다.

예:
- 기본값은 `mock`
- `paper`는 명시적 플래그 필요
- `live`는 플래그 + 추가 확인 필요

---

### KIS Credentials and Config Plan

민감정보는 저장소에 넣지 않는다.

설정 분리 원칙:
- 프로젝트 설정: `configs/*.yaml`
- 비밀정보: `.env` 또는 로컬 비밀 파일
- 계좌 정보: 환경별 설정에서 참조만

권장 항목:
- `KIS_APP_KEY_PAPER`
- `KIS_APP_SECRET_PAPER`
- `KIS_APP_KEY_LIVE`
- `KIS_APP_SECRET_LIVE`
- `KIS_ACCOUNT_NO`
- `KIS_ACCOUNT_PRODUCT_CODE`
- `KIS_HTS_ID`
- `DISCORD_WEBHOOK_URL`

`configs/broker.yaml` 예시 개념:
```yaml
broker: kis
environment: paper
product: domestic_stock
account:
  number_env: KIS_ACCOUNT_NO
  product_code_env: KIS_ACCOUNT_PRODUCT_CODE
auth:
  app_key_env:
    paper: KIS_APP_KEY_PAPER
    live: KIS_APP_KEY_LIVE
  app_secret_env:
    paper: KIS_APP_SECRET_PAPER
    live: KIS_APP_SECRET_LIVE
```

---

### KIS Adapter Responsibilities

`src/trading_agent/broker/kis.py`는 아래 책임을 가진다.

- 환경별 base URL 선택
- access token 발급 및 갱신
- 공통 헤더 구성
- 요청/응답 로깅용 메타데이터 생성
- API 오류를 내부 예외로 변환
- 내부 도메인 모델로 응답 매핑

보조 모듈:
- `kis_auth.py`: 토큰 발급, 토큰 만료 관리
- `kis_mapper.py`: KIS 응답 → 내부 모델 매핑
- `kis_errors.py`: 에러 코드/메시지 정규화

중요 원칙:
- KIS API 원문 응답을 그대로 시스템 전역에 흘리지 말고,
  내부 `Order`, `Position`, `Fill`, `Quote` 모델로 변환한 뒤 사용한다.

---

### Internal Broker Interface for KIS

KIS를 기준으로 내부 공통 인터페이스를 먼저 고정한다.

```python
class BrokerClient(Protocol):
    def get_quote(self, symbol: str) -> Quote: ...
    def get_positions(self) -> list[Position]: ...
    def get_cash_balance(self) -> CashBalance: ...
    def place_order(self, order: CreateOrderRequest) -> OrderResult: ...
    def cancel_order(self, order_id: str) -> CancelResult: ...
    def get_order(self, order_id: str) -> OrderStatus: ...
    def list_open_orders(self) -> list[OrderStatus]: ...
    def list_fills(self, from_date: date, to_date: date) -> list[Fill]: ...
```

이 인터페이스를 만족하도록 `mock`와 `kis`를 둘 다 맞춘다.

---

### KIS MVP Endpoint Categories

정확한 함수명과 path는 구현 시 공식 문서 기준으로 확정하되,
초기 MVP는 아래 카테고리만 커버하면 된다.

1. 인증
   - REST 접근토큰 발급
   - 필요 시 WebSocket 접속키 발급

2. 시세
   - 국내주식 현재가 조회
   - 필요 시 일봉/분봉 조회

3. 계좌
   - 잔고 조회
   - 주문 가능 금액 조회 또는 대체 가능 정보

4. 주문
   - 현금 매수 주문
   - 현금 매도 주문
   - 정정/취소 주문

5. 주문/체결 조회
   - 당일 주문 상태
   - 체결 내역

6. 보조
   - 종목 정보 파일 또는 종목 마스터 동기화

---

### KIS Symbol and Account Normalization

KIS 쪽 필드명을 애플리케이션 전체에서 직접 쓰지 않는다.

내부 표준 예시:
- `symbol`: `005930`
- `market`: `KRX`
- `side`: `buy | sell`
- `order_type`: `market | limit`
- `quantity`: 정수
- `limit_price`: 정수 또는 `None`
- `broker_order_id`: 브로커 원주문번호

계좌 필드도 내부 표준화:
- `account_number`
- `account_product_code`
- `available_cash`
- `buying_power`
- `settled_cash`

---

### KIS Order Lifecycle Model

내부 주문 상태는 브로커 원문과 분리한다.

추천 내부 상태:
- `decision_created`
- `risk_rejected`
- `ready_to_send`
- `submitted`
- `accepted`
- `partially_filled`
- `filled`
- `cancel_requested`
- `cancelled`
- `rejected`
- `failed`

KIS 응답/조회 결과는 위 상태 중 하나로 매핑한다.
이 매핑 규칙은 `kis_mapper.py`에 고정한다.

---

### KIS Failure Modes to Handle Early

초기부터 반드시 처리할 실패 유형:
- access token 만료
- app key / secret 설정 오류
- 계좌번호 형식 오류
- 실전/모의 환경 혼선
- 장시간 외 주문 요청
- 주문 수량/가격 단위 오류
- 잔고 부족
- 동일 주문 중복 전송
- 네트워크 타임아웃
- 브로커는 접수했지만 응답 저장 전에 프로세스 종료된 경우

대응 원칙:
- 주문 전 `client_order_id` 생성
- 제출 직후 원본 응답 저장
- 재기동 시 미확정 주문 reconciliation 수행

---

### KIS Reconciliation Plan

장중 장애나 프로세스 재시작을 대비해 reconciliation 단계를 둔다.

목표:
- DB에 있는 주문 상태와 KIS 실제 상태를 동기화

수행 시점:
- 앱 시작 시
- 장 마감 후
- 주문 전후 이상 상태 감지 시
- 수동 CLI 실행 시

CLI 예시:
- `trading-agent orders reconcile --today`
- `trading-agent broker ping`
- `trading-agent broker whoami`

---

### KIS-Specific Risk Rules

일반 리스크 외에 KIS 운영 특성을 반영한 규칙을 둔다.

- 모의투자와 실전투자의 환경 혼동 방지
- `live` 환경에서 기본 주문 수량 상한 강화
- 가격 입력이 필요한 주문에서 호가 단위 검증
- 장 운영 시간 외 자동 차단
- 브로커 응답 이상 시 재주문 금지 후 조사 상태 전환
- 동일 종목 당일 회전 수 제한
- API 오류 연속 발생 시 자동 정지

추가 권장:
- `live`에서는 `buy`보다 `sell` 허용 폭을 더 세밀히 다룰지 검토
- 장 시작 직후 몇 분간 거래 금지 옵션
- 변동성 급등 시 신규 진입 차단

---

### Agent Input Design for KIS MVP

초기 agent 입력은 KIS 원문 전체가 아니라 아래 요약만 사용한다.

```json
{
  "timestamp": "2026-04-23T14:00:00+09:00",
  "environment": "paper",
  "market_session": "regular",
  "account": {
    "available_cash": 1200000,
    "total_equity": 3100000,
    "daily_realized_pnl": 12000,
    "daily_unrealized_pnl": -3000
  },
  "positions": [
    {
      "symbol": "005930",
      "qty": 10,
      "avg_price": 58000,
      "last_price": 59200,
      "unrealized_pnl": 12000,
      "weight": 0.19
    }
  ],
  "watchlist": [
    {
      "symbol": "035720",
      "last_price": 41200,
      "change_pct": 2.4,
      "volume_ratio": 1.8,
      "ma5_gap_pct": 1.2,
      "volatility_pct": 3.1
    }
  ],
  "constraints": {
    "max_order_value": 300000,
    "max_symbol_weight": 0.2,
    "max_daily_loss": 50000
  }
}
```

중요:
- agent는 KIS 필드명을 몰라도 된다.
- agent는 내부 정규화 데이터만 본다.

---

### Agent Output Schema for KIS MVP

```json
{
  "action": "buy",
  "symbol": "035720",
  "quantity": 3,
  "order_type": "market",
  "limit_price": null,
  "confidence": 0.68,
  "reason": "거래량 증가와 단기 추세 회복",
  "risk_notes": ["장 초반 변동성 주의"],
  "hold_minutes": 90
}
```

검증 규칙:
- `symbol`은 universe 안에 있어야 함
- `quantity`는 양의 정수
- `market` 주문이면 `limit_price`는 `null`
- `limit` 주문이면 `limit_price` 필수
- `confidence`는 참고용이며 주문 허가 조건으로 쓰지 않음

---

### KIS-Oriented CLI Plan

우선 구현할 CLI를 KIS 기준으로 재정리한다.

운영 명령:
- `trading-agent broker ping`
- `trading-agent broker whoami`
- `trading-agent broker token status`
- `trading-agent market quote 005930`
- `trading-agent portfolio show`
- `trading-agent orders open`
- `trading-agent orders today`
- `trading-agent fills today`
- `trading-agent run-once`
- `trading-agent run-loop --paper`
- `trading-agent report daily`
- `trading-agent doctor`

안전 명령:
- `trading-agent panic stop`
- `trading-agent orders reconcile --today`
- `trading-agent config validate`

---

### KIS-Oriented Discord Reports

이벤트 메시지 최소 세트:
1. 판단 생성
2. 리스크 차단
3. 주문 접수
4. 부분 체결
5. 전체 체결
6. 주문 실패
7. 자동 정지

일간 요약 예시:
- 환경: `paper` 또는 `live`
- 총 평가금액
- 실현손익 / 미실현손익
- 주문 수 / 체결 수 / 차단 수
- 상위 포지션
- 마지막 오류
- 마지막 agent 판단 요약

운영 팁:
- `paper`와 `live` Discord 채널을 분리하는 것이 좋다.
- `live`는 경고성 메시지를 더 크게 보낸다.

---

### KIS-Based Delivery Phases

### Phase 0. KIS Discovery
- KIS 서비스 신청 절차 확인
- 모의/실전 앱키 발급
- 계좌번호, 상품코드, HTS ID 준비
- 국내주식 필수 API 목록 확정
- REST rate limit 및 호출 정책 확인

### Phase 1. KIS Foundation
- 프로젝트 scaffold 생성
- settings / secrets 로딩 구현
- KIS auth client 구현
- `broker ping`, `broker whoami` 구현
- mock broker 병행 구현
- SQLite schema 정의

### Phase 2. KIS Market + Portfolio Read MVP
- 현재가 조회 연결
- 잔고 조회 연결
- 현금/주문가능금액 조회 연결
- CLI `market quote`, `portfolio show` 구현
- portfolio snapshot 저장

### Phase 3. KIS Order MVP in Paper
- paper 환경에서 매수/매도 주문 연결
- 주문 조회 / 체결 조회 연결
- 내부 주문 상태 매핑 구현
- reconciliation 구현
- dry-run과 실제 paper 주문 분리

### Phase 4. Agent + Risk Integration
- watchlist / feature 계산 구현
- agent decision prompt 작성
- decision schema validator 구현
- risk engine 구현
- `run-once`로 paper 자동 판단 루프 연결

### Phase 5. Reporting + Observability
- Discord 이벤트 알림 연결
- daily report 구현
- doctor 명령 구현
- 에러 분류/로그 정리
- WebSocket 도입 여부 판단

### Phase 6. Limited Live Rollout
- `live` 전용 강화 규칙 활성화
- 소액 / 저빈도 실거래 시작
- 거래 시간, 주문 금액, 일일 손실 제한 보수적으로 설정
- 일정 기간 수동 검토 병행

---

## Delivery Phases

### Phase 0. Discovery
- 대상 국내 증권사 API 선정
- 인증 방식 확인
- 주문/체결/잔고 API 제한 확인
- 실전/모의투자 지원 여부 확인
- rate limit, 장 운영 시간, 계좌 제약 확인

### Phase 1. Foundation
- Python project scaffold
- config loading
- logging setup
- SQLite schema 정의
- CLI skeleton 작성
- mock broker 작성

### Phase 2. Agent Decision MVP
- decision prompt 초안
- agent output schema 정의
- parser + validator 구현
- `run-once` 파이프라인 연결
- dry-run 모드 지원

### Phase 3. Risk + Execution MVP
- 주요 risk rules 구현
- mock broker 주문 실행 연결
- order lifecycle 기록
- 차단/실행/실패 로깅 정리

### Phase 4. Transparency MVP
- CLI 조회 명령 구현
- Discord webhook 또는 bot 기반 report 구현
- 일간 리포트 생성
- 장중 이벤트 알림 구현

### Phase 5. Paper Trading
- 모의투자 또는 mock 환경에서 일정 기간 운영
- 실패 케이스 수집
- agent 판단 품질 리뷰
- risk rule 보정
- Discord 알림 노이즈 조정

### Phase 6. Limited Live Trading
- 소액 실거래
- 최대 주문 한도 매우 낮게 설정
- 수동 승인 옵션 검토
- 비상 정지 절차 운영

---

## Safety Requirements

필수:
- dry-run 모드
- mock broker
- schema validation
- 일간 손실 제한
- 종목별 최대 비중 제한
- 주문 전 정책 검증
- audit log 영구 저장
- panic stop 기능
- market hours guard

강력 권장:
- 주문 전 이중 확인 모드
- 특정 조건에서는 human approval
- Discord 경고 알림 분리 채널
- 비정상 상태 탐지용 `doctor` 명령

---

## Agent Design Principles

- agent는 시장 데이터 전체를 무작정 읽지 말고, 요약된 컨텍스트를 본다.
- agent는 "왜"를 설명할 수 있어야 하지만, 실행은 구조화 필드만 따른다.
- agent 출력은 작은 schema로 시작한다.
- 한 번에 하나의 결정만 하게 만드는 것이 초기 안정성에 유리하다.
- 위험한 자유도는 코드가 막고, 판단의 유연성만 agent에 맡긴다.

초기 추천 행동 공간:
- `buy`
- `sell`
- `hold`

초기 추천 주문 타입:
- 시장가 또는 단순 지정가 중 하나만 먼저 지원

---

## Operational Principles

- 장애 원인 추적이 쉬워야 한다.
- "왜 이 주문이 나갔는지"를 몇 분 안에 확인할 수 있어야 한다.
- 운영자는 Discord에서 알림을 받고, 자세한 내용은 CLI와 DB에서 본다.
- 실전 진입 전 paper trading 기간을 충분히 둔다.
- 복잡한 전략보다 운영 투명성과 안전장치를 먼저 만든다.

---

## Immediate Next Steps

1. KIS Developers 서비스 신청 상태 확인
2. 모의투자 / 실전투자 앱키 분리 발급
3. 국내주식용 필수 엔드포인트 목록 확정
4. `kis.py`, `kis_auth.py`, `kis_mapper.py` 기준으로 scaffold 생성
5. `mock`와 `paper` 환경을 동시에 지원하도록 설정 구조 확정
6. CLI 최소 명령부터 구현
   - `broker ping`
   - `broker whoami`
   - `market quote`
   - `portfolio show`
7. paper 주문 전용 order MVP 구현
8. decision schema와 KIS 전용 risk policy 초안 작성
9. Discord 리포트 초안 연결
10. paper trading 루프 운영

---

## Recommendation

이 프로젝트의 핵심은 "agent를 어디까지 믿을 것인가"가 아니라,
**agent의 판단을 안전하게 받아서 검증하고 기록하고 통제하는 운영 구조**를 만드는 것이다.

따라서 첫 버전 목표는 다음처럼 잡는 것이 좋다.

- agent가 판단한다.
- 스크립트가 검증한다.
- 브로커 adapter가 실행한다.
- DB가 기억한다.
- CLI와 Discord가 보여준다.

이 다섯 축이 첫 MVP의 기준선이다.
