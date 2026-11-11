# Agent core

## 목적

`agent-core/`는 coding-agent 공통 규칙과 runtime adapter, 로컬 skill, 외부 skill source 선언을 관리한다. Stdlib-only Python renderer는 지침 문서를 생성하고, `skill-plan`은 설치 대상과 source를 JSON으로 출력한다. 실제 skill 취득과 배치는 버전을 고정한 `skills@1.7.2` CLI가 담당한다.

Runtime module은 생성 지침과 extension을 설치한다. [Agent skills module](../modules/agent-skills/README.md)은 Home Manager switch마다 활성 agent 전체의 skill을 설치하거나 갱신한다. [설치 연결 명령](../scripts/install-agent-skills.py)은 manifest의 선택 목록을 pinned CLI에 전달하고 설치 경로별 소유권, 충돌, 백업과 복구를 관리한다. Nix build와 evaluation은 외부 skill을 다운로드하지 않으며, activation에는 network 접근이 필요하다.

## 구조

```text
agent-core/
├── pyproject.toml
├── uv.lock
├── manifest.toml
├── src/agent_core/       # Python API와 CLI
├── rules/                # Runtime에 독립적인 공통 규칙
├── adapters/             # Runtime별 전용 지침
├── skills/               # Shared skill의 canonical catalog
├── schema/               # Runtime output golden hash
├── nix/                  # Package・render helper와 flake check 조립
└── tests/
```

`manifest.toml`의 `skill_sources`는 로컬 skill과 외부 Git source를 선언한다. 외부 source는 설치 작업마다 기본 branch의 최신 full SHA를 한 번 결정하며, 같은 작업의 모든 target에 같은 SHA를 사용한다. `resolving-merge-conflicts`는 최신 upstream에서 삭제되어 사용자 결정에 따라 새 설치 목록에서 제외한다. 최신 upstream의 workflow와 UI metadata 변경도 첫 전환에 포함된다.

`skill_targets`는 Codex/Pi/OMP 공통 pool과 Claude copy에 설치할 skill을 선택한다. 세 runtime은 `~/.agents/skills`를 공유하므로 같은 shared 선택 목록을 발견한다. [OpenAI API 모듈](../modules/openai-api/README.md)은 API client에 agenix 키를 전달하며, skill 설치 자체가 package나 credential을 준비하지는 않는다.

Hunk의 `hunk-review`는 `modem-dev/hunk`의 `.agents/skills/hunk-review`에서 취득하여 shared와 Claude target에 설치한다. Skill은 `hunk skill show hunk-review`로 설치된 CLI 버전에 맞는 지침을 읽는다. Hunk 실행 파일은 공통 Home Manager 패키지가 설치하며, skill은 다른 외부 source와 동일하게 switch 때 갱신한다.

`runtimes.*.skills`와 기존 vendored tree는 이전 Nix generation의 복구와 최종 renderer 정리를 위해 남아 있다. 현재 module은 이 skill output을 설치하지 않는다. 모든 host의 Codex/Claude/Pi 전환과 실제 discovery 검증이 끝나기 전에는 이 fallback과 golden hash를 제거하거나 변경하지 않는다.

공통 [data-analysis](skills/data-analysis/SKILL.md)는 지표 정의, 집계 단위, 조인과 코호트 검증을 다루고, [data-visualization](skills/data-visualization/SKILL.md)은 데이터에서 차트를 만들고 렌더링 결과를 확인하는 절차를 다룬다. [writing-for-agents](skills/writing-for-agents/SKILL.md)는 skill 호출 조건과 로드 여부, 실행 결과를 구분해 검증한다. [korean](skills/korean/SKILL.md)은 직전 답변을 의미와 불확실성을 유지하면서 자연스러운 한국어로 정리한다. 선별 근거와 원본 버전은 [shared skill catalog](skills/README.md)에 기록한다.

[parallel-research-merge](skills/parallel-research-merge/SKILL.md)는 Codex, Claude, Pi, OMP에 같은 원본으로 설치한다. 독립적인 조사 결과가 판단이나 검증을 바꿀 수 있고 메인이 동시에 진행할 일이 있을 때 사용한다. Worker 하나부터 필요한 범위만 위임하고, 채택하거나 기각한 근거가 결과에 미친 영향을 기록한다. 분석 요청은 구현이나 커밋 없이 끝낸다. 호출 조건과 효과는 [평가 사례와 비교 절차](skills/parallel-research-merge/references/evaluation.md)로 별도 검증한다.

## 책임 경계

