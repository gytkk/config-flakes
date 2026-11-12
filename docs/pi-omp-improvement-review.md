**있어. 다만 네 Pi는 이미 많이 보강돼 있어서, OMP의 도구를 대거 가져오기보다 UI와 필요한 순간의 코드 작업만 보강하는 게 좋아.**

Astra 서브에이전트 2개로 **설정·프롬프트**와 **OMP 기능·구현**을 나눠 리뷰했고, 내가 실제 설치 상태와 주요 근거를 대조했어. 기준은 **Pi 1.5.2 / OMP 18.19.0**이야.

먼저 한계는 분명히 할게. **성공률·시간·비용을 모두 유지하면서 코드 품질이 오른다는 보장은 못 해.** 대신 모델 요청을 바꾸지 않는 UI 개선은 그 위험을 상당히 줄일 수 있어.

## 1. 우선 추천: 모델 동작을 그대로 두는 개선

| 후보 | 지금 네 Pi와의 차이 | 보수적인 적용 방법 |
|---|---|---|
| **`/context` 같은 구성 조회 화면** | `/session` 통계는 있지만, 프롬프트·skill·tool별 구성 조회와는 달라 | 사용자가 열 때만 로컬에서 목록·크기를 집계. 모델 호출·메시지 삽입 없이 구현 |
| **5시간 quota 표시 추가** | 현재 확장은 **주간 제한만** 읽어. OMP는 단기·주간 창을 함께 보여줘 | 기존 CodexBar 응답을 재사용하고 `/codex-usage` 상세 화면부터 확장. 별도 polling 금지 |
| **수동 diff 확인·주석 → 입력창 연결** | 파일 열기는 이미 편하지만, 변경 내용을 보고 피드백을 작성하는 흐름은 별개야 | 설치된 Hunk/Neovim을 활용하는 바로가기부터. 자동 리뷰 호출 없이 사용자가 제출 |

