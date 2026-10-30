# VSCode Module

Home Manager를 통한 Visual Studio Code 설정 및 확장 관리 모듈.

## 구조

```text
modules/vscode/
├── default.nix              # 메인 모듈 설정
├── one-half-light-theme/    # 로컬 커스텀 테마 확장
│   ├── package.json
│   └── themes/
└── README.md
```

## 동작 방식

### 플랫폼별 분기

| 플랫폼 | 설치 방식 |
|--------|----------|
| **macOS / Linux (non-WSL)** | `programs.vscode`로 전체 설치 (패키지 + 확장 + 설정) |
| **WSL** | 확장과 설정만 심볼릭 링크 (VSCode는 Windows에서 실행) |

### macOS/Linux 설정 경로

- **설정**: `~/Library/Application Support/Code/User/settings.json` → Nix store (symlink)
- **확장**: `~/.vscode/extensions/<ext-id>` → Nix store (symlink)
- **확장 목록**: `~/.vscode/extensions/.extensions-immutable.json` (Nix 관리 확장 추적)

### WSL 설정 경로

- **설정**: `~/.vscode-server/data/Machine/settings.json` → Nix store (symlink)
- **확장**: `~/.vscode-server/extensions/<ext-id>` → Nix store (symlink)
- **확장 목록**: `~/.vscode-server/extensions/extensions.json` (activation script으로 병합)

## 확장 관리

확장은 두 소스에서 가져옴:

- **nixpkgs**: 캐시되어 빠름 (`pkgs.vscode-extensions.*`)
- **nix-vscode-extensions**: nixpkgs에 없거나 최신 버전이 필요한 확장 (Marketplace에서 빌드)

`mutableExtensionsDir = true` 설정으로, Nix 관리 확장 외에 VSCode에서
수동으로 추가 확장을 설치할 수도 있음.

Scala는 구문 강조 확장만 설치하며 언어 서버는 설치하지 않는다.

### 로컬 커스텀 테마

`one-half-light-theme/` 디렉터리에 로컬 VSCode 테마 확장이 포함되어 있음.
`pkgs.stdenv.mkDerivation`으로 빌드되어 다른 확장과 동일하게 관리됨.

## 트러블슈팅

### `home-manager switch` 후 확장이 인식되지 않을 때

`home-manager switch` 실행 시 Home Manager는 `.extensions-immutable.json`의
`onChange` 핸들러를 통해 `extensions.json`을 재생성함.
그러나 **VSCode가 실행 중인 상태에서 switch를 하면**, VSCode가 먼저
자체적으로 `extensions.json`을 재생성하여 Nix 관리 확장이 누락될 수 있음.

**증상:**

- 테마가 적용되지 않음 (기본 테마로 표시)
- Nix로 설치한 확장이 보이지 않음
- `code --list-extensions` 출력이 비정상적으로 적음

**해결 방법:**

```bash
# extensions.json 삭제 후 VSCode에 재스캔 요청
rm ~/.vscode/extensions/extensions.json
code --list-extensions

# 이후 VSCode 재시작 또는 Reload Window (Cmd+Shift+P)
```

### 설정이 read-only라는 경고

`settings.json`은 Nix store로 심볼릭 링크되어 읽기 전용임.
VSCode UI에서 설정을 변경하려 하면 쓰기 실패가 발생할 수 있음.
설정 변경은 `config/modules/vscode/default.nix`의 `userSettings`를 수정한 뒤
`home-manager switch`로 적용해야 함.
