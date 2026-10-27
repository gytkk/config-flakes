# Vim Module

This module configures Neovim as the primary editor with a modern Lua-based setup managed through Home Manager.

## What it does

- Installs the pinned Neovim 0.13 development build and configures it as the default editor
- Creates `vi` and `vim` aliases that point to `nvim`
- Loads the Neovim configuration from modular Lua files under `modules/vim/files/config/`
- Exposes every generated colorscheme from `themes/exports/nvim/`
- Loads the selected shared theme and falls back to `one-half-light` if its generated export is missing

Vira Graphene follows the original editor's syntax and UI colors through a theme override, with comments slightly brightened to `#676767` for readability. Its selection uses the brighter gray `#545454` to stay visible against the editor background. Neovim cannot alpha-blend a Visual highlight background. See the [canonical theme resolution rules](../../themes/README.md#resolution-rules) for the source and Zed selection behavior.

## Included Features

- `lazy.nvim` plugin management
- `snacks.nvim` for file picking, explorer, notifications, status column, inlay hint toggles, and lazygit integration
- `snacks.nvim` extras for git browse, file rename, smooth scrolling, and inline image rendering
- `snacks.nvim` GitHub integration for issues and pull requests via the `gh` CLI
- `persistence.nvim` for directory and branch-aware session saving and restore on bare `nvim` startup
- `blink.cmp` completion with LSP, snippets, path, buffer, and on-demand Minuet AI suggestions
- `minuet-ai.nvim` inline suggestions backed by OpenAI `gpt-6-luna`
- Built-in Neovim LSP configuration for Nix, Go, Rust, TypeScript, HTML, CSS, JSON, Lua, shell, Terraform, YAML, Markdown and MDX, TOML, and Python via `ty`
- Neovim watches Rust files for rust-analyzer to avoid a macOS server watcher startup stall
- `nvim-treesitter` syntax parsing for the main languages used in this repository
- `conform.nvim` formatting on save, including `nixfmt`, `prettier`, `rustfmt`, `gofmt`, and `ruff_format`
- `gitsigns.nvim`, `lualine.nvim`, `flash.nvim`, `which-key.nvim`, `trouble.nvim`, and `render-markdown.nvim`
- Native diagnostics show a color-coded circle and the current-line message directly after the code
- Markdown and MDX rendering uses `render-markdown.nvim` for an in-buffer preview that returns to raw Markdown or MDX source in insert mode

## Flash compatibility

`flash.nvim` is pinned to the upstream Neovim 0.13 search-state fix. Older versions access removed `search_match_*` globals and can repeatedly report `Flash error during redraw` after a character motion, including when saving MDX files. Run `:Lazy update flash.nvim` after applying this configuration and restart Neovim.

## Search refresh

For `/` and `?` searches, entering insert mode, changing buffer text, leaving a buffer, or reloading an externally changed file clears search highlights and Noice's inline search count. Press `<Esc>` in normal mode to clear them manually. The search pattern remains available, and `n` or `N` resumes the search with highlights and the count.

Open grep pickers rerun the current search after a file is saved, after Neovim reloads an externally changed file, and when Neovim regains focus. The results and preview refresh while preserving the search query and scope. This also applies to a grep picker reopened with `<leader>sr`. External edits do not trigger continuous filesystem polling; they appear when Neovim detects the change or regains focus.

## Minuet

- Store the OpenAI API key in `secrets/openai-api-key.age` before launching Neovim.
- NixOS hosts decrypt the secret with system agenix under `/run/agenix/openai-api-key`; home-only environments declare it through the [OpenAI API module](../openai-api/README.md) and decrypt it with Home Manager agenix.
- The vim module wires the decrypted agenix path into Neovim as `vim.g.openai_api_key_path` on every environment.
- Inline suggestions use Minuet's virtual text frontend to avoid duplicate OpenAI API requests from the completion menu.
- Minuet requests OpenAI Fast mode for lower latency at the Fast mode token rate.
- Minuet uses a 6,000-character context window, a 900ms request throttle, and a 4-second streamed-response timeout. The on-demand Blink source uses the same 4-second timeout. This keeps AI suggestions responsive without requesting a completion on every keystroke.
- Press `<Tab>` to accept the current Minuet inline suggestion when one is visible.
- Press `<C-y>` to open Minuet suggestions in the `blink.cmp` menu on demand.

