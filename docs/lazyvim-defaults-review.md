# LazyVim 기본 설정 활용 검토

기존 조작을 유지하기 위해 추가한 개인 설정 중에서 키 매핑과 중복 설정부터 LazyVim 기본값으로 정리하는 것을 권장한다. Git, 상태줄, 수동 포맷은 기본 구현을 활용하고, Nix 연동과 의도적으로 선택한 편집 동작은 개인 설정으로 유지한다.

이 문서의 비교 기준은 2026-10-27의 LazyVim 전환 커밋 `ca8d121` 개인 설정과 LazyVim `v16.0.1` 커밋 `999700997f72227187d49d8b92667183dc7fc809`이다. 현재 설정은 아래 "기본 키 매핑으로 정리할 항목"과 "기본 구현을 활용할 항목"의 권장안을 따르며, 프로젝트 루트와 편집 옵션도 LazyVim 기본값을 사용한다. 실제 설정 구조와 사용법은 [Neovim README](../modules/vim/README.md)를 따른다.

## 기본 키 매핑으로 정리할 항목

아래 표의 `<Space>`는 현재 leader 키다. 개인 매핑을 삭제하면 상속할 수 있는 기본 매핑은 별도로 다시 정의하지 않는다.

| 기능 | 비교 시점의 개인 설정 | 권장하는 LazyVim 기본값 | 정리 효과 |
| --- | --- | --- | --- |
| 검색 재개 | `<Space>sr` | `<Space>sR` | `sr`을 검색 후 치환에 사용 |
| 현재 버퍼의 줄 검색 | `<Space>s/` | `<Space>sb` | `s/`을 검색 기록에 사용 |
| 버퍼 목록 | `<Space>b`, `<Space>fb` | `<Space>fb` 또는 `<Space>,` | `b`로 시작하는 버퍼 관리 키와 중복 해소 |
| 심볼 이름 변경 | `<Space>rn` | `<Space>cr` | 코드 관련 기능을 `c` 아래에 통일 |
| 구현 위치로 이동 | `gi` | `gI` | Vim의 원래 `gi` 동작인 마지막 입력 위치로 이동 후 입력 모드 진입을 사용 |
| Trouble의 LSP 목록 | `<Space>cl` | `<Space>cS` | `cl`을 기본 LSP 정보 화면에 사용 |
| Git 변경 부분 조작 | `<Space>hs`, `<Space>hr`, `<Space>hp` 등 | `<Space>ghs`, `<Space>ghr`, `<Space>ghp` 등 | 기본 Git 키 구성과 구현 활용 |

### 검색 재개

LazyVim의 `<Space>sR`과 개인 설정의 `<Space>sr`은 모두 `Snacks.picker.resume()`을 호출한다. 개인 `sr` 매핑을 삭제하면 기본 `sR`로 같은 picker를 재개할 수 있으며, LazyVim이 `sr`에 제공하는 검색 후 치환 기능과의 중복도 해소된다.

검색 결과 갱신은 키 매핑과 별개다. 저장이나 외부 파일 변경 후 grep 결과를 갱신하는 hook을 유지하면, 검색 재개 키를 바꾸기 위해 해당 기능까지 제거할 필요는 없다.

관련 개인 설정은 [snacks.lua](../modules/vim/files/lua/plugins/snacks.lua)에 있다.

### LSP와 Git의 연관 설정

현재 Trouble은 LazyVim의 기본 `<Space>cS`를 사용하며, `<Space>cl`은 기본 LSP 정보 화면을 연다. [ui.lua](../modules/vim/files/lua/plugins/ui.lua)와 [lsp.lua](../modules/vim/files/lua/plugins/lsp.lua)는 이 키들을 다시 정의하거나 비활성화하지 않는다.

현재 [git.lua](../modules/vim/files/lua/plugins/git.lua)는 개인 Git 표시 기호만 설정하고 `on_attach`는 LazyVim 기본 구현을 사용한다. 기본 구현은 diff 화면을 고려한 변경 부분 이동, 선택 영역 stage/reset, 버퍼 전체 stage, 첫 번째와 마지막 변경 부분 이동을 제공한다. `<Space>gh`는 Git 변경 부분 조작 키의 접두사이며, `<Space>ghb`로 blame을 표시한다. Preview 방식 등 세부 동작도 기본 구현을 따른다.

## 기본 구현을 활용할 항목

| 항목 | 비교 시점의 상태 | 권장 정리 방식 |
| --- | --- | --- |
| 진단, inlay hint, 줄 번호, 부드러운 스크롤, 줄바꿈 토글 | 개인 설정이 기본 토글을 다시 등록 | 기본값과 같은 등록 제거 |
| lualine 상태줄 | 개인 설정이 전체 구성을 교체 | 기본 `opts`에 Minuet 상태 등 필요한 항목만 추가 |
| 수동 포맷 | 개인 `<Space>cf`가 Conform을 직접 호출 | 기본 `<Space>cf`의 LazyVim 공통 포맷 경로 사용 |