### 가장 먼저 할 건 quota 표시야
현재 [codex-usage.ts:49](../modules/pi/files/extensions/codex-usage.ts#L49)는 `usage.secondary`만 파싱해. 같은 응답에 단기 창이 있다면 **조회 횟수나 모델 context를 늘리지 않고** 활용할 수 있어.

다만 CodexBar가 보는 Codex CLI 계정과 Pi 로그인 계정이 다를 수 있다는 기존 한계는 유지돼. 계정 연동까지 새로 만드는 건 추천하지 않아.

### `/context`는 비용 관리에도 유용해
OMP의 [Context Viewer](https://github.com/can1357/oh-my-pi/blob/v18.19.0/docs/slash-command-internals.md#14-built-in-command-note-context)는 프롬프트·도구·메시지 구성을 보여주면서 모델 호출이나 context hook을 실행하지 않아.

Pi에도 필요한 공개 조회 API가 있어. 다만 **“실제 provider 요청과 완전히 동일한 화면”이라고 표시하면 안 돼.** 우선 원문 덤프보다 출처·활성 도구·크기 중심으로 만드는 게 안전해.

### diff 주석은 OMP 명령을 그대로 복사하면 안 돼
OMP의 [`/annotate`](https://github.com/can1357/oh-my-pi/blob/v18.19.0/docs/slash-command-internals.md#12-bundled-command-note-annotate)는 편하지만, 일부 경로는 세션 내용을 모델로 요약하거나 LLM 리뷰로 이어져. 가져올 부분은 **로컬 diff 보기 → 메모 → 입력창에 붙이기**까지야.

위 세 가지도 로컬 처리·렌더링 비용은 있어. 다만 모델의 추론 경로를 건드릴 필요는 없어.

## 2. 코드 품질 쪽: 가치 있지만 조건부 추천

### A. 리뷰어에 “생산자뿐 아니라 소비자까지 추적” 추가

OMP에서 가장 가져올 만한 **프롬프트 규칙**이었어.

> 새 이벤트·enum·IPC payload를 추가했다면, 받는 쪽의 switch/router/handler가 실제로 처리하는지도 확인한다.

현재 Pi [reviewer.md:17](../modules/pi/agents/reviewer.md#L17)는 callers 확인을 요구하지만, 이 경계 검사는 구체적이지 않아. OMP [reviewer의 cross-boundary 규칙](https://github.com/can1357/oh-my-pi/blob/v18.19.0/packages/coding-agent/src/prompts/agents/reviewer.md)을 **해당 역할에만 짧게 반영**하는 건 괜찮아 보여.

- 별도 리뷰어 호출은 추가하지 않기.
- 이벤트·메시지 등 경계가 바뀐 diff에만 적용하기.
- **관련 파일을 더 읽으므로 해당 리뷰의 시간·토큰이 늘 수 있음.**

엄격하게 “추가 비용 없음”을 원한다면 이것도 자동 채택 대상은 아니야.

### B. LSP semantic rename·code actions

지금 `pi-lsp`에는 diagnostics·hover·definition·references·symbols가 있어. OMP와의 실질적인 차이는 **심볼 rename, 파일 이동에 따른 참조 수정, code actions**야.

여러 파일에 걸친 리팩터링을 자주 한다면 후보야. 단:

- 기존 LSP 연결을 재사용할 수 있을 때만.
- 명시적인 리팩터링 요청에서만 활성화.
- preview 후 적용.
- 별도 LSP 서버 체계나 상시 workspace 검사 추가는 피하기.

서버 인덱싱과 추가 도구 왕복이 있어서 **품질·시간 동시 개선은 아직 가설**이야. 당장은 이미 쓰는 Neovim의 리팩터링 기능을 활용하는 쪽이 유지보수 부담이 작아.

## 3. 가져오지 않아도 되는 것

네 설정에는 이미 아래가 있어.

- LSP 진단·탐색
- 백그라운드 셸과 `/bg`
- 웹·서브에이전트 도구의 지연 활성화
- 구조화된 사용자 질문
- fresh context와 제한된 위임
- Luna/Sol/Astra 역할 분리
- Herdr 서브에이전트 표시·Neovim 파일 링크
- quota/context 표시와 캐시 알림

실제 홈 디렉터리에는 **Orca 상태·titlebar 확장도 추가로 설치돼 있었어.** 그래서 “OMP처럼 완료 알림과 Agent Hub를 새로 만들자”는 추천은 우선순위에서 뺐어. 기존 연동에 없는 구체적인 불편이 확인될 때만 보강하는 게 맞아.

다음은 지금 조건에서는 비추천이야.

- **Hashline 전체 이식:** `read`·snapshot·편집 문법이 함께 바뀌는 큰 변경. Astra에서 이득 미검증.
- **기본 read를 자동 코드 요약으로 교체:** 출력은 줄어도 구현 누락·재독 위험.
- **DAP·persistent eval·범용 문서 도구 상시 추가:** 특정 작업에는 유용하지만 기본 도구 부담과 유지보수 증가.
- **자동 advisor/judge·추가 reviewer·광범위 병렬화:** 추론 비용이 직접 늘어남.
- **OMP 시스템 프롬프트 전체 복사:** 현재 Pi 도구·위임 정책과 충돌 가능.
- **context 확대·compaction 지연:** 성공률이나 총비용 개선 근거 없음.

## 4. 별도로 발견한 비용 관련 차이

[modules/pi/README.md:42](../modules/pi/README.md#L42)는 cache warming 기본값을 `streaming`이라고 설명하지만, **설치된 Pi 1.5.2에 현재 설정을 적용해 조회한 값은 `idle`**이었어. 명시적인 설정값은 없고, 저장소 관리 설정과 실제 설정은 일치했어.

`idle`은 조건을 충족하면 유휴 중에도 캐시를 갱신해. 추가 사용량이 생기지만 캐시 미스와 재개 지연을 줄일 수 있으므로 **무조건 끄는 게 절약은 아니야.** 우선 문서를 실제 동작에 맞추고, 정책 변경은 분리하는 게 맞아.

---

**내 선택은 `단기 quota 표시 → /context 조회 → 수동 diff 피드백 연결` 순서야.** 프롬프트에서는 전체 복사 대신 **reviewer의 소비자 경계 확인 한 가지**만 조건부 후보로 남기겠어.

설정 파일은 변경하지 않았어. Astra 실행을 위해 잠시 허용한 호출별 모델 지정도 요청대로 원래 상태로 복원했어.