Create or update the secret with:

```bash
EDITOR=vim agx -e openai-api-key.age
```

## Notable Keymaps

- `<leader>e`: file explorer
- `<leader>ff`: find files
- `<leader>sg`: grep (saves the current modified file and reloads external changes first)
- `<leader>sr`: resume the previous picker with its search query and scope
- `<leader>gs`: git status picker
- `<leader>gb`: git branches picker
- `<leader>gd`: git diff picker
- `<leader>gf`: git log for current file
- `<leader>gS`: git stash picker
- `<leader>gl`: git log picker
- `<leader>gL`: git log for current line
- `<leader>gg`: lazygit
- `<leader>gB`: open current file or selection in the git remote browser
- `<leader>gh`: blame current line
- `<leader>gi`: open GitHub issues
- `<leader>gp`: open GitHub pull requests
- `<leader>qs`: restore the current directory session
- `<leader>qS`: select a saved session
- `<leader>ql`: restore the last saved session
- `<leader>qd`: stop saving the current session on exit
- `gd`: go to definition
- `gb`: jump back in the jumplist
- `<leader>cR`: rename current file with LSP-aware updates
- `<leader>ud`: toggle diagnostics
- `[d` / `]d`: jump to the previous or next diagnostic
- `<leader>d`: toggle all diagnostics on the current line
- `<leader>xX`: show current-buffer diagnostics in Trouble
- `<leader>xx`: show workspace diagnostics in Trouble
- `<leader>uh`: toggle LSP inlay hints
- `<leader>ul`: toggle line numbers
- `<leader>uS`: toggle smooth scrolling
- `<leader>uw`: toggle wrap
- `<leader>cf`: format current buffer
- `<Tab>`: accept current Minuet inline suggestion, otherwise fall back to the usual blink/snippet behavior
- `<C-g>l`: accept current Minuet suggestion line
- `<C-g>e`: dismiss current Minuet suggestion
- `<C-g>n`: show next Minuet inline suggestion
- `<C-g>p`: show previous Minuet inline suggestion
- `<C-y>`: request Minuet completion in the blink menu

## Usage

Open a project with the saved session and the left file explorer:

```bash
nvim
```

Opening a specific file skips startup session restore and explorer opening:

```bash
nvim filename.txt
vim filename.txt
vi filename.txt
```

Markdown files opened directly receive both in-buffer rendering and Marksman support, including standalone files outside a Git repository or a directory with `.marksman.toml`. Filetype and syntax activation runs after plugin loading handlers are registered so plugins can handle the initial file's events.

## Files

- `modules/vim/default.nix`: Home Manager wiring
- `modules/vim/files/config/init.lua`: small bootstrap for options, lazy.nvim, plugin specs, and startup hooks
- `modules/vim/files/config/*.lua`: focused modules for options, lazy.nvim bootstrap, theme loading, LSP, session restore, AI helpers, and autocmds
- `modules/vim/files/config/plugins/*.lua`: plugin specs grouped by feature area
- `themes/exports/nvim/*.lua`: generated colorscheme definitions

## LazyVim 도입 검토

2026-10-27에 검토한 내용을 향후 도입 판단의 배경으로 기록한다. 저장소 기준은 `0080d76222accbafbfaf2bc08e1c15241d40ae8e`이며, LazyVim 비교는 아래 공식 문서의 당시 기본 설정을 기준으로 한다. LazyVim 도입과 기존 동작 변경은 아직 결정하지 않았다. 도입 시에는 선택한 LazyVim 버전에서 기본값과 호환성을 다시 확인한다.

### 검토 배경과 판단

Neovim은 편집기이고, `lazy.nvim`은 플러그인 관리자이며, LazyVim은 `lazy.nvim`을 사용하는 Neovim 설정 배포판이다. 이 모듈은 LazyVim 없이 플러그인과 Lua 설정을 직접 조합한다. 확인한 로컬 설치는 Neovim `0.13.0-nightly+c049b8f`였으며, `~/.config/nvim/lua/config`는 이 저장소의 설정 디렉터리를 참조했다.

