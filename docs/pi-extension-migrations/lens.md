# Pi LSP와 AST 기능을 pi-lens로 통합

상태: 독립 리뷰와 Linux 격리 검사를 거쳐 구현을 main에 통합했다. 실제 Home Manager 적용은 하지 않았다. 기준은 2026-11-12, Pi 1.5.2, pi-lens 4.4.12다. 부모 세션에서 실제 package와 언어 서버 검사를 다시 실행했으며, Nix hover의 유효 응답과 Rust 실행, macOS 및 실제 TUI 검증은 여전히 남아 있다. 자동 수정과 명시적 AST apply가 연결된 upstream 제약 때문에 보수적인 초기 수정 정책을 유지한다.

## 목표와 완료 조건

패치된 `pi-lsp`, 로컬 LSP refactoring 코드와 별도 AST extension을 `npm:pi-lens@4.4.12`로 대체한다. Nix는 실행 파일과 설정을 공급하고 Lens는 LSP 연결, diagnostics, navigation 및 AST 도구 구현을 소유한다. 기존 도구 이름과 preview token을 재현하는 compatibility layer를 만들지 않는다.

완료 조건은 Lens 단일 로드, 아래 기능 계약의 검증, 기존 패치 및 구현의 제거, 설정 소유권 정리와 기능 차이의 문서화다. 질문, shell, prompt UI 전환 없이도 완료할 수 있어야 한다.

## 시작할 때 읽을 자료

