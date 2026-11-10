# Ghostty

`default.nix`는 `files/config`의 테마 토큰을 `modules.commonTheme` 값으로 바꾸고 Home Manager를 통해 `~/.config/ghostty/config`에 설치한다. 테마는 `themes/exports/ghostty`에서 가져온다.

`one-half-light`의 생성 테마는 `faint-opacity = 0.85`를 사용해 Claude Code 등의 흐린 글씨를 더 진하게 표시한다. 이 값은 [Ghostty override](../../themes/overrides/ghostty/one-half-light.yaml)에서 관리하며, 다른 테마는 Ghostty 기본값인 `0.5`를 사용한다. ANSI 팔레트는 바꾸지 않으므로 같은 색상을 배경으로 쓰는 앱의 색상도 유지된다.

새 창은 `maximize = true`에 따라 해당 모니터의 사용 가능한 영역을 채운다. macOS의 별도 전체 화면 Space로 전환하지 않으며 메뉴 막대와 Dock의 공간은 시스템 설정에 따른다. macOS에서 `window-save-state = always`는 기존 창, 탭, 분할과 창 크기를 복원하므로 복원된 창은 이전 크기를 유지할 수 있다.

맥북에서는 다음 명령으로 설정을 설치한 뒤 Ghostty에서 `Cmd+Shift+,`를 눌러 다시 읽는다.

```bash
home-manager switch --flake .#devsisters-macbook
```

최대화 설정은 새 창에 적용된다. 기존 창이나 복원된 창은 원하는 모니터로 옮긴 뒤 `Window > Return To Default Size`를 실행하면 현재 모니터의 사용 가능한 영역에 맞춰진다.
