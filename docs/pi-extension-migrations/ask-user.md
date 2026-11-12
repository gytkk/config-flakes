# Pi 질문 도구를 pi-ask-user로 전환

상태: pi-ask-user 전환은 채택하지 않고 rpiv-mono의 질문 도구를 유지한다. 사용자가 2026-11-12에 [rpiv-mono](https://github.com/juicesharp/rpiv-mono/tree/main/packages/rpiv-ask-user-question)의 질문 도구를 선택했으며, 기존 pin인 `@juicesharp/rpiv-ask-user-question@2.15.0`이 이미 이 저장소에서 배포된 npm latest임을 확인했다. 패키지 이름이나 버전을 바꿀 필요는 없고 monorepo 전체도 설치하지 않는다. 아래는 채택하지 않은 pi-ask-user 0.17.2의 전환 명세다. 해당 후보에서 발견한 native blocked metadata 누락과 검증은 `pi-migration/ask-user` branch의 커밋 c5cc2444에 보존한다.

## 목표와 완료 조건

현재 RPIV 질문 extension을 `npm:pi-ask-user@0.17.2`로 교체하고, 모델이 패키지의 `ask_user` 도구로 터미널에서 사용자 결정을 받도록 한다. 독립 실행한 이 작업의 완료 조건은 새 패키지의 단일 로드, 기존 질문 도구의 제거, 질문과 취소 흐름의 검증, 변경된 사용법의 문서화다. 다른 extension 전환의 완료를 기다리지 않는다.

## 시작할 때 읽을 자료

- `AGENTS.md`와 `modules/pi/README.md`: 저장소 작업 규칙, 설치 소유권, 검증 및 사용자 적용 절차.
- `modules/pi/default.nix`: `extensionPackages`가 package 선택의 원본이다. `modules/pi/files/settings.json`에 package 목록을 다시 만들지 않는다.
- `modules/pi/tests/module-config.test.ts`와 `modules/pi/tests/extension-loading.test.ts`: 생성 설정과 Node/jiti loader 검증 패턴.
- [pi-ask-user README](https://github.com/edlsh/pi-ask-user)와 [0.17.2 배포본](https://www.npmjs.com/package/pi-ask-user/v/0.17.2): 실행 전에 실제 배포본의 schema, peer dependencies, 설정 및 fallback을 확인한다.

작업 시작 시 Git 상태, Pi pin, 실제 package 목록을 다시 확인한다. 조사 이후의 사용자 변경과 다른 전환 작업을 보존한다. 다른 버전이 필요하면 이유를 보고하고, 이 작업에 다른 패키지 업데이트를 묶지 않는다.

## 목표 동작

| 영역 | 요구사항 |
| --- | --- |
| 도구 | upstream의 `ask_user`를 직접 사용한다. 기존 이름을 유지하는 wrapper나 별도 질문 UI를 만들지 않는다. |
| 질문 수 | 단일 질문 또는 독립적인 2~4개 질문 묶음을 지원한다. 앞선 답에 의존하는 질문은 다음 호출에서 묻는다. |
| 응답 | single-select, multi-select, 직접 입력과 제출 전 review를 패키지 UI로 제공한다. 선택지는 설명을 포함한다. |
| 취소 | Escape, timeout, session 종료는 취소 또는 미응답으로 전달한다. 기본 선택이나 승인으로 바꾸지 않는다. |
| 표시 | 패키지의 터미널 UI를 사용한다. 초기 표시는 upstream 기본 overlay로 시작하고, 기존 대화를 다시 읽는 패키지 기능을 유지한다. 브라우저를 추가하지 않는다. |
| 비대화형 모드 | RPC에서는 지원되는 fallback을 사용하고, 사용자 입력을 받을 수 없는 실행은 성공한 답변으로 처리하지 않는다. |
| 상태 보고 | 질문 대기와 해제 시 Pi의 blocked 상태가 정상적으로 갱신되어야 한다. Herdr 연동을 새로 구현하지 않는다. |

Schema는 `question` 또는 `questions`, `options[].title`과 `description`, `allowMultiple`, `allowFreeform` 등 배포본의 필드를 따른다. 모델 지침과 테스트에 RPIV의 `multiSelect`, `header`, `options[].preview`를 계속 요구하지 않는다. 선택지별 별도 preview는 동등하게 보존하지 않으며, 설명과 context로 전달할 수 있는 범위만 패키지 기능을 사용한다.

## 변경 범위

1. `modules/pi/default.nix`의 RPIV package 선언만 새 package로 교체한다. Object 형식으로 extension은 로드하고 bundled skill과 prompt는 제외한다. 사용자의 질문 정책은 기존 지침이 소유한다.
2. 현재 지침과 agent 정의에서 이전 도구 이름이나 schema를 직접 요구하는 부분을 조사하고, 실제로 남은 참조만 수정한다. 새 schema 설명은 우선 패키지의 도구 선언을 사용한다.
3. `modules/pi/tests/module-config.test.ts`의 package 기대값과 loader 검증을 갱신한다. 기존 질문과 새 질문 도구가 동시에 등록되지 않는지 검사한다.
4. `modules/pi/README.md`에 package pin, `ask_user` 사용법, RPIV 대비 schema 및 preview 차이, 적용 절차를 기록한다.

공통 agent 지침을 수정해야 한다면 먼저 `agent-core/README.md`를 읽고 canonical 원본과 renderer 검증 절차를 따른다. 생성된 홈 디렉터리 지침을 직접 편집하지 않는다. 이미 일반적인 질문 사용 원칙으로 표현된 지침은 다시 쓰지 않는다.

## 유지할 범위와 독립 실행

모델, thinking, credentials, web, subagents, shell, LSP, prompt UI, rounded-tools 및 Herdr 구성은 유지한다. Child session에 질문 도구를 새로 전달하거나 위임 정책을 바꾸는 작업은 포함하지 않는다.

공통 수정 파일은 `modules/pi/default.nix`, `modules/pi/tests/module-config.test.ts`, `modules/pi/tests/extension-loading.test.ts`, `modules/pi/README.md`다. 현재 checkout의 package 목록에서 질문 항목만 바꾸고, 다른 작업이 추가하거나 제거한 항목을 이전 목록으로 덮어쓰지 않는다. [Shell](interactive-shell.md), [Lens](lens.md), [Powerline](powerline-footer.md) 작업은 선행 조건이 아니다. Powerline이 이미 적용됐다면 질문 overlay와 editor focus의 조합도 확인한다.

## 검증

자동 검증은 다음을 확인한다.

- 생성된 settings에 새 pin이 있고 RPIV 선언은 없다. 다른 package와 device-local metadata의 병합 동작은 유지된다.
- 실제 Pi 버전의 Node/jiti loader에서 `ask_user`가 한 번 등록되며 이전 `ask_user_question`은 이 전환의 provider로 남지 않는다. 패키지가 요구하는 peer가 해결되고 loader 오류가 없다.
- 단일 선택, 복수 선택, 직접 입력, 여러 질문의 review 및 취소 결과가 원래 입력과 구분되어 반환된다. Headless 실행이 응답을 기다리며 무기한 멈추지 않는다.
- `/reload`나 session 교체 후 중복 handler 또는 이전 질문에 대한 늦은 응답이 새 session에 적용되지 않는다.

기존 runner에서 좁은 검증을 시작한다.

```bash
bash modules/pi/tests/run.sh module-config.test.ts extension-loading.test.ts
bash modules/pi/tests/sync-settings.sh
uv run --no-project scripts/check-paths.py
uv run --no-project scripts/check-paths.py modules/pi/README.md docs/pi-extension-migrations/ask-user.md
```

추가한 질문 integration 테스트도 실행한다. Nix 파일을 바꾸면 `nixfmt`를 적용하고, module wiring 변경에 필요한 최종 구성 검증은 `modules/pi/README.md`를 따른다. upstream 내부 UI 전체를 복제하는 테스트 대신 이 구성의 로딩과 사용자 동작을 검증한다.

실제 TUI 확인은 한국어 직접 입력, 좁은 pane, 대화 스크롤, overlay 숨김과 복귀, 취소 후 editor focus, 질문 대기 상태를 포함한다. 실제 모델 없이 검증할 수 있는 경로를 우선 사용한다. 실행할 수 없는 UI 확인은 미검증으로 남긴다.

## 적용과 복구

실제 Home Manager 적용은 사용자가 수행하고 새 Pi session에서 확인한다. 실행 중인 질문을 종료한 뒤 전환하며 기존 session과 인증 상태는 보존한다. Package 선언 제거와 설치 cache 삭제는 별개이므로, 사용 중이지 않은 RPIV 설치 파일을 별도로 삭제하는 cleanup은 수행하지 않는다.

복구는 이 작업에서 바꾼 package 선언과 관련 지침만 되돌려 사용자가 다시 적용하는 방식으로 한다. 문서와 검증 결과를 포함한 이 작업의 변경만 커밋하고 push하지 않는다. 결과에는 자동 검사, 실제 UI 확인, 사용자 적용이 남은 항목을 구분한다.
