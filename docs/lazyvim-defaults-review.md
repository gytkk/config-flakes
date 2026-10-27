# LazyVim 기본 설정 활용 검토

기존 조작을 유지하기 위해 추가한 개인 설정 중에서 키 매핑과 중복 설정부터 LazyVim 기본값으로 정리하는 것을 권장한다. Git, 상태줄, 수동 포맷은 기본 구현을 활용하고, Nix 연동과 의도적으로 선택한 편집 동작은 개인 설정으로 유지한다.

이 문서는 2026-10-27 기준 비교와 권장안이며, 아래 변경을 적용했다는 뜻은 아니다. 비교 기준은 LazyVim 전환 커밋 `ca8d121`의 개인 설정과 LazyVim `v16.0.1` 커밋 `999700997f72227187d49d8b92667183dc7fc809`이다. 실제 설정 구조와 사용법은 [Neovim README](../modules/vim/README.md)를 따른다.

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

Trouble을 `<Space>cS`로 정리할 때는 개인 Trouble 매핑과 기본 `<Space>cl`을 비활성화하는 LSP 설정을 함께 제거해야 한다. 관련 설정은 [ui.lua](../modules/vim/files/lua/plugins/ui.lua)와 [lsp.lua](../modules/vim/files/lua/plugins/lsp.lua)에 있다.

개인 [git.lua](../modules/vim/files/lua/plugins/git.lua)의 `on_attach`는 LazyVim의 기본 키 등록 함수를 통째로 대체한다. 기본 구현에는 diff 화면을 고려한 변경 부분 이동, 선택 영역 stage/reset, 버퍼 전체 stage, 첫 번째와 마지막 변경 부분 이동 등이 있으므로 기본값으로 돌아갈 실익이 크다. 이때 `<Space>gh`에 지정한 개인 blame 매핑도 함께 정리해 기본 Git 변경 부분 조작 키의 접두사로 사용할 수 있게 한다. 기본 키를 채택하면 preview 방식 등 세부 동작도 기본 구현을 따른다.

## 기본 구현을 활용할 항목

| 항목 | 비교 시점의 상태 | 권장 정리 방식 |
| --- | --- | --- |
| 진단, inlay hint, 줄 번호, 부드러운 스크롤, 줄바꿈 토글 | 개인 설정이 기본 토글을 다시 등록 | 기본값과 같은 등록 제거 |
| lualine 상태줄 | 개인 설정이 전체 구성을 교체 | 기본 `opts`에 Minuet 상태 등 필요한 항목만 추가 |
| 수동 포맷 | 개인 `<Space>cf`가 Conform을 직접 호출 | 기본 `<Space>cf`의 LazyVim 공통 포맷 경로 사용 |

토글은 [keymaps.lua](../modules/vim/files/lua/config/keymaps.lua), 상태줄은 [ui.lua](../modules/vim/files/lua/plugins/ui.lua), 포맷은 [editing.lua](../modules/vim/files/lua/plugins/editing.lua)에서 확인한다. 상태줄 전체를 반환해 대체하는 대신 기존 `opts`를 수정하면 LazyVim의 기본 구성 변경을 받아들이기 쉽다. 수동 포맷 키를 정리하더라도 포맷터 선택과 파일 형식별 자동 포맷 제외 정책은 유지한다.

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

현재 개인 설정은 작업 디렉터리를 프로젝트 루트 기준으로 사용한다. LazyVim 기본값은 LSP와 `.git`, `lua` 등의 표식을 통해 루트를 찾으며, 기본 파일 검색과 grep도 프로젝트 루트를 사용한다. 기본값을 채택하면 검색 대상 파일 범위가 달라질 수 있다. 기본 `<Space>fF`와 `<Space>sG`는 작업 디렉터리를 기준으로 검색하므로 두 범위를 구분해서 사용할 수 있다.

자동 저장, 상대 줄 번호, 줄바꿈, 접기 같은 편집 옵션도 개인 취향과 실제 동작에 영향을 준다. 기본값과 동일한 중복 설정은 제거할 수 있지만, 값이 다른 설정까지 일괄 삭제하지 않는다.

## 권장 적용 순서와 확인 기준

1. 개인 키 매핑과 기본값이 같은 토글 등록을 정리한다. `sR`의 검색 재개, `sr`의 검색 후 치환, `s/`의 검색 기록이 각각 동작하는지 확인한다.
2. Git의 기본 `on_attach`, 기본 상태줄 확장 방식, 기본 수동 포맷 경로를 적용한다. Git의 일반 화면 및 diff 화면 이동, Minuet 상태 표시, 자동 포맷을 제외한 파일의 수동 포맷을 확인한다.
3. 유지할 개인 설정의 회귀 여부를 확인한다. Blink 메뉴의 Tab 완성, 검색 결과 갱신, 세션 복원, 테마와 LSP 연동을 점검한다.
4. 프로젝트 루트와 편집 옵션은 원하는 동작을 결정한 뒤 별도로 변경한다. 루트 기준 검색과 작업 디렉터리 기준 검색의 범위를 확인한다.

## 근거

기본값은 비교 버전의 소스를 우선한다. 공식 사이트는 이후 버전에 맞춰 바뀔 수 있다.

- [LazyVim v16.0.1 Snacks picker 키 매핑](https://github.com/LazyVim/LazyVim/blob/999700997f72227187d49d8b92667183dc7fc809/lua/lazyvim/plugins/extras/editor/snacks_picker.lua).
- [LazyVim v16.0.1 편집기 플러그인 설정](https://github.com/LazyVim/LazyVim/blob/999700997f72227187d49d8b92667183dc7fc809/lua/lazyvim/plugins/editor.lua).
- [LazyVim v16.0.1 공통 키 매핑](https://github.com/LazyVim/LazyVim/blob/999700997f72227187d49d8b92667183dc7fc809/lua/lazyvim/config/keymaps.lua).
- [LazyVim 기본 키 안내](https://www.lazyvim.org/keymaps).
- [LazyVim 플러그인 설정 확장 방식](https://www.lazyvim.org/configuration/plugins).
