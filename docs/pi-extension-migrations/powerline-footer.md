# Pi prompt UI를 pi-powerline-footer로 전환

상태: 알려진 마우스 제약을 수용하고 통합. Pi 1.5.2와 pi-powerline-footer 0.22.0의 실제 component 검사에서 마우스 좌표 회귀를 확인했으며, 사용자가 2026-11-12에 이 제약을 수용하고 통합하도록 승인했다. 회귀 검사는 실패 상태로 유지하고, 실제 Home Manager 적용과 남은 TUI 검증은 별도로 수행한다.

## 목표와 완료 조건

로컬 `prompt-ui`를 `npm:pi-powerline-footer@0.22.0`으로 대체하고 editor와 statusline의 구현을 upstream에 맡긴다. 모델, thinking, 경로, Git 상태, context와 기존 usage/fast 상태를 표시한다. 기존 UI의 모양과 키보드 동작을 그대로 복제하는 별도 renderer나 adapter를 만들지 않는다.

완료 조건은 editor/footer 소유자가 Powerline 하나인 상태, 필요한 정보의 표시, 입력과 cursor 동작의 검증, 기존 prompt 구현 및 전용 테스트의 정리다. 다른 세 전환 작업을 기다리지 않고 완료할 수 있어야 한다.

## 시작할 때 읽을 자료