현재 구성에는 LazyVim에서도 사용하는 Snacks, Blink, Flash, which-key, Gitsigns, Trouble, Lualine, Noice, Conform, Persistence 등이 이미 포함되어 있다. 따라서 전환으로 얻을 핵심 이점은 새로운 편집 기능보다 공통 설정을 직접 관리하는 부담을 줄이는 데 있다. 개인 설정을 이식하면 사용감을 비슷하게 유지할 수 있지만, LazyVim 기본 설정으로 교체하는 것만으로 현재 동작이 보존되지는 않는다.

### 구성 비교

| 항목 | 검토 시점의 구성 | LazyVim 도입 시 차이 |
| --- | --- | --- |
| 플러그인 관리 | `lazy.nvim`으로 플러그인과 설정을 직접 조합 | 같은 관리자를 사용하며 LazyVim의 기본 spec에 개인 설정을 추가 |
| 탐색과 검색 | Snacks 사용. 검색 전 저장과 외부 변경 확인, 열린 검색 결과 갱신을 직접 구현 | 기본 탐색과 검색은 유사하지만 프로젝트 root와 현재 디렉터리를 구분한다. 기존 저장과 갱신 동작은 별도 이식 필요 |
| 자동 완성과 AI | Blink의 `super-tab` 기반 설정, Minuet 제안을 우선 수락하는 `<Tab>`, agenix 키 연결 | Blink 기본값은 `enter` preset이다. Minuet와 키 연결, 완성 키 동작은 별도 설정 필요 |
| 언어 서버 | Nix가 서버 패키지를 설치하고 Lua에서 활성화. Rust는 활성 rustup toolchain 사용 | Mason 연동과 언어별 Extras를 제공한다. 기존 설치 방식을 유지하려면 서버별 Mason 사용 여부와 활성화 목록 조정 필요 |
| 포맷 | Conform에서 도구를 지정하며 Markdown과 YAML은 저장 시 포맷 제외 | 같은 플러그인을 사용하지만 기존 도구 선택과 예외 규칙은 별도 설정 필요 |
| 세션과 시작 화면 | 인자 없는 실행에서 세션 복원과 탐색기 열기. 파일을 지정하면 해당 동작 생략 | Persistence와 대시보드를 제공한다. 현재의 자동 복원 조건과 창 처리 로직은 별도 이식 필요 |
| 테마와 UI | 공통 테마 export를 로드하며 확인한 로컬 선택값은 `one-half-light` | 기본 테마는 TokyoNight이며 Bufferline과 대시보드 등을 포함한다. 공통 테마 연결과 UI 선택 필요 |
| 단축키 | 이름 변경은 `<leader>rn`, 검색 재개는 `<leader>sr` | 각각 `<leader>cr`, `<leader>sR`이다. 유지할 개인 키와 수용할 기본 키를 구분해야 함 |
| 기능 확장과 유지보수 | 플러그인을 직접 선택하고 공통 설정과 개인 설정을 함께 관리 | `:LazyExtras`로 준비된 구성을 활성화할 수 있다. 공통 설정 변경은 LazyVim에서 받아오고 개인 설정과의 호환성을 관리 |

