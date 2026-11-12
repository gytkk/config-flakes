# Pi background shell을 pi-interactive-shell로 전환

상태: 구현됨. Pi 1.5.2와 pi-interactive-shell 0.18.0의 Linux 격리 PTY 검사에서 27초 무출력 후 성공/실패, 패키지 완료 알림과 로그, 입력 및 overlay takeover/attach/stop, reload와 session shutdown hook, native bash/artifact 경계를 확인했다. 모델 없이 실제 배포본과 Pi loader/renderer를 실행한 fixture 검사이며, 사용자 Home Manager 적용과 macOS 검증은 별도다. 조사 기준은 2026-11-12다.

## 목표와 완료 조건

`pi-bg-bash`의 built-in bash override를 제거하고, 짧은 비대화형 명령은 Pi 기본 `bash`, 장기 실행과 입력이 필요한 명령은 `npm:pi-interactive-shell@0.18.0`으로 실행한다. 사용자는 패키지의 overlay, attach 및 작업 관리 UI를 사용한다. 기존 `/bg`의 화면이나 도구 API를 재현하는 호환 계층을 만들지 않는다.

완료 조건은 기본 bash 복원, 패키지 단일 로드, 실제 종료에 근거한 결과 처리, 장기 실행의 알림 및 관리 UI 검증, 기존 bg 전용 UI와 지침의 제거다. 다른 전환 작업 없이 이 상태로 사용할 수 있어야 한다.

## 시작할 때 읽을 자료