### Agent core

`agent-core/`는 다음을 소유한다.

- 공통 운영 규칙과 prose 규칙
- 공통 위임 계약과 runtime별 모델 라우팅 지침
- Runtime adapter 원문
- 로컬 skill 원문과 보조 파일, 외부 source 선언
- Runtime별 문서 조합 순서와 CLI target별 선택 목록
- Deterministic materialization과 generated marker
- Source validation, golden hash, tree comparison
- Python package, CLI, 단위 테스트, Nix package

`agent-core/`는 다음을 소유하지 않는다.

- Credentials, API key, provider 인증
- Runtime의 모델 설정과 사용 가능 여부, MCP 인증 상태, session, cache
- Runtime 홈 경로와 Home Manager activation 순서
- Claude marketplace plugin 원문
- Runtime CLI를 통한 설치 후 discovery 확인

### Runtime module

`modules/claude`, `modules/codex`, `modules/pi`, `modules/omp`는 다음을 소유한다.

- Runtime package 또는 user-managed runtime의 host integration
- Renderer의 지침 문서를 실제 runtime 경로에 설치하는 선언
- Runtime settings, plugin, extension, hook, MCP 설정
- Mutable state의 안전한 경계와 activation 순서
- Runtime이 설치 결과를 발견하는지 확인하는 smoke test

Runtime module은 공통 원문, shared skill 원문, 독자적인 render 또는 merge 알고리즘을 소유하지 않는다. Renderer는 manifest에 선언한 source만 선택한다.

공통 `modules/agent-skills`는 활성 agent에 맞춰 한 번의 설치 작업을 구성한다. Codex, Pi, OMP 중 하나라도 활성화되면 shared target을 한 번 선택하고, Claude가 활성화되면 Claude target을 선택한다. `linkGeneration`과 필요한 `codexUserConfig` 이후에 실행하며 Node/npm, Git, packaged canonical resource를 포함한 Python을 제공한다. Home Manager의 `run`으로 dry-run을 존중하고 설치 오류가 나면 switch를 실패시킨다.

## 의존성 방향

```text
agent-core source
        ↓ materialize
runtime별 immutable output tree
        ↓ install
modules/claude | modules/codex | modules/pi
        ↓
~/.claude | ~/.codex | ~/.pi
```

허용하는 의존성은 다음과 같다.

```text
modules/* -> agent-core package와 render interface
flake packages, apps, checks -> agent-core
```

다음 의존성은 만들지 않는다.

```text
agent-core source -> modules/* 경로
agent-core -> 개인 memory 또는 runtime state
agent-core -> 암묵적인 runtime 홈 경로
runtime A -> runtime B의 session 또는 credentials
```

## Python interface

Pure CLI는 다음 command를 제공한다.

```bash
agent-core render --runtime <name> --output <empty-dir>
agent-core check
agent-core verify-install --expected <dir> --actual <dir>
agent-core skill-plan --target shared
```

- `render`는 manifest와 canonical resource를 읽고 runtime별 shared skill allowlist에 맞는 output tree를 생성한다. Output 경로는 없거나 비어 있는 실제 디렉터리여야 한다.
- `check`는 manifest, rule, adapter, skill, generated marker, golden hash를 검증한다. 홈 디렉터리와 module 경로는 읽지 않는다.
- `verify-install`은 명시적으로 전달한 두 tree의 파일 내용, symlink 대상, 누락 경로, 추가 경로를 비교한다. Runtime 이름이나 기본 홈 경로를 추론하지 않는다.
- `skill-plan`은 `shared`, `claude` target의 source별 이름과 경로, 갱신 정책, CLI 버전을 JSON으로 출력한다. 네트워크나 홈 디렉터리를 읽지 않는다.

핵심 API는 경로별 bytes map을 반환한다.

```python
materialize(runtime: str) -> Mapping[PurePosixPath, bytes]
```

파일 쓰기와 CLI parsing은 이 API 밖에 둔다. Renderer는 manifest allowlist에 있는 shared skill만 선택한다. 같은 source, manifest, runtime은 byte 단위로 같은 결과를 만들어야 한다.

## Manifest

`manifest.toml`은 다음 정보만 선언한다.

- Manifest와 generated marker version
- 지원 runtime
- Runtime별 output 문서 경로와 source 순서
- Runtime별 shared skill allowlist
- Runtime별 skill output root
- Local/Git source와 GitHub URL, latest 정책, upstream skill 경로와 license 경로
- CLI 버전과 target별 설치 선택 목록