현재 토글과 수동 포맷 키는 LazyVim이 등록한다. [ui.lua](../modules/vim/files/lua/plugins/ui.lua)는 기본 상태줄의 `opts`에 Minuet 상태만 추가하고, [editing.lua](../modules/vim/files/lua/plugins/editing.lua)는 선택한 포맷터와 실행 옵션을 설정한다. [autocmds.lua](../modules/vim/files/lua/config/autocmds.lua)의 Markdown 및 YAML 자동 포맷 제외 정책을 유지하며, 이 파일 형식에서도 기본 `<Space>cf`로 수동 포맷을 실행할 수 있다.

## 유지할 개인 설정

다음 항목은 환경 연동이나 의도적으로 선택한 동작을 담당하므로, 중복 키 매핑과 구분해서 유지한다.

- Nix에서 제공하는 LSP와 도구 사용, Mason 비활성화, agenix API 키 연동.
- 저장소의 공통 테마 export 연동과 테마 fallback.
- Blink 메뉴로 Minuet AI 제안을 통합하고 `<Tab>`으로 완성하는 설정. 인라인 AI 제안 비활성화, 요청 정책, 일반 완성을 우선하는 점수 설정도 포함한다.
- 선택한 포맷터와 Markdown 및 YAML의 저장 시 자동 포맷 제외 정책.
- 검색 전 현재 파일 저장, 외부 파일 변경 확인, 열린 grep 결과와 preview 갱신.
- 파일 인자 없이 실행했을 때의 세션 복원과 explorer 열기.
- Rust 파일 감시 보완과 Neovim nightly 호환성을 위한 Flash 버전 고정. 제거하려면 해당 문제가 해소됐는지 별도로 확인한다.

## 동작 차이를 판단한 뒤 변경할 항목

현재 설정은 LazyVim 기본값에 따라 LSP, `.git` 또는 `lua` 표식, 작업 디렉터리 순서로 프로젝트 루트를 찾는다. 기본 파일 검색, grep, explorer는 프로젝트 루트를 사용하므로 하위 디렉터리에서 실행해도 검색 범위가 프로젝트 전체로 넓어질 수 있다. `<Space>fF`와 `<Space>sG`는 작업 디렉터리를 기준으로 검색하며, `<Space>E`는 작업 디렉터리의 explorer를 연다. `:LazyRoot`로 감지된 루트를 확인할 수 있다.

편집 옵션도 LazyVim 기본값을 따른다. `autowrite`가 켜져 버퍼 전환 등 해당 옵션을 사용하는 명령에서 수정 내용이 자동 저장된다. 상대 줄 번호, 공백 문자 표시, 현재 줄 강조, 부드러운 스크롤, 창 전체 상태줄을 사용한다. 코드의 줄바꿈은 끄고 Markdown과 MDX 등 텍스트 파일에서는 줄바꿈과 맞춤법 검사를 켠다. 접기는 들여쓰기 또는 사용할 수 있는 Treesitter/LSP 기능을 따르며 처음에는 펼친 상태다. Markdown과 YAML의 저장 시 자동 포맷 제외 정책은 유지한다.

## 권장 적용 순서와 확인 기준

1. 개인 키 매핑과 기본값이 같은 토글 등록을 정리한다. `sR`의 검색 재개, `sr`의 검색 후 치환, `s/`의 검색 기록이 각각 동작하는지 확인한다.
2. Git의 기본 `on_attach`, 기본 상태줄 확장 방식, 기본 수동 포맷 경로를 적용한다. Git의 일반 화면 및 diff 화면 이동, Minuet 상태 표시, 자동 포맷을 제외한 파일의 수동 포맷을 확인한다.
3. 유지할 개인 설정의 회귀 여부를 확인한다. Blink 메뉴의 Tab 완성, 검색 결과 갱신, 세션 복원, 테마와 LSP 연동을 점검한다.
4. 프로젝트 루트와 편집 옵션은 LazyVim 기본값을 사용한다. 루트 기준 검색과 작업 디렉터리 기준 검색의 범위, 코드와 Markdown의 줄바꿈 차이를 확인한다.

## Mason과 Nix의 도구 관리 범위

2026-10-27에 확인한 공식 소스와 저장소 구성으로 판단하면, 현재 환경에서는 공통 실행 파일을 Nix로 유지하는 편을 권장한다. Mason은 Neovim에서만 사용하는 도구를 빠르게 추가하고 LazyVim의 자동 설치 흐름을 따르려는 경우에 유용하다. 따라서 앞의 Mason 비활성화 유지 권장은 이 저장소의 공유 도구와 여러 호스트 구성을 고려한 선택이며, Mason으로 도구를 관리할 수 없다는 뜻은 아니다.