기본값 비교의 근거는 공식 [설정 구조](https://www.lazyvim.org/configuration), [기본 플러그인](https://www.lazyvim.org/plugins), [Blink 설정](https://www.lazyvim.org/extras/coding/blink), [LSP 설정](https://www.lazyvim.org/plugins/lsp), [세션 관리](https://www.lazyvim.org/plugins/util), [단축키](https://www.lazyvim.org/keymaps) 문서다. 현재 구성의 세부 동작은 이 README의 기능 설명과 아래 소스 파일을 기준으로 확인한다.

### 설정을 줄일 수 있는 범위

검토 시점에 `files/config/`의 Lua 파일은 15개, 빈 줄과 주석을 포함해 1,027줄이었다. 이미 기능별로 분리되어 있으므로 LazyVim 도입의 정리 효과는 파일을 합치는 것보다 기본 플러그인 선언과 초기화 코드를 줄이는 데 있다. 다음은 실제 전환 전의 예상이며, 줄 수 감소를 측정한 결과는 아니다.

| 현재 파일 | 정리 가능한 부분 | 계속 관리할 부분 |
| --- | --- | --- |
| [init.lua](files/config/init.lua), [plugins/init.lua](files/config/plugins/init.lua) | 기본 플러그인 목록의 수동 조합과 일부 초기화 연결 | LazyVim 초기화와 개인 spec 연결. 파일 열기 이벤트 순서는 회귀 확인 필요 |
| [plugins/ui.lua](files/config/plugins/ui.lua), [plugins/git.lua](files/config/plugins/git.lua) | 기본 spec과 중복되는 의존성, 로딩 조건, 키 설정 | 개인 상태 표시줄, 알림과 Git 표시, 유지할 키, Flash 버전 고정 |
| [lsp.lua](files/config/lsp.lua), [plugins/treesitter.lua](files/config/plugins/treesitter.lua) | 공통 capabilities와 서버 초기화, 기본 parser 설정 | Nix 설치 서버 선택, 진단 표시, Rust watcher 설정, 필요한 언어 목록 |
| [plugins/completion.lua](files/config/plugins/completion.lua), [plugins/editing.lua](files/config/plugins/editing.lua) | 기본 완성 및 포맷 설정과 중복되는 선언 | Minuet 연동, 완성 키, 포맷 도구와 예외, Markdown 외부 변경 처리 |
| [plugins/snacks.lua](files/config/plugins/snacks.lua), [session.lua](files/config/session.lua), [autocmds.lua](files/config/autocmds.lua) | 기본 탐색기, 세션 플러그인, 일부 이벤트와 키 선언 | 검색 갱신, 탐색기 창 처리, 자동 세션 복원, 외부 변경 후 화면 갱신 |
| [default.nix](default.nix), [ai.lua](files/config/ai.lua), [theme.lua](files/config/theme.lua) | 설정 디렉터리 연결 방식의 조정 | Home Manager 설치, agenix 키 읽기, 공통 테마 로딩과 fallback |

LazyVim은 개인 `opts`와 `keys`를 기본 spec에 병합하고 설정 파일을 정해진 시점에 로드한다. 이 방식을 사용하면 기본값과 다른 설정을 중심으로 파일을 유지할 수 있다. 기존 `config` callback과 초기화 코드를 그대로 복사하면 공통 설정을 대체하거나 중복 실행할 수 있으므로 각 파일을 역할에 맞게 이식해야 한다. [플러그인 병합 규칙](https://www.lazyvim.org/configuration/plugins), [설정 로딩 규칙](https://www.lazyvim.org/configuration/general)

현재 동작을 모두 유지하면 개인 로직이 상당 부분 남으므로 파일 수나 줄 수가 크게 줄어든다고 단정할 수 없다. LazyVim 기본 UI와 단축키를 수용할수록 설정을 더 줄일 수 있지만, 어느 동작을 바꿀지는 별도 결정이 필요하다.

### 도입 전에 결정하고 확인할 사항

- 전환 목적을 공통 설정의 유지보수 부담 감소로 둘지 판단한다. 현재 기능에 만족한다면 기능 추가만을 이유로 전환할 필요는 크지 않다는 것이 이번 검토의 판단이다.
- 기존 단축키, 완성 수락 방식, 시작 화면, 검색 범위 중 유지할 동작과 LazyVim 기본값을 따를 동작을 정한다. 현재 동작 변경에 대한 합의는 없다.
- Nix의 패키지 설치, agenix 비밀 키 처리, 공통 테마 export 연결은 유지하는 방향을 검토한다. Mason을 통한 중복 설치와 서버 중복 활성화 여부를 확인한다.
- 자동 세션 복원, 파일 지정 실행, Markdown과 MDX의 최초 로딩, 검색 갱신, 저장 시 포맷 예외, Minuet 완성과 진단 표시를 전환 전후에 비교한다.
- Neovim nightly에 필요한 Flash 수정과 Rust watcher 설정은 호환성을 확인하기 전까지 유지한다. LazyVim 도입 자체가 기존 문제를 해결한다고 가정하지 않는다.

이번 검토는 저장소 설정, 로컬 실행 파일 버전과 설정 경로, 공식 문서 확인에 한정했다. LazyVim 설치, 실제 전환, Nix 평가나 빌드, 시작 시간과 실행 성능 비교는 하지 않았다. 따라서 설정 감소량, 호환성, 성능 개선 여부는 확인하지 않은 상태다.
