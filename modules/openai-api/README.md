# OpenAI API access

Codex와 Claude는 agent-core의 [openai-decisions skill](../../agent-core/skills/openai-decisions/SKILL.md)을 사용한다. Skill은 [공식 Decisions API](https://developers.openai.com/api/docs/guides/decisions)의 `gpt-6-luna`로 predicate, choice, score를 구현하는 방법을 안내한다. SDK는 사용하는 프로젝트에서 관리한다.

`modules.openaiApi.enable`은 Neovim, Codex 또는 Claude가 활성화된 환경에서 기본으로 켜진다. Standalone Home Manager에서는 이 모듈이 기존 `secrets/openai-api-key.age`를 agenix에 선언하며 Neovim도 같은 키를 사용한다. NixOS에서는 시스템 agenix의 사용자 소유 키를 사용한다. 파일 권한은 `0400`이며, macOS 경로와 재시도 동작은 [agenix Darwin 모듈](../agenix-darwin/README.md)이 관리한다.

`with-openai COMMAND [ARG...]`는 복호화된 파일을 읽고 실행하는 프로세스와 자식에게만 `OPENAI_API_KEY`를 전달한다. 기존 환경 변수보다 agenix 값을 우선하며, 키를 읽을 수 없거나 비어 있으면 명령을 실행하지 않는다. 키 값은 Nix store, skill, agent 설정 또는 전역 shell 환경에 저장하지 않는다.

```bash
with-openai uv run app.py
with-openai node app.mjs
```

환경에 맞는 `home-manager switch --flake .#<environment>` 또는 `nixos-rebuild switch --flake .#<host>`를 사용자가 실행한 뒤 새 agent 세션에서 Codex의 `$openai-decisions` 또는 Claude의 `/openai-decisions`로 skill을 호출한다. [Manifest](../../agent-core/manifest.toml)는 이 skill을 두 runtime에만 노출한다.

Home Manager는 `~/.ssh/id_ed25519`를 identity로 사용한다. 별도 recipient가 없는 `pylv-denim`에서는 관리자 identity가 필요하다. Recipient 변경은 [저장소 secret 절차](../../README.md#secrets-with-agenix-and-1password)를 따른다.

키 전달, 오류 처리, 인자와 종료 상태 보존, Nix의 `XDG_RUNTIME_DIR` 경로 확장은 실제 키나 네트워크 없이 검사한다. 경로 테스트는 Nix와 로컬에 캐시된 flake input을 사용한다.

```bash
uv run --no-project modules/openai-api/test_with_openai.py
uv run --project agent-core -m pytest agent-core/tests
uv run --project agent-core agent-core check
```