Credentials, 절대 홈 경로, host 값, module 경로는 manifest에 넣지 않는다. Unknown key, unknown runtime, unsafe path, source 사이의 중복 이름, 미선언 선택과 존재하지 않는 로컬 source는 오류다. 외부 파일의 존재와 bytes는 별도 network suite와 실제 CLI 설치 후에 검증한다.

Skill 검증은 각 디렉터리의 `SKILL.md` frontmatter에서 `name`과 `description`을 요구한다. `name`은 디렉터리 이름과 같아야 한다. Symlink와 output tree를 벗어나는 경로는 거부한다.

## Runtime output

모든 output root에는 `.agent-core-generated` marker가 있다. Skill 파일에도 generated marker를 넣기 때문에 생성 결과를 canonical source로 오인하지 않아야 한다.

### Claude Code

```text
CLAUDE.md
skills/
```

`CLAUDE.md`는 공통 operating rule, 공통 agent rule, prose rule, Claude adapter 순서로 생성한다. Home Manager는 문서를 설치한다. Skill은 CLI의 `--copy --global --agent claude-code`로 `~/.claude/skills`에 설치한다. `devils-advocate` command와 agent는 local marketplace plugin이 별도로 소유한다.

### Codex

```text
AGENTS.md
skills/
```

`AGENTS.md`는 공통 operating rule, 공통 agent rule, prose rule, 공통 위임 계약, Codex adapter 순서로 생성한다. Codex adapter는 native subagent의 명시적 모델 라우팅을 정의한다. Skill은 `--global --agent codex pi`로 `~/.agents/skills`에 설치한다. Nix는 `/etc/codex/config.toml`을 계속 관리하며 `/etc/codex/skills`를 관리하지 않는다.

### Pi

```text
AGENTS.md
APPEND_SYSTEM.md
skills/
```

Pi는 operating invariant를 `APPEND_SYSTEM.md`로 분리한다. `AGENTS.md`에는 공통 agent rule, prose rule, Pi adapter만 포함한다. Skill은 Codex와 같은 native shared pool을 사용하며 `~/.pi/agent/skills`에 별도 복사본을 설치하지 않는다.

### OMP

OMP는 shared skill 소비자로만 등록한다. OMP용 문서 renderer나 legacy skill output은 만들지 않는다. [OMP 모듈](../modules/omp/README.md)을 단독으로 활성화해도 공통 activation이 `~/.agents/skills`를 설치·갱신하며, OMP의 `agents` provider가 이를 `agents:user`로 발견한다.

`skill_targets.shared.runtimes`는 소비 runtime 목록이다. 설치 adapter의 `--agent codex pi`는 pinned Skills CLI에서 공통 설치 경로를 선택하는 인자이므로 그대로 유지한다. OMP 전용 복사본이나 symlink는 만들지 않는다.

## Nix 연결

`agent-core/nix/package.nix`는 wheel에 manifest와 canonical resource를 포함한 Python application을 만든다. `agent-core/nix/render.nix`는 runtime 이름을 받아 build derivation 안에서 renderer를 실행한다.

루트 `outputs.nix`는 설정·패키지·앱 출력을 조립하고, agent-core 전용 검증은 `agent-core/nix/checks.nix`에 위임한다. 이 Nix 통합 계층은 시스템별 package set, 완성된 Home Manager/NixOS 설정과 activation 테스트 디렉터리를 명시적으로 받아 render 비교, golden output, source wiring과 activation 검사를 구성한다. Runtime 경로와 activation 순서의 선언은 계속 각 module이 소유하며, Python source와 renderer에는 module 의존성을 추가하지 않는다.

Flake는 다음 output을 노출한다.

```text
packages.<system>.agent-core
apps.<system>.agent-core
checks.<system>.agent-core
checks.<system>.agent-core-renders
```

Runtime module은 `render.nix`가 만든 immutable tree에서 필요한 파일을 설치한다. `checks.<system>.agent-core-renders`는 세 runtime final output의 recursive golden hash와 Home Manager/NixOS source wiring을 검증한다. Nix evaluation과 renderer는 홈 디렉터리, checkout 밖의 mutable state, runtime state를 읽지 않는다.

## Skill 설치와 복구