- `AGENTS.md`와 `modules/pi/README.md`: 설치 소유권, background 작업 수명, 현재 위임 정책과 검증 절차.
- `modules/pi/default.nix`와 `modules/pi/files/settings.json`: package 선택. 전환 전 prompt 원본인 modules/pi/files/extensions/prompt-ui/는 남아 있는 경우에만 읽어 `/bg` 표시의 연결을 확인한다.
- `agent-core/adapters/pi.md`, `modules/pi/agents/`, `modules/pi/files/extensions/subagent/config.json`: shell과 subagent의 권한 및 역할을 구분하는 계약.
- `modules/pi/files/extensions/session-artifacts/index.ts`: native bash 출력과 외부 package 출력의 보존 경계.
- [0.18.0 README](https://github.com/nicobailon/pi-interactive-shell/blob/v0.18.0/README.md)와 [실행 및 종료 구현](https://github.com/nicobailon/pi-interactive-shell/blob/v0.18.0/index.ts).

작업 시작 시 Git 상태와 실제 Pi pin을 확인하고 다른 작업의 변경을 보존한다. 0.18.0의 문서는 macOS tested, Linux experimental을 명시하므로 현재 Linux/WSL에서의 실행 검증을 완료 조건으로 취급한다. 패키지의 native PTY 의존성이 설치된다는 것만으로 동작을 검증했다고 판단하지 않는다.

## 실행 계약

| 작업 | 목표 경로 |
| --- | --- |
| 파일 조사, 짧은 검사, 단발 명령 | Pi 기본 `bash`를 사용하고 종료 상태를 받는다. |
| 장기 빌드나 테스트 | `interactive_shell`의 background dispatch를 사용하고 실제 process exit로 성공과 실패를 판단한다. |
| 입력이 필요한 CLI | 패키지의 interactive UI에서 입력하거나 사용자에게 제어를 넘긴다. |
| 특정 출력 또는 상태 감시 | 구체적인 조건이 있는 경우에만 monitor 기능을 사용한다. |
| 결과 조회 | 완료 알림과 저장된 출력으로 확인한다. 같은 명령을 다시 실행하거나 대기용 polling을 반복하지 않는다. |

빌드와 테스트의 dispatch 호출에는 `handsFree.autoExitOnQuiet: false`를 명시한다. 0.18.0에는 quiet 상태에서 supervision을 끝내는 기본 경로가 있으므로, 조용해졌다는 알림을 성공이나 process 종료로 보고하지 않는다. 이 규칙은 Pi 전용 지침과 integration fixture에 반영하며 upstream 기본값을 바꾸는 fork나 실행 wrapper는 만들지 않는다.

Timeout, 사용자의 감독 중단, 실제 process exit, process-tree 종료는 별개의 결과로 취급한다. Stop 이후에는 패키지가 확인한 상태만 보고하고, 취소 요청을 보냈다는 사실을 종료 확인으로 바꾸지 않는다. `/reload`에서는 패키지가 작업을 유지하고 다른 session shutdown에서는 정리하는 계약을 실제 배포본으로 확인한다. Pi 종료 후에도 작업이 유지된다고 약속하지 않는다.

Shell 기능의 도입은 위임 권한을 추가하지 않는다. 기준 commit 2699454a의 현재 subagent 계약을 유지한다: 승인된 작업 안에서 병렬/background child, 독립 worktree와 명시적 follow-up은 허용하며 중첩 위임과 자동 resume 등은 계속 제한한다. `/spawn`, structured spawn 또는 shell의 agent CLI 실행을 별도 위임 경로로 사용하지 않는다. 일반 shell 자체가 agent CLI를 실행할 수 있으므로 이 지침을 보안 격리라고 표현하지 않는다.

## 구성과 변경 범위

1. `modules/pi/default.nix`에서 `pi-bg-bash` pin을 제거하고 새 package를 추가한다. Extension만 사용하고 bundled skill과 prompt는 제외한다. 기존 `pi-subagents`는 유지한다.
2. 일반 명령의 `bash`가 Pi built-in provider로 돌아오는지 확인한다. 기존 `background`, `waitFor`, `hold`, `bash_watch`, `bash_output` 등의 schema를 기본 bash에 요구하는 지침이 남지 않게 한다.
3. 패키지의 문서화된 설정으로 lean tool schema와 on-demand 로드를 구성한다. 설정 이름과 활성화 도구 이름은 배포본에서 확인한다. 추가 LLM 호출, semantic supervision과 output selection의 외부 전송은 활성화하지 않는다.
4. 전환 전 디렉터리인 modules/pi/files/extensions/prompt-ui/가 남아 있으면 `/bg (N)`, `bg-selection`, background count parser와 Down/Enter 전용 처리를 제거한다. 나머지 prompt, statusline, cursor 동작은 유지한다. 이미 Powerline 작업으로 디렉터리가 제거됐다면 재생성하지 않는다.
5. `modules/pi/README.md`의 package 목록과 background 사용법을 갱신한다. `agent-core/adapters/pi.md` 등 실제로 패키지 전용 이름을 요구하는 원본만 수정한다. 공통 지침을 바꾸기 전에는 `agent-core/README.md`를 읽고 renderer 검사를 따른다.
6. Module 설정, loader, prompt 관련 테스트와 shell integration 검증을 갱신한다. 기존 bg message 이름이 범용 rendering fixture로만 남은 경우와 실제 runtime 의존성을 구분한다.

설정은 runtime module이 소유한다. 사용자 HOME의 설치본을 patch하거나 별도 background task manager, 로그 watcher, 알림 bridge를 구현하지 않는다. `rounded-tools`, fast/usage, MCP, web, 모델과 context 설정은 유지한다.

## 출력 보존

기본 bash가 복원되면 `session-artifacts`의 기존 native provenance 검사를 통과하는 full output은 원래 계약대로 자동 보존할 수 있다. 이를 위해 검사 범위를 넓히지 않는다. Interactive-shell 로그는 패키지의 보존 및 조회 경로를 사용하며, 필요할 때 기존 artifact 도구로 명시적으로 등록한다. 임의 출력에서 경로를 추출하거나 다른 session의 로그를 탐색하는 자동 수집을 추가하지 않는다.

## 독립 실행과 공통 파일

[Powerline](powerline-footer.md)은 선행 조건이 아니다. Shell 작업이 먼저면 기존 prompt의 bg 연동만 제거하고, Powerline이 먼저면 이미 없는 코드를 건너뛴다. Powerline 쪽 sticky bash는 agent용 장기 실행 engine의 대체품이 아니다.

`modules/pi/default.nix`, `modules/pi/tests/module-config.test.ts`, `modules/pi/tests/extension-loading.test.ts`, `modules/pi/README.md`는 다른 전환과 공유한다. 현재 목록에서 shell 항목만 갱신한다. Prompt 디렉터리의 최종 삭제는 Powerline 작업이 소유하며, 그 변경과 합칠 때 삭제된 bg UI를 복구하지 않는다. [질문](ask-user.md)과 [Lens](lens.md) package 선택을 수정하지 않는다.

## 검증

격리된 agent directory와 disposable project에서 다음을 검사한다. 실제 사용자 작업을 kill하거나 테스트 대상으로 사용하지 않는다.

- 기본 bash의 성공, 비정상 exit, timeout이 upstream schema와 결과로 동작한다.
- 패키지가 한 번 로드되고 activation 후 `interactive_shell`을 호출할 수 있다. 오래된 bg 도구나 shortcut이 중복 등록되지 않는다.
- 출력 없이 기다린 뒤 종료하는 fixture가 quiet 종료 기준보다 오래 살아 있어도 완료로 오인되지 않는다. 성공과 실패 결과를 모두 확인한다.
- 완료 알림이 결과를 전달하고, 알림을 받은 뒤 status를 확인할 수 있으며 명령을 다시 실행할 필요가 없다.
- 입력 전달, 사용자 takeover, attach, stop, reload 및 session 교체의 수명이 패키지 계약과 맞는다. 테스트 종료 후 fixture process가 남지 않는다.
- Shell 출력과 패키지 UI를 `rounded-tools`와 함께 사용할 수 있다. Native bash artifact 보존과 외부 로그의 명시적 등록 경계가 유지된다.

좁은 module 및 loader 검사부터 실행한다.

```bash
bash modules/pi/tests/run.sh module-config.test.ts extension-loading.test.ts
bash modules/pi/tests/sync-settings.sh
uv run --no-project scripts/check-paths.py
uv run --no-project scripts/check-paths.py modules/pi/README.md docs/pi-extension-migrations/interactive-shell.md
```

추가한 shell integration은 `PI_INTERACTIVE_SHELL_PACKAGE_DIR`에 의존성을 갖춘 정확한 0.18.0 임시 설치 경로를 지정하고 `bash modules/pi/tests/run.sh interactive-shell.test.ts`로 실행한다. 이 검사는 Linux 전용이며, 의존성이 없어서 skip된 결과는 PTY 검증으로 세지 않는다. 영향받은 prompt/artifact 테스트도 실행한다. Nix formatting과 최종 configuration 검증은 `modules/pi/README.md`를 따른다. Linux/WSL PTY 또는 실제 알림을 검증할 수 없으면 해당 항목을 미검증으로 보고하고 전환 완료로 표시하지 않는다. macOS 검증 결과는 별도로 기록하며 Linux 결과로 대신하지 않는다.

## 적용과 복구

진행 중인 기존 bg 작업이 있으면 사용자에게 종료 또는 대기 여부를 확인한다. 승인 없이 작업을 중단하지 않는다. Home Manager 적용은 사용자가 수행하고 새 Pi session에서 확인한다. 기존 task ID와 실행 중 process를 새 패키지로 이관하는 기능은 만들지 않는다.

복구는 이 작업의 package 선택, 지침, bg UI 변경만 되돌린다. 다른 전환과 사용자 수정은 유지한다. 이 작업의 검증된 변경만 커밋하고 push하지 않으며, 결과에 package loading, 실제 PTY, 알림, 사용자 적용의 검증 상태를 각각 기록한다.