- `AGENTS.md`와 `modules/pi/README.md`: UI 소유권, theme, hardware cursor와 사용자 적용 절차.
- `modules/pi/default.nix`, `modules/pi/files/settings.json`과 전환 전 UI 원본인 modules/pi/files/extensions/prompt-ui/.
- `modules/pi/files/extensions/codex-fast-mode.ts`, `modules/pi/files/extensions/codex-usage.ts`: 상태 제공과 실제 기능을 분리하는 경계.
- 전환 전 UI 검사인 modules/pi/tests/prompt-ui.test.ts, modules/pi/tests/prompt-statusline.test.ts, modules/pi/tests/prompt-git-status.test.ts와 유지할 `modules/pi/tests/extension-loading.test.ts`. 이미 제거된 경로는 이전 commit에서 확인하고 재생성하지 않는다.
- [0.22.0 README](https://github.com/nicobailon/pi-powerline-footer/blob/v0.22.0/README.md)와 [설정 및 UI 구현](https://github.com/nicobailon/pi-powerline-footer/blob/v0.22.0/index.ts).

착수 시 Git 상태, Pi pin과 다른 전환의 완료 여부를 확인한다. 사용자 수정과 이미 적용된 package 선택을 보존한다. 다른 버전이나 Pi 자체 patch가 필요하면 범위를 확대하기 전에 보고한다.

## 목표 UI

| 표시 또는 동작 | 요구사항 |
| --- | --- |
| 기본 배치 | Powerline의 default preset과 editor 위 배치에서 시작한다. 현재 모양의 재현보다 upstream 설정과 좁은 pane 동작을 우선한다. |
| 작업 위치 | 경로와 Git branch, staged/unstaged/untracked 상태를 표시한다. 표기와 갱신 주기는 upstream을 사용하며 로컬 Git polling을 추가하지 않는다. |
| 모델 | 현재 model과 thinking level을 표시한다. model 전환 후 stale 값이 남지 않는다. |
| Context | 실제 선택 모델의 effective context window와 사용량을 반영한다. 1M을 화면에 고정하지 않는다. |
| Weekly quota | 기존 `codex-usage` status를 표시한다. 구독 quota와 API 비용, session token을 같은 값으로 설명하지 않는다. |
| Fast | 실제로 적용 가능한 OpenAI parent session에서 켜졌을 때만 fast 상태를 표시한다. |
| 입력 | 한국어 IME, multiline, history, autocomplete와 mouse selection이 정상 동작한다. |
| Theme | 기존 Pi theme 선택과 hardware cursor 설정을 유지한다. 별도 app-local theme 사본을 만들지 않는다. |

기존의 정확한 `╰─` 입력줄 모양, 전체 폭 context meter, quota의 정확한 위치, 전용 `/bg (N)` 선택 UX는 보존 대상이 아니다. 의미 있는 상태 정보와 입력 기능을 보존하고 나머지는 패키지 UI를 수용한다.

## 최소 구성

- Welcome overlay는 끈다.
- `powerline.queue.compactPromptMode`는 `native`로 설정해 `/compact <text>`의 기존 의미를 유지한다. 메시지의 전송 순서를 바꾸는 추가 queue나 자동 follow-up을 운영 기능으로 도입하지 않는다.
- `bashMode.toggleShortcut`은 `null`로 해제한다. Sticky bash와 shell ghost completion은 사용하지 않으며 agent용 장기 실행은 별도 shell package가 소유한다.
- Delayed send, AI-generated working vibes 등 추가 지연이나 모델 호출을 만드는 기능은 활성화하지 않는다. 실제 설정 이름은 고정 버전의 schema로 확인한다.
- `powerline.customItems`와 extension status API로 usage/fast를 연결한다. Session history나 다른 extension의 내부 자료구조를 파싱하는 bridge를 만들지 않는다.
- 기존 `editorPaddingX`, `outputPad`, `showHardwareCursor`는 우선 유지하고, 패키지와 실제 입력 검증으로 필요한 설정 변경만 한다.

Powerline의 shell 기능과 package 존재 자체를 새로운 실행 권한으로 해석하지 않는다. 현재 승인 및 subagent 정책은 유지한다.

## 변경 범위

1. `modules/pi/default.nix`에 Powerline pin을 추가하고 로컬 `prompt-ui` 설치 선언을 제거한다. Runtime 설정은 `modules/pi/files/settings.json`의 해당 namespace에 둔다. Package 목록은 Nix의 `extensionPackages`에서만 관리한다.
2. 전환 전 디렉터리인 modules/pi/files/extensions/prompt-ui/를 제거한다. 다른 consumer가 없는지 확인하고, 이 구현만 검사하는 테스트 및 loader fixture를 정리한다.
3. `codex-usage.ts`의 조회 방식, 인증 출처, 주기, 실패 및 stale 처리와 quota 의미는 유지한다. 데이터 provider를 다른 usage 패키지로 바꾸지 않는다.
4. Fast의 display 값이 현재 `on`/`off`여서 직접 표시하기 어렵다면 `codex-fast-mode.ts`의 status 발행만 최소 수정할 수 있다. 비활성 또는 적용 불가 상태는 숨기고 model 전환 때 갱신한다. `/fast` 명령, session branch 복원, child 제외, provider request payload는 바꾸지 않는다.
5. `modules/pi/README.md`의 prompt, cursor, Git 표시와 verification 내용을 새 UI에 맞춘다. 기존 특수 키와 픽셀 단위 배치를 여전히 지원한다고 쓰지 않는다.
6. Module/package selection 및 실제 loader 검사, status 제공자와 Powerline 표시의 integration 검증을 추가한다. Upstream renderer를 복제하는 snapshot suite는 만들지 않는다.

`rounded-tools`와 extension-message frame은 그대로 유지한다. Herdr App/TUI, theme pipeline, 파일 링크 patch, LSP, 질문, background backend와 모델 설정은 이 작업의 교체 대상이 아니다.

## 독립 실행과 충돌 처리

[Shell](interactive-shell.md)이 먼저 적용됐으면 package-owned shell UI와 함께 검증한다. 아직 적용되지 않았으면 `pi-bg-bash`는 유지하고 그 패키지의 `/bg` UI를 사용한다. 어느 순서든 입력줄의 별도 `/bg (N)` 기능은 재구현하지 않는다. Shell package 선택은 Shell 작업이 소유한다.

`modules/pi/default.nix`, `modules/pi/files/settings.json`, `modules/pi/tests/module-config.test.ts`, `modules/pi/tests/extension-loading.test.ts`, `modules/pi/README.md`는 다른 전환과 공유할 수 있다. 최신 파일에 Powerline 관련 변경만 적용하고 전체 설정을 과거 snapshot으로 교체하지 않는다. Prompt 디렉터리 삭제와 Shell 작업의 bg 부분 수정이 충돌하면 최종 삭제를 유지하고, 필요한 shell 문서·테스트 변경만 남긴다. [질문](ask-user.md)과 [Lens](lens.md)의 package 선언은 수정하지 않는다.

## 검증

- 실제 Node/jiti loader에서 Powerline이 한 번 로드되고 로컬 prompt UI는 발견되지 않는다. Editor/footer의 중복 등록이나 shortcut 충돌 경고가 없다.
- Path, Git 상태, 모델, thinking, context, quota가 유효한 값으로 표시된다. 좁은 폭에서는 덜 중요한 값을 생략하더라도 입력과 cursor가 남는다.
- `/fast on`, `/fast off`, 다른 provider 전환, OpenAI 복귀, branch 복원과 child session에서 label이 실제 적용 상태를 반영한다. Payload 동작은 기존 회귀 검사로 보존한다.
- Usage 조회 실패와 stale 상태가 사라지거나 정상 quota로 보이지 않는다. 추가 조회 process나 polling loop가 생기지 않는다.
- `/compact`와 `/compact <text>`의 native 의미, 메시지 제출·취소·follow-up 동작을 확인한다. UI 전환 때문에 추가 모델 호출이나 자동 메시지 전송이 생기지 않는다.
- 한국어 IME, multiline wrapping, history, autocomplete, paste, mouse selection, resize, 좁은 Herdr pane, reload 후 입력을 실제 TUI에서 확인한다.
- 이미 적용된 질문 overlay, shell overlay, Lens feedback과 기존 rounded-tools가 함께 표시되고 닫힌 뒤 focus가 복원된다. 적용되지 않은 후보를 이 작업을 위해 추가하지 않는다.

좁은 검사부터 실행한다.

```bash
bash modules/pi/tests/run.sh module-config.test.ts extension-loading.test.ts codex-fast-mode.test.ts codex-usage.test.ts
bash modules/pi/tests/sync-settings.sh
uv run --no-project scripts/check-paths.py
uv run --no-project scripts/check-paths.py modules/pi/README.md docs/pi-extension-migrations/powerline-footer.md
```

추가한 UI integration 검사와 범용 rounded-tools 검사를 실행한다. Nix formatting과 최종 configuration 검증은 `modules/pi/README.md`를 따른다. 실제 TUI를 실행할 수 없으면 loader 성공을 IME나 mouse 동작 검증으로 대신하지 않고 사용자 확인 항목을 남긴다.

## 적용과 복구

Home Manager 적용은 사용자가 수행하고 새 Pi session에서 확인한다. Package 추가와 로컬 prompt 설치 제거를 함께 적용해 두 editor가 경쟁하는 중간 상태를 피한다. Reload만으로 Nix 설치 선언이 바뀌었다고 판단하지 않는다.

복구는 이 작업의 package, Powerline 설정, display 전용 변경과 prompt 설치 선언만 되돌린다. Shell 전환이 이미 끝났다면 과거 `/bg` UI를 재도입하지 않는다. 다른 사용자 설정과 전환 결과를 보존한다. 이 작업의 검증된 변경만 커밋하고 push하지 않으며 자동 검사와 실제 UI 확인 결과를 구분해 보고한다.

## 검증 상태와 수용한 제약

- 정확한 npm 0.22.0 release의 README, config parser, preset, editor/input 구현을 확인했다. Nix package 선택, default/above 배치, welcome off, native compact, bash shortcut null/completions false, zero delayed send, auto follow-up false와 workingVibe off를 설정했다.
- 로컬 prompt 구현과 그 전용 검사를 제거했다. Usage provider는 수정하지 않았고 fast provider는 표시값과 model-select 재발행만 변경했다. Shell과 Lens는 각 스펙에 따라 독립적으로 전환했으며, 질문 package와 subagent 정책, 범용 rounded-tools는 유지했다.
- `modules/pi/tests/powerline-integration.test.ts`는 실제 upstream component를 사용한다. 독립된 상태·입력 검사는 통과했지만 mouse 회귀 검사는 일반 assertion으로 실패를 유지한다. Width 40, padding 1, `abcdefghij`의 화면상 `d`를 클릭하면 base Pi는 x=4 → col=3, Powerline은 x=7 → col=6이다. 기대값은 col=3이다.
- 원인은 release `index.ts`의 렌더링 경로에서 세 열의 prefix를 추가하지만 `BashModeEditor`가 상속한 Pi `handleMouse`의 좌표를 변환하지 않는 것이다. 로컬 renderer/patch 또는 다른 pin으로 우회하지 않았다. 설정에서 해당 변환을 제공하는 문서화된 옵션도 확인되지 않았다.
- 독립 worktree에서 발견한 마우스 회귀를 부모 세션에서도 재현했다. Shell과 Lens를 함께 통합한 전체 suite는 199 pass, 1 fail이며, 유일한 실패는 수용한 마우스 회귀다. 세 패키지와 status provider, rounded-tools를 함께 로드했을 때 중복 도구·명령은 없었다. 자동 검사는 실제 TUI에서 overlay를 동시에 사용하는 검증을 대신하지 않는다.
- 실제 terminal의 한국어 IME, mouse selection, 좁은 Herdr pane, 설치된 질문·shell overlay와 rounded-tools의 동시 표시 및 focus 복원은 미검증이다. Headless cursor-marker/input 검사나 Node/jiti 성공으로 이 조건을 대체하지 않는다. Home Manager 적용, 인증/실사용 세션 접근은 하지 않았다.

재현은 unpacked npm release를 임시 디렉터리에 준비한 뒤 다음 명령으로 한다. Runner는 release를 임시 복사하고 설치된 Pi 의존성을 연결한다. Package directory를 지정하지 않으면 prerequisite 오류로 실패한다. 필요한 mouse 검사를 skip하거나 expected failure로 바꾸지 않는다.

```bash
PI_POWERLINE_PACKAGE_DIR=/path/to/unpacked/package \
  bash modules/pi/tests/run.sh powerline-integration.test.ts
```

사용자는 마우스 위치 오류가 남아 있는 0.22.0의 통합을 승인했다. 이는 회귀가 해결됐다는 뜻이 아니며, 실패하는 검사를 skip하거나 기대 실패로 바꾸지 않는다. 실제 사용자 환경 적용은 사용자가 수행한다.