OpenClaw는 공통 지침과 skill 설치 대상에 포함하지 않는다. 기존 integration과 session recorder의 제거 절차는 [OpenClaw migration](../modules/openclaw/README.md#removing-the-shared-agent-integration)을 따른다. 기존 설치 파일, ownership 기록과 backup은 자동 삭제하지 않는다. OpenClaw target을 포함한 이전 설치 영수증을 복구하려면 해당 영수증을 생성한 이전 installer revision을 사용한다.

정상 운영에서는 사용자가 Home Manager/NixOS switch를 실행하면 활성 agent 전체의 skill이 함께 설치되거나 갱신된다. 각 외부 source의 최신 full SHA는 한 작업에서 한 번 결정하여 모든 target에 공통으로 사용한다. 성공한 switch 뒤에 별도 설치 명령은 필요하지 않다. 직접 연결 명령을 실행하는 방식은 격리 probe, 복구와 명시적 SHA 재현에 사용한다. 이 명령에는 Node, npm, Git와 network 접근이 필요하다. Activation은 `XDG_STATE_HOME`을 Home Manager의 `xdg.stateHome`으로 설정하므로 직접 설치와 복구에도 같은 값을 사용한다. 아래 예시는 기본값이며 `xdg.stateHome`을 변경했다면 그 경로를 지정한다. 다음 `probe`는 임시 HOME에서 설치를 검증하고, 기존 host의 skill이나 credential을 변경하지 않는다. 영수증 파일은 새 경로여야 한다.

```bash
export XDG_STATE_HOME="${XDG_STATE_HOME:-$HOME/.local/state}"
uv run --project agent-core scripts/install-agent-skills.py probe \
  --target shared --target claude --output /tmp/agent-skills-probe.json
```

Switch 전에 이전 Nix generation과 `/etc/codex/skills`, `~/.claude/skills`, `~/.pi/agent/skills`의 대상과 파일을 백업한다. Inventory의 다섯 host 모두 Codex/Pi/Claude 대상이다.

최초 switch 전에 legacy 경로의 소유권을 확인하고 필요한 backup과 인계를 완료한다. Standalone `/etc/codex/skills`는 자동 삭제되지 않으므로 repo-managed 링크임을 확인하고 승인한 뒤 백업 위치로 옮긴다. Claude와 Pi의 기존 Home Manager symlink는 새 generation에서 제거되지만 다른 legacy 링크는 자동으로 삭제하지 않는다. 설치 경로 안의 동명 skill은 backup한 뒤 manifest 기준으로 교체한다. Switch 후에는 legacy 경로와 중복 discovery가 없는지 확인한다.

다음은 직접 계획을 확인하고 probe의 exact SHA로 설치를 재현하는 명령이다. 일반 switch는 replay 영수증을 사용하지 않고 최신 SHA를 새로 결정한다.

```bash
uv run --project agent-core scripts/install-agent-skills.py plan --target shared --target claude
uv run --project agent-core scripts/install-agent-skills.py install \
  --target shared --target claude --replay /tmp/agent-skills-probe.json --apply
```

`--replay` 없이 직접 설치해도 외부 latest를 새로 결정한다. `--revision matt=<full-sha> --revision vercel=<full-sha>`로 명시적 revision을 지정할 수도 있다. 설치는 먼저 격리된 경로에서 모든 source를 검증한 뒤 실제 target에 CLI를 실행하고 파일 bytes를 대조한다. 최신 source에 선택한 이름이나 license가 없으면 기존 설치를 변경하지 않는다.

소유권은 설치 경로와 이름별로 `$XDG_STATE_HOME/agent-core/skills` 또는 `~/.local/state/agent-core/skills`에 기록한다. 원본 SHA 또는 local content, 설치 파일의 hash와 mode, 추가한 `AGENT-CORE-SOURCE-LICENSE`를 구분해 기록한다. CLI의 이름별 global lock을 target별 소유권으로 간주하지 않는다. 설치 결과는 항상 manifest와 일치하도록 맞춘다. 설치본이 없거나 수정되었거나, 기록된 source가 manifest와 다르거나, 소유권 기록이 없는 동명 skill이 있으면 기존 디렉터리를 backup한 뒤 다시 설치한다. 선택 목록에서 빠진 기존 관리 항목은 backup한 뒤 삭제한다. 포괄적인 `skills update`는 사용하지 않는다.

`diagnosing-bugs`, `codebase-design`, `research`는 로컬 수정본을 설치한다. 기존 소유권 기록이 `matt`이어도 다음 switch에서 기존 설치본을 backup한 뒤 로컬 수정본으로 교체한다.

실제 설치 오류는 backup 경로를 출력한다. 다른 skill의 lock 항목을 보존하며 선택한 항목만 복구한다. Backup 이후 같은 이름의 lock이나 설치 파일, 소유권이 변경되었다면 복구를 중단한다. Symlink나 특수 파일을 남긴 비정상 실패는 기존 backup을 유지하고 수동 보존이 필요하다고 보고한다.

```bash
uv run --project agent-core scripts/install-agent-skills.py restore \
  --backup <reported-backup-directory> --apply
```

소유권 전환 자체를 되돌릴 때는 새 CLI 설치본을 먼저 백업 위치로 옮기고 이전 repo commit과 Nix generation을 복원한 뒤, 해당 파일과 lock 항목을 복구한다. Nix rollback만으로 CLI 상태가 복구되지는 않는다. Claude의 ordinary directory가 이전 Home Manager symlink 복원을 막지 않도록 한다. 복구 후에는 discovery와 관련 없는 사용자 skill 보존을 다시 검사한다.

## 전환 상태

격리 CLI 검증, source 선언, 설치 연결 명령과 shared activation 코드는 구현되어 있다. `devsisters-macbook`에서는 backup과 소유권 인계 후 Home Manager switch로 Codex/Pi 공유 skill 20개와 Claude skill 17개를 설치했다. 설치 파일 fingerprint, 소유권 기록과 lock의 일치를 확인했으며 native discovery는 아직 확인하지 않았다. 다섯 host의 Codex/Pi/Claude 전환과 discovery를 확인한 뒤에만 기존 vendored 9개와 skill renderer를 정리한다. `resolving-merge-conflicts`도 이 fallback 정리 시 제거한다.

| Host | 적용 대상 | 확인 상태와 남은 작업 |
| --- | --- | --- |
| `devsisters-macbook` | Codex/Pi, Claude | Home Manager generation 307의 switch가 완료됐다. Legacy Codex 링크와 기존 `find-skills`는 백업으로 이동했으며 Codex/Pi의 legacy 경로는 없다. 공유 skill 20개와 Claude skill 17개의 설치 파일, 소유권 기록과 lock을 검증했다. Claude의 기존 `synced` 디렉터리와 바로 아래 항목의 메타데이터도 유지됐다. 각 runtime의 native discovery가 남아 있다. |
| `devsisters-macstudio` | Codex/Pi, Claude | 실제 상태 확인, backup과 인계, switch와 discovery가 남아 있다. |
| `pylv-denim` | Codex/Pi, Claude | 실제 상태 확인, backup과 인계, switch와 discovery가 남아 있다. |
| `pylv-sepia` | Codex/Pi, Claude | 실제 상태 확인, backup과 인계, switch와 discovery가 남아 있다. |
| `pylv-onyx` | Codex/Pi, Claude | `gytkk@pylv-onyx`에서 호스트, 실행 파일과 Nix generation 경로를 확인했다. 후속 SSH 연결은 시간 초과되어 runtime 점검, backup과 인계, switch와 discovery가 남아 있다. |

## 개발과 검증

좁은 검증부터 실행한다.

```bash
uv run --project agent-core -m pytest agent-core/tests
uv run --project agent-core -m pytest scripts/tests
uv run --project agent-core agent-core check
AGENT_SKILLS_NETWORK=1 uv run --project agent-core -m pytest scripts/tests/test_skills_cli_network.py
```

Wheel에 canonical resource가 포함되는지도 확인한다.

```bash
uv build --project agent-core --out-dir /tmp/agent-core-build
uv run --isolated \
  --with /tmp/agent-core-build/agent_core-1.0.0-py3-none-any.whl \
  agent-core check
```

Runtime module을 변경할 때는 최종 Home Manager 또는 NixOS configuration을 평가한다. 실제 `home-manager switch`와 `nixos-rebuild switch`는 사용자 실행 단계다.

Golden hash는 intentional output 변경을 검토한 뒤에만 갱신한다. Hash 변경만으로 validation failure를 숨기지 않는다.

## 완료 조건

- 공통 rule, adapter, 로컬 skill과 외부 source 선언의 canonical source가 `agent-core/` 한 곳에 있다.
- Python renderer 하나가 manifest에서 세 runtime output을 생성한다.
- Runtime module에 독자적인 merge logic이나 shared skill 복사본이 없다.
- Pi의 system append 분리가 유지된다.
- Nix module은 지침과 extension을 설치하며 CLI가 skill 배치를 소유한다.
- Credentials, session, cache, 개인 memory가 materialized output에 포함되지 않는다.
- Offline test, CLI integration, package resource와 필요한 Nix 평가가 통과하며 모든 host의 실제 discovery가 확인된다.
