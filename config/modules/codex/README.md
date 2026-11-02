# Codex

`files/config.toml`은 Codex의 기본 모델과 effort, MCP, 기능 설정을 관리한다. 메인 에이전트는 `gpt-6.1-sol`과 `high` effort를 사용하며, worker의 기본값은 `gpt-6-luna`와 `medium`이다. 세션이나 사용자 설정에서 이 값을 덮어쓸 수 있다.

Cloudflare 작업은 공통 Home Manager profile이 설치하는 `cf` CLI를 사용한다. 인증과 명령 검색은 [Cloudflare CLI 설정](../../README.md#cloudflare-cli)을 참고한다. MCP 설정 변경 후에는 Codex 세션을 다시 시작한다.

Codex가 사용자에게 웹 페이지, 로컬 미리보기, 인증 링크를 열어 줄 때는 시스템 기본 브라우저를 사용한다. [공통 지침](../../../agent-core/rules/AGENTS.md#tool-specific-workflows)은 macOS의 `open <URL>`과 같은 운영체제의 URL 열기 기능을 사용하도록 지시한다. 브라우저 자동화와 테스트에는 공통 profile이 설치하는 `agent-browser`를 사용하며, 기본적으로 headless 모드로 실행한다. Playwright MCP는 등록하지 않는다. 이 동작은 에이전트 지침이며, Codex 앱의 링크 클릭 동작을 강제하는 설정은 아니다. 지침 변경은 아래 [적용](#적용) 절차로 설치한다.

TUI 구문 강조는 밝은 터미널 배경에 맞춰 기본 제공 `one-half-light`를 사용한다. 공통 테마의 Codex용 export는 없으므로 canonical 배경색 조정은 이 구문 강조 테마에 반영되지 않는다. [공식 CLI 테마 문서](https://learn.chatgpt.com/docs/cli-customization)에서 설명하는 사용자 지정 테마는 별도의 `.tmTheme` 파일이 필요하다.

[Codex 패키지](../../packages/apps/codex/package.nix)는 공식 `codex-package` 번들 전체를 설치한다. 백그라운드 서버가 패키지를 인식하도록 `codex-package.json`, `codex-path`, `codex-resources`를 실행 파일과 함께 유지한다. 패키지 변경은 환경에 맞는 Home Manager 또는 NixOS switch 후 새 Codex 세션에 적용된다.

## LSP 코드 탐색

`lsp` MCP 서버는 [lspi](https://github.com/Latias94/lspi)를 통해 정의, 참조, hover, 문서 심볼과 진단을 제공한다. `codex-lsp-mcp`는 [LSP 모듈](../lsp/default.nix)이 설치한 `ty`, `gopls`, `typescript-language-server`, `terraform-ls`, `nixd`를 PATH에서 사용한다. Rust는 활성 rustup toolchain의 `rust-analyzer`를 사용한다. Python에는 Pyright를 연결하지 않는다.

[files/lspi.toml](files/lspi.toml)은 [Claude의 LSP 플러그인](../claude/README.md#installed-plugins)과 같은 서버 및 확장자 매핑을 사용한다. Python, Go, Rust, TypeScript와 JavaScript, Terraform, Nix를 지원한다. lspi 0.2.0은 서버 설정마다 language ID 하나를 사용하므로 TS, TSX, JS, JSX와 Terraform의 `.tf`, `.tfvars`는 각각 구분한다. 또한 빈 인자 목록에 `--stdio`를 추가하므로 이를 지원하지 않는 Go와 Nix에는 각각 `serve`와 `--log=error`를 지정한다. 서버가 지원하는 LSP 기능만 사용할 수 있다.

MCP 프로세스의 작업 디렉터리를 workspace root로 사용하므로 작업할 프로젝트 루트에서 Codex를 시작한다. 하위 프로젝트의 root marker를 자동 탐색하지 않으며, 명시적인 설정 파일을 사용하므로 프로젝트의 별도 lspi 설정은 읽지 않는다. 읽기 전용 모드에서는 rename과 서버 제어 도구를 제공하지 않는다. 도구 입력의 행과 열 번호는 1부터 시작한다.

환경에 맞는 Home Manager 또는 NixOS switch를 실행한 뒤 새 Codex 세션의 `/mcp`에서 `lsp` 연결을 확인한다. Switch 전에는 새 `codex-lsp-mcp` 실행 파일이 없으므로 MCP 서버가 시작되지 않는다. 기존 세션의 `pyright` 도구는 세션을 다시 시작할 때 교체된다.

## 모델별 위임

[Codex adapter](../../../agent-core/adapters/codex.md)는 독립적인 작업을 일찍 위임하고, worker를 생성할 때 작업 성격에 맞는 모델과 effort를 명시하도록 지시한다. 메인 에이전트는 사용자와의 대화, 요구사항 해석, 의사결정, 결과 통합과 최종 검증을 담당한다. 모델별 작업 범위와 재위임 기준은 adapter에서 관리한다.

Worker는 기본적으로 `fork_turns = "none"`으로 생성하고, 필요한 요구사항과 제약, 담당 경로, 완료 기준을 작업 메시지에 포함한다. 부모 대화가 필요한 경우에는 필요한 최소 턴 수를 선택한다. 전체 대화가 필요한 경우에만 `fork_turns = "all"`을 사용하고 작업 메시지에 이유를 명시한다. 전체 대화를 상속하는 worker는 부모의 모델과 effort도 상속한다.

일반적인 작업과 설계는 `gpt-6.1-sol`이 담당한다. `gpt-6-astra`는 여러 제약이 얽힌 아키텍처 설계, 여러 하위 시스템에 걸친 깊은 디버깅 등 아주 복잡한 작업에만 subagent로 사용한다. 이때도 메인 에이전트는 `gpt-6.1-sol`을 유지한다.

이 동작은 에이전트가 따르는 위임 지침이며, 프롬프트의 복잡도를 계산해 모델을 강제로 선택하는 별도 라우터는 없다. Worker의 모델과 effort 선택은 메인 세션의 모델과 effort를 변경하지 않는다. 생성된 지침과 shared skill의 구조는 [Agent core README](../../../agent-core/README.md)에서 확인한다.

## Herdr subagent sidebar

Herdr 안에서 실행한 Codex는 Agents 항목 아래에 직계 subagent의 상태, 작업 이름, 실제 모델, 최근 메시지를 두 줄씩 표시한다. 첫 줄에는 상태 아이콘, 이름, 모델만 표시하고 둘째 줄의 메시지는 옅게 표시한다. 이름은 Codex의 `agent_path`에서 가져오며, 모델은 자식 세션의 `turn_context`를 사용한다. 별도의 접기 버튼이나 terminal pane은 만들지 않는다. 표시 개수와 레이아웃은 [Herdr README](../herdr/README.md#subagents)에서 확인한다.

상태는 Codex의 작업 이벤트로 결정한다. 실행 중은 초록색 `●`, 완료는 파란색 `✓`, 중단은 노란색 `■`, 실패는 빨간색 `×`로 구분하며, 완료·중단·실패는 이벤트 발생 후 5초 동안 표시한다. 같은 subagent에 후속 작업을 보내면 다시 실행 중으로 바뀐다. 도구 실행이나 메시지 갱신은 작업 상태를 변경하지 않는다. 출력이 없는 동안에도 종료 이벤트를 받기 전에는 실행 중 상태를 유지한다.

Codex의 subagent 기능으로 생성한 자식은 부모가 답변을 마친 뒤 백그라운드에서 계속 작업해도 감시한다. 셸에서 `codex exec ... &`로 직접 실행한 별도 세션은 이 부모·자식 연결에 포함되지 않는다.

[files/hooks.json](files/hooks.json)의 `SessionStart`, `SubagentStart`, `SubagentStop`, `SessionEnd` 훅이 Home Manager로 설치한 `~/.local/bin/codex-herdr-subagents`를 호출한다. 훅은 Herdr pane마다 감시 프로세스 하나를 실행하고, 감시 프로세스는 부모와 자식의 로컬 JSONL 기록을 읽어 `codex:herdr-subagents` metadata를 갱신한다. 부모 세션 종료나 Codex 프로세스 종료 시 표시를 지운다. Herdr 환경 변수가 없는 터미널에서는 실행하지 않는다.

JSONL 파일 읽기는 Claude watcher와 같은 [공통 reader](../herdr/README.md#shared-jsonl-reader)를 사용한다. Nix는 watcher와 공통 파일을 함께 패키징하고, 기록 탐색과 상태 해석은 Codex 모듈에서 처리한다.

[Codex launcher](files/codex-launcher.sh)는 `HERDR_ENV=1`일 때 `--no-daemon`을 추가한다. Codex 0.162.1의 공유 daemon은 처음 실행된 pane의 환경 변수를 다른 세션에도 사용하므로, pane별 훅은 각 Codex 프로세스에서 실행해야 한다. Launcher는 Home Manager profile에서 원본 `codex`보다 높은 우선순위로 설치되며, Herdr 밖에서는 인자를 그대로 전달한다. 공유 daemon에서 실행된 훅은 pane 소유자를 변경하지 않고 `--no-daemon`으로 재시작하라는 오류를 기록한다. 패키지의 실행 파일을 절대 경로로 호출하여 launcher를 우회하는 경우에도 Herdr 안에서는 `--no-daemon`이 필요하다.

`SessionEnd` 훅의 timeout은 Codex가 허용하는 최대값인 3초로 설정한다. 3초를 초과하면 Codex가 시작 시 경고를 표시하고 실제 제한을 3초로 낮춘다.

실행 중인 자식의 기록을 읽지 못하거나 기록이 손상되면 회색 `○`와 마지막 메시지를 표시한다. 기록을 다시 읽어 작업 상태를 확인하면 정상 표시로 돌아간다. 감시 프로세스 자체가 종료되거나 Herdr에 전송할 수 없으면 새로운 상태를 표시할 수 없으므로 마지막 metadata가 15초 뒤 만료된다.

활동 줄은 최근 assistant 메시지의 공백과 제어 문자를 정리한 한 줄 발췌문이다. 별도의 모델 호출 없이 80자 이내로 표시하고, 길면 말줄임표를 붙인다. 도구 이벤트는 메시지를 덮어쓰지 않으며, 작업 완료 시 `last_agent_message`가 있으면 마지막 응답으로 갱신한다. 새 작업이 시작되면 이전 메시지를 지우고, 메시지가 없으면 활동 줄을 숨긴다. 고정 안내 문구는 영어이며 메시지는 원문 언어를 유지한다. Reasoning, 암호화된 메시지, 원시 명령어는 표시하지 않는다. 이 연동은 Codex CLI 0.155.1과 Herdr 0.9.1을 기준으로 검증했다. Codex의 내부 JSONL 형식은 안정된 API가 아니므로 CLI 업데이트 후 표시가 달라지면 실제 기록 형식과 parser를 확인한다.

부모의 정보 줄에는 `codex`와 부모 기록의 최근 `turn_context.model`만 표시한다. 첫 작업의 기록이 생기기 전에는 모델을 표시하지 않으며, 모델을 변경하면 다음 작업의 기록을 읽을 때 갱신한다. 자식 실행 수와 별도의 요약 문구는 표시하지 않는다.

감시 프로세스의 상태 파일은 쓰기와 탐색 권한이 있는 기존 `XDG_RUNTIME_DIR` 아래에 저장한다. 값이 없거나 절대 경로가 아니거나 해당 디렉터리를 사용할 수 없으면 시스템 임시 디렉터리를 사용한다. macOS에서 Linux용 `/run/user/<uid>` 값이 남아 있어도 해당 경로를 생성하지 않는다.

Linux와 WSL에서는 PID와 `/proc/<pid>/stat`의 시작 tick으로 부모와 감시 프로세스를 식별한다. WSL의 시계 보정으로 `ps lstart`의 시작 시각이 달라져도 실행 중인 프로세스를 종료된 것으로 판단하지 않는다. macOS에서는 기존 `ps` 기반 식별을 사용한다.

## 적용

Superpowers는 공식 Codex marketplace의 `superpowers@openai-curated` 플러그인으로 설치한다. Home Manager activation은 이 플러그인을 설치하고 활성화한 뒤, `~/.agents/skills/superpowers`가 예전 `~/.codex/superpowers/skills`를 가리키면 해당 링크를 제거한다. 설치가 실패하면 activation을 중단하고 `~/.codex/nix-setup.log`에 오류를 기록한다. 다른 대상의 링크와 일반 디렉터리는 보존한다.

현재 사용자 환경에 플러그인 설치를 적용하려면 다음 명령을 실행하고 새 Codex 세션을 시작한다.

```bash
codex plugin add superpowers@openai-curated
```

Ponytail은 `DietrichGebert/ponytail` marketplace의 `ponytail@ponytail` 플러그인으로 설치한다. Home Manager activation은 설치 목록을 조회하고, 이미 설치되어 있으면 현재 버전과 활성화 상태를 유지한다. 설치되어 있지 않을 때만 marketplace 등록과 플러그인 설치를 실행하며, 조회나 설치가 실패하면 activation을 중단한다. 업데이트가 필요할 때는 `codex plugin add ponytail@ponytail`을 직접 실행한다. `$ponytail:ponytail-review`로 현재 변경을 검토할 수 있다.

처음 설치하거나 업데이트한 뒤에는 Codex의 `/hooks`에서 Ponytail lifecycle hook을 검토하고 승인한다. Codex는 `~/.codex/config.toml`의 `hooks.state`에 승인을 저장하며, 플러그인 hook은 선언과 플러그인 파일 내용의 hash를 함께 확인한다. [공식 hook 문서](https://learn.chatgpt.com/docs/hooks#review-and-trust-hooks)에서 설명하듯 승인 대상이 바뀌면 재승인이 필요하다. 단순한 switch는 설치된 Ponytail을 갱신하지 않으므로, switch에 플러그인 업데이트가 따라와 재승인을 요구하는 일을 방지한다. 실제 hook이나 플러그인 내용이 바뀌면 다음 세션에서 다시 승인한다.

Impeccable은 [agent-core manifest](../../../agent-core/manifest.toml)의 외부 skill source로 관리한다. Agent skills activation이 공식 저장소의 `.agents/skills/impeccable`을 공용 `~/.agents/skills/impeccable`에 설치하므로 Codex와 Pi에서 발견할 수 있다. `$impeccable audit`, `$impeccable polish`로 호출한다. Claude는 별도 공식 플러그인을 사용하므로 shared installer의 Claude target에는 중복 설치하지 않는다. Codex의 자동 디자인 검사 hook은 프로젝트별 선택 사항이며 전역 설치에 포함하지 않는다. 필요한 프런트엔드 프로젝트에 [공식 hook 설정](https://impeccable.style/docs/hooks)을 추가하면 Codex의 `/hooks`에서 검토하고 승인한다. Agent-core가 소유한 전역 skill을 다른 installer로 덮어쓰지 않는다. Launcher는 첫 실행 때 고정된 engine binary를 내려받을 수 있다.

`sites-building`과 `sites-hosting`은 공식 remote marketplace의 Sites 플러그인이 스크립트와 템플릿을 포함해 제공한다. 일반 웹 artifact 생성과 Sites 배포에는 이 플러그인을 사용한다. 사용자 환경에서 다음 명령으로 설치하고 새 Codex 세션을 시작한다. Nix activation은 이 설치를 관리하지 않는다. 실제 Sites 사용에는 선택한 계정과 workspace의 기능 접근 권한도 필요하다.

```bash
codex plugin add sites@openai-curated-remote
```

Decisions API를 사용하는 기능은 `$openai-decisions` skill로 설계하고, API client는 `with-openai`로 실행한다. 모델과 API 사용법, agenix 인증, 적용 절차는 [OpenAI API README](../openai-api/README.md)에서 확인한다.

[default.nix](default.nix)은 Home Manager의 사용자 파일과 activation을 구성한다. Standalone Home Manager는 [system-activation.nix](system-activation.nix)에서 생성한 스크립트로 `/etc/codex/config.toml`을 checkout의 `files/config.toml`에 연결한다. 스크립트는 관리할 경로를 모두 검사한 뒤 필요한 sudo 작업을 실행하며, 일반 파일은 덮어쓰지 않는다.

NixOS는 [system.nix](system.nix)의 `environment.etc`로 시스템 설정을 설치하고 Home Manager의 시스템 activation을 생략한다. `~/.codex/config.toml`은 사용자 설정을 보존하며 activation이 덮어쓰지 않는다.

`~/.codex/AGENTS.md`는 agent-core의 immutable output을 사용한다. [Agent skills activation](../agent-skills/README.md)은 Home Manager switch마다 `skills@1.7.2`로 Codex/Pi 공통 경로인 `~/.agents/skills`의 skill을 설치하거나 갱신한다. 활성 agent 전체가 같은 작업에서 결정한 최신 외부 source SHA를 사용하며 activation에는 network가 필요하다. 기존 standalone 환경의 `/etc/codex/skills` 링크는 선언을 제거해도 남으므로, 최초 switch 전에 관리 대상과 backup을 확인하고 승인한 뒤 별도로 인계해야 중복 discovery와 설치 충돌을 피할 수 있다.

Adapter 변경을 적용하려면 환경에 맞는 `home-manager switch --flake .#<environment>` 또는 `nixos-rebuild switch --flake .#<host>`를 사용자가 실행한 뒤 새 Codex 세션을 시작한다. 최초 skill 전환에는 [Agent core README](../../../agent-core/README.md#skill-설치와-복구)의 backup과 소유권 인계가 필요하다. Switch가 성공하면 별도 CLI 설치 없이 discovery를 검증한다.

Herdr 연동도 같은 switch로 훅과 launcher를 설치한 뒤 새 셸에서 Codex를 시작한다. 기존 세션을 이어가려면 해당 pane의 Codex를 종료한 뒤 `codex resume <session-id>`를 실행한다. Switch 전에도 `codex --no-daemon resume <session-id>`로 pane별 실행을 선택할 수 있다. 실행 중인 세션의 daemon 연결과 훅 설정은 자동으로 변경되지 않는다. Herdr 표시 설정을 변경했으면 `herdr server reload-config`도 실행한다.