- `AGENTS.md`, `modules/pi/README.md`, `modules/lsp/default.nix`: 검증 정책, 현재 LSP 동작, 다른 editor도 사용하는 실행 파일 공급.
- `modules/pi/default.nix`와 전환 전 원본인 modules/pi/extensions/pi-lsp.nix, modules/pi/lsp-refactor.patch, modules/pi/files/lsp.json.
- 전환 전 구현인 modules/pi/files/extensions/lsp-refactor/와 modules/pi/files/extensions/ast-transform/, 유지할 `modules/pi/files/extensions/rounded-tools.ts` 및 대응 테스트. 이미 제거된 경로는 이전 commit에서 확인하고 재생성하지 않는다.
- [4.4.12 설정](https://github.com/apmantza/pi-lens/blob/v4.4.12/docs/configuration.md), [환경 변수](https://github.com/apmantza/pi-lens/blob/v4.4.12/docs/environment-variables.md), [navigation 구현](https://github.com/apmantza/pi-lens/blob/v4.4.12/tools/lsp-navigation.ts), [AST replace 구현](https://github.com/apmantza/pi-lens/blob/v4.4.12/tools/ast-grep-replace.ts).
- [Release QA](https://github.com/apmantza/pi-lens/blob/v4.4.12/docs/release-qa-baseline.md): 1.5 계열의 지원 선언과 실제 QA 실행 증거는 구분되어 있으므로 현재 host에서 확인한다.

착수 시 Git 상태와 실제 Pi 버전을 확인하고 사용자 변경을 보존한다. 호환성 문제를 해결하기 위해 Pi 자체, Nixpkgs 또는 다른 extension을 함께 업데이트하지 않는다. 추가 업데이트가 필요하면 별도 결정을 요청한다.

## 초기 기능 및 수정 정책

| 기능 | 목표 계약 |
| --- | --- |
| Diagnostics | edit/write 후 Lens feedback과 명시적인 diagnostics 조회를 사용한다. 서버 미응답과 unavailable을 clean 결과로 처리하지 않는다. |
| Navigation | hover, definition, references, symbols 등을 upstream `lsp_navigation`으로 사용한다. 인자 단위와 position encoding은 실제 schema를 따른다. |
| Semantic rename | 명시적인 요청과 적용 승인이 있을 때만 Lens preview/apply를 사용한다. 현재 single-use preview token과 동일한 보장이라고 설명하지 않는다. |
| Code actions | Lens의 조회 기능을 사용한다. 4.4.12의 `codeAction`은 기존 action 선택, resolve, preview ID 적용 도구와 동등하지 않다. 적용은 승인된 일반 edit 또는 editor에서 수행한다. |
| AST | Lens의 search, outline과 replacement preview로 통합한다. 초기 전환에서는 자동 수정 OFF 정책에 따라 Lens AST apply도 비활성 상태로 둔다. 실제 수정은 승인된 일반 edit 또는 editor를 사용한다. |
| 자동 변경 | 자동 format, autofix, warning autofix는 기본적으로 끈다. 기존 승인 경계와 관계없는 자동 source 변경을 추가하지 않는다. |
| 권한 | 기존 project trust와 승인 규칙을 유지한다. 서버 command 실행이나 파일 이동이 지원된다는 이유로 포괄적인 실행 권한을 부여하지 않는다. |

4.4.12는 `autofix.enabled: false` 또는 `--no-autofix`일 때 명시적인 `ast_grep_replace` apply도 거부한다. 자동 수정 OFF와 Lens AST apply를 동시에 제공한다고 약속하지 않는다. 초기 정책에서는 로컬 AST apply 기능이 제거된다는 점을 사용자 문서와 결과에 명시한다. AST apply를 다시 제공하기 위해 autofix를 켜거나 upstream을 patch하는 변경은 이 스펙에 포함하지 않으며 별도 승인이 필요하다.

Rename도 apply 호출에서 서버에 다시 요청하므로, 이전 preview의 정확한 bytes를 고정하는 계약은 제거된다. 파일 변경과 충돌에 대한 Lens의 실제 거부 동작을 검증하고 그 한계를 기록한다. 이전 token 계약이 필수인 요구가 추가되면 큰 wrapper를 만들지 말고 통합 범위를 다시 결정한다.

## 구성과 소유권

1. `modules/pi/default.nix`에서 npm `pi-lsp` 선언, patched source import와 설치 링크를 제거하고 Lens를 버전 고정한다. Bundled skill과 prompt는 초기 로드에서 제외하고 upstream 도구 schema와 필요한 Pi 전용 지침을 사용한다.
2. Lens의 machine-global 설정은 runtime module에서 관리한다. 기본 catalog를 우선 사용하고 차이가 필요한 서버만 override한다. Canonical 위치는 `~/.pi-lens/config.json`의 `lsp` namespace이며, deprecated 설정 위치를 새로 만들지 않는다.
3. `PI_LENS_DISABLE_LSP_INSTALL=1`, `PI_LENS_DISABLE_TOOL_INSTALL=1`로 runtime 자동 설치를 끈다. 이 값이 package 자체의 npm dependencies나 lifecycle script를 제거하는 것은 아니므로 배포본의 설치 과정도 확인한다.
4. 기존 `modules/lsp` 실행 파일 공급은 유지한다. Python은 현재의 `ty` 선택을 명시적으로 유지한다. `nixd`, Go, TypeScript, Rust와 기타 현재 route는 Lens catalog 및 override로 대조한다. 지원되지 않는 dialect나 root 탐색 차이는 숨기지 않는다.
5. 자동 format/fix, 자동 test 실행, startup 전체 scan 및 요청하지 않은 분석 기능을 켜지 않는 최소 구성을 사용한다. 실제 설정 이름과 우선순위는 배포본으로 검증한다. 특히 프로젝트 설정이 machine-global 수정 정책을 바꿀 수 있다는 점을 검사한다. 기존 프로젝트 설정이 자동 변경을 켠다면 적용 전에 보고하고 정책을 확정한다.
6. Package-owned tool UI와 diagnostics를 사용한다. 범용 `rounded-tools` frame은 유지하되, 제거된 `pi-lsp-diagnostics` 전용 parser/renderer와 그 전용 검사만 정리한다. Lens 메시지를 옛 payload로 변환하지 않는다.

현재 모델, context window, compaction, MCP, web, subagent 정책, Python kernel, phase tasks, session artifacts와 Herdr 통합은 유지한다. Shell의 native bash 여부나 Powerline 설치 여부에 의존하지 않는다.

## 제거 및 문서 갱신 범위

- 전환 전 원본인 modules/pi/extensions/pi-lsp.nix, modules/pi/lsp-refactor.patch, modules/pi/files/extensions/lsp-refactor/와 소비가 사라진 기존 LSP 설정.
- 전환 전 AST 구현인 modules/pi/files/extensions/ast-transform/와 해당 설치 선언.
- 제거된 구현만 검사하는 테스트와 patched-source 전용 loader fixture. 새 동작에 필요한 안전·기능 검사는 Lens integration 검사로 대체하며, 실패를 숨기기 위해 검증을 삭제하지 않는다.
- `modules/pi/README.md`의 package 목록, LSP, semantic refactoring, AST, verification 절. 제거한 token 보장과 새 도구 이름을 정확히 반영한다.

기존 `pkgs.ast-grep`가 새 구성이나 다른 기능에도 필요한지 확인한 뒤 사용자가 관리하는 실행 파일을 보존한다. 다른 module의 LSP package나 Nix flake dependency는 정리하지 않는다. 지침 수정이 필요하면 `agent-core/README.md`를 읽고 canonical 원본만 갱신한다.

## 독립 실행과 공통 파일

`modules/pi/default.nix`, `modules/pi/tests/module-config.test.ts`, `modules/pi/tests/extension-loading.test.ts`, `modules/pi/README.md`는 다른 전환과 공유한다. 최신 목록에서 LSP/AST 항목만 교체한다. [질문](ask-user.md), [Shell](interactive-shell.md), [Powerline](powerline-footer.md) 선택을 이전 값으로 복구하지 않는다. `rounded-tools`의 범용 frame 유지가 공통 계약이며, prompt UI 교체는 이 작업의 범위가 아니다.

## 검증

격리된 HOME, agent directory, Lens home/cache와 disposable project를 사용한다. 실제 사용자 설정과 source를 fixture로 수정하지 않는다.

- 실제 Node/jiti loader와 생성 settings에서 Lens가 한 번 로드되고 기존 LSP/AST 도구는 등록되지 않는다. Package 자체 peer/native dependencies와 startup 오류를 확인한다.
- 자동 설치를 끈 상태에서 Nix/PATH의 실행 파일로 서버가 시작된다. 누락된 binary는 오류나 unavailable로 보고하고 자동 다운로드하지 않는다.
- Nix, TypeScript, Go, Python `ty`, Rust 중 사용 가능한 각 서버에 대해 의도적인 오류, 수정 후 diagnostics, hover/definition/reference를 확인한다. 나머지 현재 route도 설정 누락 없이 대조한다. 실행하지 못한 route는 따로 기록한다.
- 요청하지 않은 formatter, fixer, test runner 또는 scan이 fixture를 바꾸거나 명령을 실행하지 않는다. 최소 설정과 프로젝트 override의 실제 효과를 확인한다.
- Multi-file rename의 preview와 승인 후 apply, preview 이후 파일 변경, code action 조회, AST search/preview를 확인한다. AST apply는 초기 정책에 따라 거부된다.
- `/reload`, session 교체와 종료 뒤 불필요한 서버가 남지 않고, `rounded-tools` 안에서 feedback과 expansion이 표시된다.

좁은 검사부터 실행한다.

```bash
bash modules/pi/tests/run.sh module-config.test.ts extension-loading.test.ts
bash modules/pi/tests/sync-settings.sh
uv run --no-project scripts/check-paths.py
uv run --no-project scripts/check-paths.py modules/pi/README.md docs/pi-extension-migrations/lens.md
```

추가한 Lens integration 검사와 영향받은 rounded-tools 및 custom-message 검사를 실행한다. Nix formatting과 최종 configuration 검증은 `modules/pi/README.md`를 따른다. 오래된 `PI_LSP_PACKAGE_DIR` fixture가 skip되었다는 결과를 Lens 검증으로 대신하지 않는다. 서버 기능과 process 수명은 설정 snapshot만으로 검증하지 않는다.

## 구현 및 검증 기록 (2026-11-12)

- `modules/pi/default.nix`는 Lens 4.4.12만 LSP/AST 제공자로 선택하고 bundled skill/prompt를 제외한다. `modules/pi/files/lens.json`을 canonical machine-global 위치에 연결한다. 두 runtime install 차단 환경 변수, format/autofix/warning-fix/test/startup-scan OFF, minimal startup과 비-LSP dispatch runner 거부 목록을 실제 4.4.12 schema/loader로 확인했다.
- Python은 `ty server` override와 Jedi fallback 거부로 유지한다. TypeScript catalog의 별도 tsserver 탐색이 Nix wrapper를 찾지 못하는 unavailable 결과를 확인한 뒤, `typescript-language-server --stdio` launcher override로 Nix가 제공하는 tsserver를 사용했다. 다른 서버 실행 파일과 `pkgs.ast-grep`는 그대로다.
- 원래 명시한 patched source, patch, 설정, 로컬 LSP refactoring/AST 구현과 그 전용 테스트를 제거했다. 범용 rounded frame은 유지하고 옛 diagnostics parser/renderer만 제거했다. 새 integration 검사는 `modules/pi/tests/lens.test.ts`, `modules/pi/tests/fixtures/lens.mjs`, `modules/pi/tests/fixtures/lens-stalled-server.mjs`에 있다. Canonical Pi 지침과 그 Pi-only renderer hash만 의도한 기능 차이에 맞춰 갱신했다.

### 실행 증거

검사는 x86_64 Linux, Node 24.21.0, Bun 1.4.4, 설치된 Pi 1.5.2에서 실행했다. npm exact-release tarball의 설정 schema, configuration/environment/QA 문서, navigation/AST source와 dependency lifecycle을 읽었다. Scratch 설치의 dependency 16개, ast-grep platform binary 복사 postinstall, upstream install-selftest 12 passed / 0 failed를 확인했다. Selftest 자체는 host-provided import에서 멈추는 제한이 있으므로, 전체 extension 로드는 별도의 실제 Pi Node/jiti 검사로 확인했다. 저장소 dependency pin이나 global install은 추가하지 않았다.

| 검사 | 관측 결과 |
| --- | --- |
| 생성 settings + Node/jiti loader | Lens 한 번, rounded-tools 한 번; 옛 LSP/AST 도구 없음; bundled skill/prompt 없음 |
| 자동 변경/실행 | 기본 정책에서 fixture bytes 유지, test sentinel 미생성, installer/formatter/추가 analyzer 실행 없음. 읽기 전용 `npm config get prefix` 발견은 설치로 세지 않음 |
| 프로젝트 override | Disposable `.pi-lens.json`의 autofix true가 실제 AST apply를 허용함. 이어서 CLI no-autofix가 다시 거부함. Global 기본값은 강제 정책이 아님. Format/warning-fix의 project provenance도 확인했으며, 이 fixture의 format은 CLI로 차단함 |
| Diagnostics | 실제 edit/write hook feedback과 명시 조회; TypeScript/Go/ty/Nix의 의도적 오류 및 수정 후 clean 확인 |
| Navigation | TypeScript/Go/ty hover·definition·references 유효 응답. Nix definition/references 유효 응답; hover는 advertised support에도 빈 결과라 유효 응답 검증 미완료 |
| Rename | TypeScript multi-file preview/apply; preview 후 변경은 유지한 채 새 요청을 적용함. Apply 요청 중 mutation queue를 잡고 바꾼 파일은 stale로 거부하고 다른 파일 bytes 유지 |
| Code actions / AST | 실제 TypeScript codeAction 목록; AST search/outline/replacement preview, 기본 정책의 apply 거부, package renderer의 rounded frame 및 expansion |
| 실패 서버 | 존재하지 않는 binary는 no_server/unconfirmed, 응답하지 않는 fixture LSP는 hover timeout 및 diagnostics timedOut/unconfirmed. 0개를 clean으로 오인하지 않음 |
| Lifecycle | 실제 SDK `/reload`, new-session 교체, quit 후 fixture가 시작한 서버 8개 모두 종료 확인. Provider turn 없이 real registered execute와 Pi hook dispatcher를 구동함 |
| 설정 평가 | pylv-denim의 canonical Lens config와 두 install 차단 변수, old LSP/AST 설치 부재, rounded 유지 확인. `nix flake check --no-build --offline` 성공; aarch64-darwin은 host 비호환으로 생략됨 |

Full suite 명령은 `PI_SUBAGENT_CHILD=0 PI_LENS_PACKAGE_DIR=/tmp/lens-migration-inspect/install/node_modules/pi-lens bash modules/pi/tests/run.sh`였고 **254 passed / 0 failed**였다. 자식 실행 환경을 그대로 상속한 첫 full suite는 parent-only UI test 54개가 실패했다. 원인을 child marker로 확인하고 테스트 프로세스에만 `PI_SUBAGENT_CHILD=0`을 지정해 통과했으며, 제품의 delegation 정책은 바꾸지 않았다. 이 설정은 실제 자식 실행을 parent로 바꾸는 우회가 아니라 mock/fixture test의 parent UI 조건이다.

Focused module-config/extension-loading/rounded-tools/custom-message suite는 **28 passed / 0 failed**, settings sync는 **6 PASS**, agent-core pytest는 **37 passed**, `agent-core check`는 성공했다. Nix formatting, JSON 및 기본/추가 문서 경로 검사도 통과했다. 변경 surface를 제외한 질문, shell, prompt/Powerline, delegation, MCP, 모델, Python kernel과 server packages는 유지한다.

### 미검증과 적용 경계

- **필수 잔여 증거:** Nix hover는 빈 응답이어서 유효 hover 기능을 확인하지 못했다. Rust는 PATH의 rustup shim에서 rust-analyzer 실행 파일을 찾지 못했으며, component를 설치하지 않았다. 둘을 검증 완료로 표시하지 않는다.
- JSON/JSONC, CSS/SCSS/Less, HTML/HTM, Lua, Bash, Terraform/tfvars, YAML, Markdown, TOML 등 나머지 route는 effective catalog와 거부 목록을 대조했지만 실제 서버 질의는 하지 않았다. `.zsh`는 설치하지 않은 shuck으로 바뀌며 `.markdown`은 catalog route가 없다. 전체 route/root 차이는 `modules/pi/README.md`에 기록했다.
- Fixture는 provider 호출을 하지 않고 scratch auth/models 경로만 사용한다. 최종 harness는 process.env 전체 상속 대신 PATH/LANG과 명시적인 격리 HOME/agent/Lens/cache 변수만 전달하고, Pi offline 및 `allowModelNetwork: false`를 명시한다. 기존 rustup toolchain 경로는 실행 가능 여부의 읽기 전용 확인에만 사용한다. Project trust yes는 테스트가 만든 disposable settings/project에만 설정하며, 실제 trust 설정을 수정하지 않는다.
- `effective_config`는 값이 아닌 provenance를 보고하며, 4.4.12에서는 runtime에서 무시할 global-only project 입력도 project 출처로 표시할 수 있다. Snapshot만으로 강제 정책이나 실행을 검증했다고 주장하지 않는다. Private project 설정은 스캔하지 않았으므로 사용자 적용 전 scoped 정책 충돌 확인이 남는다.
- macOS, 실제 대화형 사용자의 TUI 조작, provider가 수행하는 실제 모델 turn과 Home Manager 적용은 검증하지 않았다. Independent reviewer의 승인과 필요한 언어 증거가 남는다. 빌드, deploy, push 또는 live server/cache 정리는 하지 않았다.

## 적용과 복구

실제 Home Manager 적용은 사용자가 수행하고 새 Pi session에서 확인한다. 예전 LSP와 Lens를 동시에 활성화하는 중간 상태를 배포하지 않는다. 실행 중인 LSP, 사용자 cache 또는 session history를 승인 없이 삭제하지 않는다.

복구는 이 작업에서 제거한 원본과 package 선택, 설정을 해당 커밋 기준으로 복원하는 방식으로 한다. 다른 전환의 변경은 유지한다. 이 작업만 커밋하고 push하지 않는다. 결과에는 제거된 유지보수 대상, 기능 차이, 검증한 언어·플랫폼, 미검증 항목과 사용자 적용 여부를 기록한다.