`tree-sitter` CLI 누락은 Mason으로도 해결할 수 있다. [LazyVim v16.0.1의 설치 코드](https://github.com/LazyVim/LazyVim/blob/999700997f72227187d49d8b92667183dc7fc809/lua/lazyvim/util/treesitter.lua#L111)는 PATH에 CLI가 없으면 Mason의 `tree-sitter-cli` 패키지 설치를 시도한다. [공식 registry 정의](https://github.com/mason-org/mason-registry/blob/main/packages/tree-sitter-cli/package.yaml)는 조회 시점에 `v0.27.1`의 플랫폼별 GitHub release 실행 파일을 내려받도록 되어 있다. 현재 [Neovim Nix 모듈](../modules/vim/default.nix)에도 `pkgs.tree-sitter`가 선언되어 있으므로, CLI 누락만으로 전체 도구 관리 방식을 바꿀 필요는 없다.

| 판단 기준 | Mason | 현재 Nix 구성 |
| --- | --- | --- |
| LazyVim 연동 | CLI 누락 시 자동 설치 등 기본 흐름 활용 | 필요한 실행 파일을 직접 선언하고 Home Manager로 적용 |
| 다른 프로그램과 공유 | 기본 설치 경로와 PATH 추가가 Neovim 세션 기준 | [LSP 모듈](../modules/lsp/default.nix)의 실행 파일을 Claude Code 등에서도 사용 |
| 버전 복원 | 패키지별 버전 지정 가능. 별도 관리 필요 | 저장소의 flake 입력 고정과 패키지 선언으로 관리 |
| 현재 Nix LSP | 공식 registry에는 `nil`이 있지만 `nixd`는 조회 시점에 없음 | 현재 사용하는 `nixd` 제공 |

Mason의 [공식 설명](https://github.com/mason-org/mason.nvim#introduction)에 따르면 실행 파일은 기본적으로 Neovim data 디렉터리에 설치되고 Neovim의 PATH 앞에 추가된다. 공통 LSP 패키지를 Nix에서 제거하면 [Claude Code의 nixd 플러그인](../modules/claude/marketplace/lsp/nixd-lsp/README.md)처럼 PATH에서 실행 파일을 찾는 소비자도 별도로 처리해야 한다. [공식 package 목록](https://mason-registry.dev/registry/list)에 없는 `nixd`까지 Mason으로 옮기려면 추가 registry 등을 준비해야 한다.

Mason은 [공식 명령 문서](https://github.com/mason-org/mason.nvim/blob/main/doc/mason.txt)에 명시된 `:MasonInstall package@version`으로 특정 버전을 설치할 수 있다. 다만 [lazy.nvim의 lockfile](https://lazy.folke.io/usage/lockfile)은 플러그인 revision을 복원하므로 Mason이 설치한 외부 실행 파일의 버전까지 고정하지는 않는다.

NixOS에서는 일부 사전 빌드 Linux 실행 파일이 기대하는 동적 로더와 공유 라이브러리가 문제가 될 수 있다. 이 저장소는 [NixOS 공통 설정](../modules/nixos/baseline.nix)에서 [실행 파일 호환성을 위한 nix-ld](https://github.com/nix-community/nix-ld#how-does-nix-ld-work)를 이미 활성화한다. 그래도 Mason 전환 시 각 실행 파일의 라이브러리 요구 조건은 확인해야 하며, macOS와 일반 Linux 위의 Home Manager 환경에는 NixOS의 로더 제약을 그대로 적용하지 않는다.

전체 전환보다 작은 대안은 공통 LSP와 CLI를 Nix로 유지하고, 필요할 때 Neovim 전용 DAP adapter 등에만 Mason을 사용하는 것이다. 이 경우 동일 실행 파일의 중복 설치와 Mason의 PATH 우선순위를 확인해야 한다. 이 비교를 위해 설정을 변경하거나 도구를 설치하지 않았다.

## 근거

기본값은 비교 버전의 소스를 우선한다. 공식 사이트는 이후 버전에 맞춰 바뀔 수 있다.

- [LazyVim v16.0.1 Snacks picker 키 매핑](https://github.com/LazyVim/LazyVim/blob/999700997f72227187d49d8b92667183dc7fc809/lua/lazyvim/plugins/extras/editor/snacks_picker.lua).
- [LazyVim v16.0.1 편집기 플러그인 설정](https://github.com/LazyVim/LazyVim/blob/999700997f72227187d49d8b92667183dc7fc809/lua/lazyvim/plugins/editor.lua).
- [LazyVim v16.0.1 공통 키 매핑](https://github.com/LazyVim/LazyVim/blob/999700997f72227187d49d8b92667183dc7fc809/lua/lazyvim/config/keymaps.lua).
- [LazyVim 기본 키 안내](https://www.lazyvim.org/keymaps).
- [LazyVim 플러그인 설정 확장 방식](https://www.lazyvim.org/configuration/plugins).
