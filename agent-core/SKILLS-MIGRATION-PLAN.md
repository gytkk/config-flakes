# Skills CLI로 skill 설치를 전환하는 계획

2026-10-23 사용자 지시에 따라 이 계획의 공통 coding-agent 범위를 Codex, Claude와 Pi로 제한한다. 이전 OpenClaw 설치와 discovery 의무는 적용하지 않는다. 공통 지침, skill과 session recorder integration의 제거 절차는 [OpenClaw migration](../modules/openclaw/README.md#removing-the-shared-agent-integration)을 따른다. 기존 OpenClaw 설치 파일, ownership 기록과 backup은 자동 삭제하지 않으며, OpenClaw target을 포함한 이전 영수증은 해당 영수증을 생성한 이전 installer revision으로 복구한다. Codex/Claude/Pi의 모든 host 전환과 discovery를 확인하기 전에는 legacy renderer와 vendored source를 제거하지 않는다.

## 결론

Skill source 취득과 파일 배치는 버전을 고정한 `skills` CLI에 맡긴다. Agent-core는 지침 문서 생성과 skill source 선언 및 검증을 담당하고, runtime integration은 설치 경로별 소유권, 충돌 검사와 복구를 담당한다. Codex와 Pi는 `~/.agents/skills`를 함께 사용한다. Claude는 `--copy -g`로 독립적인 설치본을 둔다.

외부 dependency로 우선 전환할 skill은 `mattpocock/skills`의 9개다. Skill 탐색을 돕는 `vercel-labs/skills`의 `find-skills`도 모든 runtime의 기본 설치 대상으로 추가한다. 두 external source는 첫 설치와 이후 갱신 시 upstream 기본 branch의 최신 commit을 사용한다. 실제 workflow가 다른 `code-review`, `writing-for-agents`와 로컬 수정이 확인된 `pi-agent`, 로컬에서 관리하는 8개는 이 repo를 source로 설치한다. CLI 버전은 고정하고, 실제 설치한 source SHA는 재현과 복구를 위해 기록한다.

모든 적용 대상 host의 Codex/Claude/Pi 전환과 실제 discovery를 확인한 뒤 기존 skill renderer와 vendored source를 제거한다. Host 접근이나 검증이 불가능하면 해당 작업과 최종 정리를 미완료로 남긴다.

이 문서는 Plannotator 피드백과 코드 리뷰를 반영한 원래 계획과 조사 결과다. 구현과 호스트별 전환 상태는 [Agent core README](README.md#전환-상태)에 기록한다. 아래 조사 결과와 수량은 계획 작성 당시를 기준으로 한다.

후속 사용자 지시에 따라 설치 실행 경계를 변경했다. 현재 [Agent skills module](../modules/agent-skills/README.md)은 Home Manager switch마다 활성 agent 전체의 skill을 설치하거나 갱신하며, 한 작업에서 결정한 최신 외부 source SHA를 모든 target에 사용한다. 따라서 아래 원래 계획의 activation 외부 수동 설치 전제는 현재 동작에 적용하지 않는다. Nix build와 evaluation은 계속 외부 skill을 취득하지 않는다. 최초 switch 전에 legacy 소유권 충돌을 명시적으로 인계하고 backup하며, 정상 switch 뒤에는 별도 설치 없이 discovery를 확인한다. 직접 설치 명령은 probe, 복구와 exact SHA 재현에 남겨 둔다. 실제 host의 Codex/Claude/Pi 전환과 discovery 완료 조건은 유지한다.

실행 중 최신 Matt upstream에서 삭제된 `resolving-merge-conflicts`를 사용자 승인에 따라 새 설치 목록에서 제외했다. 이 제외 직후 대상은 Matt 8개, Vercel 1개, 로컬 11개였으며, Codex/Pi 공통 20개와 Claude 17개였다. 이후 추가한 skill을 포함한 현재 선택 목록은 [manifest](manifest.toml)를 따른다. 기존 vendored source의 삭제는 원래 계획대로 모든 호스트 전환 후에 진행한다.

## 피드백 반영

| 피드백 | 수정한 계획 |
| --- | --- |
| Codex와 Pi의 공통 경로를 분리하지 않는다. | 같은 설치 pool을 사용한다. 이전 계획의 native 제외 설정과 runtime별 격리 검증을 제거한다. 기존 합집합 20개에 `find-skills`를 더한 21개를 두 runtime에 노출한다. |
| Claude는 `--copy -g`로 설치한다. | 기존 17개에 `find-skills`를 더한 18개를 `~/.claude/skills`에 복사한다. 공유 pool에 대한 symlink는 만들지 않는다. |
| 원본 그대로 사용할 수 있는 외부 skill은 dependency로 직접 참조한다. | 모든 skill을 이 repo에 vendoring하는 전제를 제거한다. 외부 source와 갱신 정책을 선언하고, 로컬 수정본 및 로컬 작성 skill과 구분한다. |
| CLI 동작을 먼저 검증하고 전환 계약을 구체화한다. | 격리 실험 후 schema와 연결 명령을 구현한다. 설치 경로별 ownership 기록, 부분 실패 판정, 복구 순서와 단계별 검증을 명시한다. |
| Skill 탐색을 위한 `find-skills`를 기본 설치한다. | `vercel-labs/skills`를 두 번째 external source로 선언하고 Codex, Pi, Claude의 기본 선택 목록에 포함한다. |
| Matt skill과 `find-skills`는 가능하면 latest를 사용한다. | 각 설치 및 갱신 작업에서 기본 branch의 최신 SHA를 한 번 결정해 사용한다. 실제 SHA를 기록하고 재현과 rollback에는 그 기록을 사용한다. |

Codex/Pi의 공유 pool은 filesystem과 discovery 위치를 공유하는 선택이다. 보안 격리를 제공하지 않으며, 기존 runtime별 목록을 그대로 유지하는 방식도 아니다. Codex에서 `pi-agent`, Pi에서 `openai-decisions`도 발견할 수 있게 된다. Skill 설치가 관련 package나 credential을 준비해 주지는 않는다.

## 외부 dependency 조사

### 비교 기준과 방법

[현재 catalog 문서](skills/README.md)에 기록된 `mattpocock/skills`의 commit `6654f6b60cd9d5be8b54c6fafe44346dabeb3b76`, version `1.2.3`을 기준으로 11개 imported skill의 전체 파일 목록을 비교했다. 현재 repo의 generated runtime output이 아니라 `agent-core/skills/`의 원본 파일 bytes를 비교했다. 따라서 generated marker 차이는 이 결과에 섞이지 않는다.

이 비교는 로컬 수정본을 구분하기 위한 과거 snapshot의 조사 결과다. 아래 동일성 및 metadata 설명은 그 snapshot에만 적용하며, 최신 upstream의 내용이나 호환성을 보장하지 않는다. 실제 전환 시 결정한 최신 SHA에서 이름, 경로, 참조 파일과 workflow 변경을 다시 확인한다.

7개 skill은 현재 보관 중인 workflow 파일 전체가 upstream과 동일했다. 2개는 보조 스크립트의 주석만 달랐으며, 빈 줄과 comment-only line을 제외한 코드가 동일했다. 나머지 2개는 workflow 지침이 달랐다. 모든 11개에서 upstream의 `agents/openai.yaml`은 현재 repo에 없다. 직접 설치하면 이 파일도 포함되며, 외부 전환 대상 9개의 해당 파일에는 `interface.display_name`과 `interface.short_description`만 있다. 이 UI metadata의 추가는 설치/discovery 테스트에서 확인한다.

`pi-agent`는 repo에 source URL이나 SHA가 기록되어 있지 않았지만, 공개 upstream과 파일 이력을 조사해 비교 가능한 snapshot을 찾았다. 이 snapshot을 최초 import의 기록된 SHA라고 주장하지는 않는다. 비교 결과와 로컬 commit 이력을 근거로 별도로 분류한다.

### 외부 source로 전환할 9개

아래 skill은 모두 `mattpocock/skills`의 기본 branch를 추적한다. 표의 경로와 비교 결과는 위 baseline snapshot 기준이며, 각 설치 작업에서는 새로 결정한 하나의 SHA를 공통으로 사용한다.

| Skill | Upstream 경로 | 실제 비교 결과 |
| --- | --- | --- |
| `codebase-design` | `skills/engineering/codebase-design` | `SKILL.md`, `DEEPENING.md`, `DESIGN-IT-TWICE.md`가 동일하다. |
| `domain-modeling` | `skills/engineering/domain-modeling` | `SKILL.md`, `ADR-FORMAT.md`, `CONTEXT-FORMAT.md`가 동일하다. |
| `grilling` | `skills/productivity/grilling` | `SKILL.md`가 동일하다. |
| `prototype` | `skills/engineering/prototype` | `SKILL.md`, `LOGIC.md`, `UI.md`가 동일하다. |
| `research` | `skills/engineering/research` | `SKILL.md`가 동일하다. |
| `resolving-merge-conflicts` | `skills/engineering/resolving-merge-conflicts` | `SKILL.md`가 동일하다. |
| `tdd` | `skills/engineering/tdd` | `SKILL.md`, `mocking.md`, `tests.md`가 동일하다. |
| `diagnosing-bugs` | `skills/engineering/diagnosing-bugs` | `SKILL.md`는 동일하다. [보조 script](skills/diagnosing-bugs/scripts/hitl-loop.template.sh)는 header comment만 다르고 실행 코드는 동일하다. |
| `wizard` | `skills/engineering/wizard` | `SKILL.md`는 동일하다. `template.sh`는 comment만 다르고 실행 코드는 동일하다. |

`diagnosing-bugs`와 `wizard`는 로컬에서 줄이거나 번역한 주석을 유지하기 위해 fork를 보관하지 않는다. Upstream 주석을 그대로 사용하는 것이 이 계획의 의도된 변경이다. Script 동작 자체를 바꾸거나 실행할 필요는 없다.

첫 dependency source는 [mattpocock/skills](https://github.com/mattpocock/skills)의 기본 branch다. `--skill`로 위 9개만 선택한다. Setup skill, user-invoked wrapper와 그 밖의 upstream skill을 함께 설치하지 않는다. 최신 upstream에서 선택한 skill이 없어지거나 별도 setup을 요구하면 해당 변경을 보고하고 기존 설치를 보존한다. 과거 SHA로 조용히 대체하지 않는다.

### 기본 설치에 추가할 find-skills

두 번째 external source는 `https://github.com/vercel-labs/skills.git`의 기본 branch다. `skills/find-skills` 경로의 `find-skills`를 `--skill find-skills`로 선택하고 설치 및 갱신 시 최신 SHA를 결정한다. 사전 조사에서는 CLI gitHead인 `671e8c320810d36fed80fac5f1a2c1bf7e82d812`의 [skill 본문](https://github.com/vercel-labs/skills/blob/671e8c320810d36fed80fac5f1a2c1bf7e82d812/skills/find-skills/SKILL.md)과 root license를 확인했다. 이 SHA는 조사 기준이며 설치 버전 고정값이 아니다. CLI package 버전과 skill source 갱신 정책은 별도로 관리한다.

사용자가 특정 작업에 맞는 skill을 찾거나 agent 기능 확장을 원할 때 검색과 후보 제시를 돕도록 Codex/Pi 공통 pool과 Claude copy에 포함한다. 기존 vendored skill의 외부화와 구분되는 신규 기능 추가이며, CLI의 추가 설치 제안에 의존하지 않고 manifest에 기본 대상으로 선언한다. Local vendored 디렉터리는 만들지 않는다.

탐색에 사용하는 CLI도 `DISABLE_TELEMETRY=1 npx --yes skills@1.7.2 find <query>`로 버전을 고정한다. Upstream 본문의 `npx skills` 및 `update` 예시는 이 repo의 갱신 및 설치 소유권 정책을 대체하지 않는다. 검색한 후보를 repo-managed 목록에 추가할 때는 source와 갱신 정책을 검토해 manifest 및 연결 명령으로 설치한다. `find-skills` 본문은 로컬 patch 없이 사용한다.

### 로컬 수정본으로 유지할 3개

| Skill | 확인한 차이 | 계획 |
| --- | --- | --- |
| `code-review` | 원본은 issue tracker 문서가 없으면 `/setup-matt-pocock-skills` 실행을 요청한다. 로컬은 repo 문서와 git remote에서 tracker를 발견한다. Spec 조회 단계도 그 workflow를 사용한다. | 제외한 setup skill에 다시 의존하지 않도록 로컬 수정본을 유지한다. |
| `writing-for-agents` | Description과 도입부, `SKILL-MECHANICS.md`의 문구가 다르며, 원본에 없는 `BEHAVIOR-VALIDATION.md`와 이를 읽는 지침이 추가되어 있다. | Skill 선택, 로드, 실행 결과를 구분하는 로컬 검증 지침을 유지한다. |
| `pi-agent` | 아래에서 확인한 upstream snapshot과 frontmatter/prose의 requirement 위치 및 모델 예제가 다르다. | 첫 전환에서는 로컬 수정본을 유지한다. 원본 예제를 그대로 받아들이는 별도 결정이 있으면 외부 dependency로 전환할 수 있다. |

`pi-agent`의 upstream은 [K-Dense-AI/scientific-agent-skills](https://github.com/K-Dense-AI/scientific-agent-skills)이며 경로는 `skills/pi-agent`다. Local version `1.2`와 가까운 [commit ef1b2d8026bdccccf8a72f22bf3f2ea9eda76df4](https://github.com/K-Dense-AI/scientific-agent-skills/tree/ef1b2d8026bdccccf8a72f22bf3f2ea9eda76df4/skills/pi-agent)를 비교했다.

- 참고 문서 34개 중 33개는 Git blob hash가 같아서 bytes가 동일함을 확인했다.
- `SKILL.md`에서는 upstream의 Node.js/npm `compatibility` frontmatter가 로컬 본문으로 이동했다. Requirement의 의미는 같다.
- `references/models.md`는 upstream의 `gpt-5.6-sol` 예제가 로컬의 `gpt-6.1-sol` 예제로 변경되어 있다. 로컬 commit `7690a53`과 `a749fbc`에 모델 이름 갱신 이력이 있다.
- Local `LICENSE.md`는 upstream repo root의 `LICENSE.md`와 bytes가 동일하다. Upstream skill 디렉터리에는 이 license 파일이 없다.
- 비교 시점의 최신 skill 변경 commit `a92663a0214b3b9fa1ba6b958284ffd919c00a0c`에서는 `SKILL.md`와 기존 참고 문서 34개가 모두 다르고, `references/mcp.md`가 추가되었다. 최신 원본 채택은 설치 도구 전환과 구분해야 하는 content upgrade다.

따라서 `pi-agent`를 외부로 옮기는 데 기술적인 source 문제가 있는 것은 아니다. 위 고정 snapshot을 참조할 수 있지만, 로컬 모델 예제 변경을 되돌리는 결정이 필요하다. 첫 전환에서는 이 차이를 묵시적으로 없애지 않는다. 이후 외부화한다면 requirement 위치 복원과 예제 변경을 검토하고 license를 포함한다. Runtime의 모델 설정은 변경하지 않는다.

### 로컬에서 관리하는 8개

`data-analysis`, `data-visualization`, `devils-advocate`, `openai-decisions`, `parallel-research-merge`, `repeated-work-analyzer`, `theme-template-authoring`, `user-friction-review`는 catalog 문서상 로컬에서 관리한다. 동일한 외부 원본으로 대체할 근거가 없으므로 이 repo를 source로 사용한다.

Quokka에서 참고한 분석/검증 workflow는 독립적으로 작성한 지침이다. Private Quokka skill dependency로 전환하지 않는다. `openai-decisions`가 사용하는 외부 API와 이 skill 문서의 source도 서로 다른 dependency다.

초기 전환 결과는 외부 10개(Matt 9개와 `find-skills` 1개), 로컬 11개로 총 21개다. 로컬 11개의 내용은 유지하고, 외부 10개에는 최신 upstream의 workflow와 metadata 변경을 반영한다. 따라서 첫 전환에는 설치 도구 변경과 외부 skill content upgrade가 함께 포함되며, 기존 20개 전체의 workflow 보존을 완료 조건으로 삼지 않는다.

## Dependency를 참조하는 방식

### CLI 버전 고정과 최신 source 선택

검토한 CLI는 npm 배포본 `skills@1.7.2`이며, npm metadata의 `gitHead`는 `671e8c320810d36fed80fac5f1a2c1bf7e82d812`다. 해당 버전의 bundle을 읽었고, 코드를 실행하거나 package를 설치하지 않았다.

이 버전은 GitHub `/tree/<ref>` URL과 `#<ref>`를 파싱한다. 40자리 SHA는 branch clone 실패 후 `git fetch --depth 1 origin <SHA>`로 취득하는 fallback을 갖고 있다. 두 upstream의 위 고정 SHA를 별도 임시 Git 디렉터리에서 실제로 fetch하고 `FETCH_HEAD` 일치를 확인했다. 이는 Git 취득 검증이며, `npx skills` 설치의 end-to-end 검증은 아직 아니다.

`owner/repo@<SHA>`는 사용하지 않는다. 이 CLI에서 `@...`는 revision이 아니라 skill 이름 filter다. 여기서 latest는 npm CLI의 `latest`나 release tag가 아니라 source repo 기본 branch의 최신 commit을 뜻한다. 두 repo의 현재 기본 branch는 `main`이며, 실행 시 remote HEAD로 기본 branch와 SHA를 확인한다.

연결 명령은 source별로 최신 SHA를 작업 시작 시 한 번 결정하고, 그 작업에 속한 모든 runtime 설치에 같은 SHA를 전달한다. Remote ref 조회만 연결 계층에서 수행하고 source 취득과 파일 배치는 CLI에 맡긴다. 별도 작업으로 갱신하는 host는 다른 SHA를 사용할 수 있으므로 각 설치 경로에 실제 SHA를 기록한다. 최신 조회나 검증이 실패하면 기존 설치를 보존하고 실패를 보고한다.

설치 형태의 예시는 다음과 같다. 아직 실행하지 않은 명령이며, `MATT_SKILLS_REV`와 `VERCEL_SKILLS_REV`에는 그 작업에서 결정하고 40자리 SHA임을 검증한 값을 넣는다. 이 실행 중의 SHA 고정은 다음 갱신 시 새 latest를 선택하는 것을 막지 않는다.

```bash
DISABLE_TELEMETRY=1 npx --yes skills@1.7.2 add \
  "https://github.com/mattpocock/skills/tree/${MATT_SKILLS_REV:?}" \
  --skill codebase-design domain-modeling grilling prototype research \
    resolving-merge-conflicts tdd diagnosing-bugs wizard \
  --agent codex pi --global --yes --json
```

`find-skills`는 별도 source 호출로 설치한다. 아래도 아직 실행하지 않은 계획 예시다.

```bash
DISABLE_TELEMETRY=1 npx --yes skills@1.7.2 add \
  "https://github.com/vercel-labs/skills/tree/${VERCEL_SKILLS_REV:?}" \
  --skill find-skills --agent codex pi --global --yes --json
```

Claude에는 Matt의 9개와 `find-skills`를 각 source별로 `--agent claude-code --copy --global --yes --json`을 사용해 설치한다. Local source는 이 repo checkout의 `agent-core/skills/` 절대 경로다.

첫 npx 실행에는 npm 접근이, 외부 Git source 취득에는 GitHub 접근이 필요할 수 있다. Nix 빌드와 Home Manager activation 안에서는 npm 설치나 외부 skill 다운로드를 수행하지 않는다. 설치는 명시적인 사용자 실행 작업이며, network 실패를 성공으로 처리하지 않는다.

### 하나의 source 선언과 설치 정책

기존 [manifest](manifest.toml)에 skill 이름과 source의 대응을 선언한다. Local source와 external source를 구분하고, 두 external에는 repo URL, 기본 branch의 latest를 추적하는 정책, 선택할 이름과 upstream 경로를 기록한다. 실제 40자리 commit SHA는 source 선언의 영구 고정값 대신 설치 작업 및 ownership 기록에 저장한다. Runtime별 설치 대상도 같은 manifest에서 정한다. Module마다 skill 목록을 복제하거나 CLI가 manifest를 직접 해석한다고 가정하지 않는다.

초기 선언은 external source 두 개에 Matt의 9개와 Vercel의 `find-skills` 1개를 각각 배정하고, local source에 나머지 11개를 둔다. Codex/Pi는 기존 합집합에 `find-skills`를 더한 21개를 하나의 pool로 설치하고, Claude는 기존 17개에 1개를 더한 18개를 선택한다. Source 사이에 같은 이름이 중복 선언되면 오류로 처리한다.

`package.json`의 `skills` field와 `experimental_sync`도 조사했다. 검토한 배포본에는 Git source 선언과 설치 처리가 있지만, local directory를 해당 field에 넣으면 `is not a git source` 오류가 된다. 또한 remote 설치 경로에는 copy 설정 전달을 별도로 확인해야 한다. 이 experimental 기능을 기반으로 새 npm project나 `node_modules` 관리 구조를 추가하지 않는다. 작은 연결 명령이 manifest의 source별 선택 목록을 CLI의 안정적인 `add` 명령에 전달하는 방식을 사용한다.

CLI의 global lock은 CLI의 설치 기록으로 취급한다. Global lock은 `XDG_STATE_HOME`이 설정되면 그 아래 `skills/.skill-lock.json`, 아니면 `~/.agents/.skill-lock.json`에 있다. 이를 전체 source 선언이나 frozen dependency resolver로 간주하지 않는다. 일반 설치 및 갱신은 manifest의 latest 정책에 따라 SHA를 새로 결정하고, 동일 버전 재현과 rollback은 저장한 SHA나 backup을 사용한다. CLI의 포괄적인 `update` 대신 연결 명령으로 선택한 source와 skill만 갱신한다. 주기적 자동 갱신은 이 계획에 추가하지 않는다.

License notice는 실제 설치 SHA에 맞춰 보존한다. Matt의 현재 `skills/LICENSE`는 조사 baseline의 root `LICENSE`와 동일하지만 최신 설치본에 과거 notice를 재사용하지 않는다. CLI가 root license를 skill 디렉터리에 자동으로 포함한다고 가정하지 않으며, 설치 결과에 notice가 없으면 설치한 revision의 license를 포함하는 최소한의 metadata 단계를 둔다. License 보완 파일은 content 검증과 ownership 기록에서 명시적으로 구분한다. Skill 본문에 로컬 patch를 다시 적용하는 방식으로 외부 dependency를 유지하지 않는다.

`find-skills`에도 실제 설치한 Vercel revision의 root `LICENSE`를 같은 방식으로 보존한다. 사전 조사에서 읽은 [license](https://github.com/vercel-labs/skills/blob/671e8c320810d36fed80fac5f1a2c1bf7e82d812/LICENSE)는 비교 기준으로 남긴다. 두 source의 notice를 혼동하거나 Matt의 license로 대신하지 않는다.

### 설치 상태와 실패 처리

Global lock은 `skills[skillName]`으로 기록하며 설치 경로별 상태를 구분하지 않는다. Shared pool만 갱신하고 Claude copy를 이전 상태로 유지하면 하나의 이름 기록으로 두 설치본의 상태를 표현할 수 없다. 따라서 runtime integration은 처음부터 `설치 경로 + skill 이름`을 key로 하는 작은 ownership 기록을 둔다. Source 식별자, external revision 또는 local source의 content hash, 설치 직후의 파일 목록과 hash, 보완한 license 파일을 기록한다. 이 기록은 manifest의 선택 정책을 복제하지 않고, 설치된 상태와 사용자 수정 여부를 판단하는 데만 사용한다.

검토한 CLI에는 일반 출력 모드에서 일부 설치 실패를 표시하고도 실패 종료 코드를 설정하지 않는 경로가 있다. `--json` 모드는 실패 결과에 종료 코드를 설정하지만, 일부 성공 항목의 CLI lock 갱신은 전체 작업 성공과 별개로 일어날 수 있다. 연결 명령은 종료 코드, JSON의 각 결과와 기대 이름 집합, 실제 설치 파일을 함께 검증한다. CLI lock이 갱신되었다는 사실을 전체 설치의 성공으로 취급하지 않는다.

변경 전에 영향받는 repo-owned 설치본과 CLI lock 항목, ownership 기록을 backup한다. 검증이 모두 끝난 작업 단위만 ownership 성공 상태로 기록한다. 부분 실패 시 성공한 경로와 실패한 경로를 모두 보고하고, 이전 파일 및 해당 lock 항목을 복구할 수 있게 한다. 전체 global lock을 덮어써 다른 skill의 기록을 되돌리지 않는다. 복구가 끝나기 전에는 정상 완료로 기록하지 않는다.

연결 계층은 manifest 전달, 충돌 검사, CLI 호출, 결과 검증과 제한된 상태 기록 및 복구만 담당한다. Source downloader, 일반적인 copy engine, 자동 충돌 해결이나 범용 transaction engine은 추가하지 않는다. 격리 실험에서 이 범위를 넘는 별도 설치 framework가 필요해지면, schema 확장 전에 CLI 채택의 이점과 구현 범위를 다시 판단한다.

## 목표 설치 구조

| Runtime | Repo-managed skill 수 | 설치 위치와 방식 |
| --- | --- | --- |
| Codex/Pi | 공통 21개 | `find-skills`를 포함해 `~/.agents/skills`에 공통 pool로 global 설치한다. 두 runtime을 분리하는 경로나 native 제외 설정을 추가하지 않는다. |
| Claude | 18개 | `find-skills`를 포함해 `~/.claude/skills`에 `--copy --global`로 설치한다. Marketplace plugin skill은 그대로 유지한다. |

Shared global pool은 Claude와 별개로 갱신한다. 원본 수정이 checkout에서 설치본으로 실시간 반영되는 것은 아니다. 연결 명령을 다시 실행하면 external source의 latest를 새로 선택하고 local source 수정도 반영한다. 동일 설치 상태의 재현은 기록한 SHA와 local content 기준을 사용해 별도로 수행한다.


## 구현 단계

### 1. 격리 실험과 CLI 채택 조건 확인

1. 기존 manifest와 skill renderer를 유지한 상태에서 임시 HOME, XDG 경로, npm cache와 작업 디렉터리로 pinned CLI를 검증한다. User credential을 상속하거나 runtime 설정을 복사하지 않고, skill에 포함된 보조 script는 실행하지 않는다.
2. 두 external source의 latest SHA를 결정해 10개와 local 선택 목록을 설치한다. 정확한 이름 집합, 전체 파일 목록과 bytes, 보조 문서 및 script, UI metadata와 source별 license를 해당 SHA와 대조한다. 이전 조사 baseline과의 content 차이도 확인하되 동일성을 요구하지 않는다. `--skill`이 의도한 항목만 선택하고 `find-skills`가 모든 runtime 대상에 포함되는지 확인한다.
3. Codex/Pi shared pool과 Claude global copy를 검증한다. 설치 파일에 symlink나 hardlink가 없는지 확인하고, root checkout에 `.agents/`나 설치 lock이 생기지 않게 한다.
4. 재설치, source 파일 삭제 반영, 충돌과 사용자 수정, 부분 실패를 재현한다. 종료 코드와 JSON, 파일 및 lock의 실제 변화를 대조하고 backup에서 복구하는 절차를 검증한다.
5. 실제 CLI 취득 및 설치 테스트는 기본 pytest와 분리된 명시적 실행 대상으로 둔다. 필요한 Node/npm/Git와 network 조건을 해당 테스트 실행 방법에 기록한다.

완료 조건: Shared pool과 global copy 배치 방식 및 실패 처리에 대한 실제 증거가 있고, 위에서 정한 작은 연결 계층으로 요구사항을 충족할 수 있어야 한다. 이 단계의 파일 설치 검증은 3단계의 실제 host discovery를 대체하지 않는다. 조건을 충족하지 못하면 원인을 해결하거나 설치 방식을 재검토한 뒤 다음 단계로 진행한다.

### 2. Source 선언과 사용자 설치 명령

1. Runtime별 기존 지침 파일 bytes를 비교 기준으로 고정한다. Manifest schema에 mixed source 선언을 추가하고 이름 중복, external 갱신 정책, 안전한 경로, local source 존재와 선택 목록을 검증한다. 실행 시 결정한 SHA와 복구 기록에는 full SHA 검증을 적용한다. Dependency 선언 변경은 구현 승인 후 수행한다.
2. Agent-core에 source별 선택 목록을 읽기 전용으로 출력하는 작은 interface를 마련한다. Pure renderer는 홈 디렉터리나 네트워크를 읽지 않는다. Local skill에는 기존 구조 검증을 적용하고 external source에는 선언 검증을 적용하며, external 파일의 구조와 bytes는 취득 후 검증한다.
3. 전환 기간에는 기존 vendored 9개와 skill render output을 유지한다. 새 source 선언은 CLI 설치 계획에 적용하고, 기존 renderer는 4단계까지 로컬 tree를 사용하는 임시 배포 경로로 남긴다. 새 manifest를 읽을 수 있도록 parser를 수정하되 legacy render의 선택 목록과 bytes는 보존한다. 신규 `find-skills`는 CLI 설치 선택에만 추가하며 legacy renderer가 로컬 source를 요구하지 않도록 구분한다.
4. Host 경로와 CLI 실행은 runtime integration이 담당한다. 실행 전 변경 계획을 출력하고, 동명 타 source 설치나 설치본의 사용자 수정이 있으면 중단한다. 기존 Nix 관리 항목의 최초 소유권 인계는 3단계에서 별도로 검증한다.
5. 설치 경로별 ownership 기록, `--json` 및 실제 파일 검증, 영향받는 항목의 backup과 복구를 구현한다. Shared pool과 Claude copy를 서로 다른 revision으로 설치한 경우에도 각각의 상태와 사용자 수정을 구분하는지 검사한다.
6. Source 간 이동과 stale skill 정리는 이전 관리 대상과 content를 확인하고 사용자 승인 후 수행한다. Shared pool에서 다른 runtime이 사용하는 항목과 marketplace 또는 개인 skill은 삭제하지 않는다. 실패 시 source, skill, 대상 경로와 실패 단계를 보고한다.

완료 조건: 같은 source SHA와 local content로 재현하면 설치 content와 소유권이 같고, 일반 갱신은 새 latest를 반영한다. Local/external source를 동일한 명령으로 갱신할 수 있으며, 사용자 변경과 관련 없는 skill을 보존한다. 기존 Nix 배포는 계속 동작하고 runtime 지침 bytes는 기존 기준과 같아야 한다.

### 3. 설치 소유권 전환

1. [Inventory](../systems/inventory.nix)와 실제 적용 상태에서 대상 host/runtime 목록을 확정하고 각각의 상태를 추적한다. 대상 runtime은 Codex, Claude와 Pi다. 기존 경로와 새 CLI 경로의 타입, content와 충돌, 이전 Nix generation과 복구 가능성을 점검한다.
2. [Claude module](../modules/claude/default.nix)의 `~/.claude/skills`, [Pi module](../modules/pi/default.nix)의 `~/.pi/agent/skills`에 대한 directory-level Home Manager 선언을 제거한다. [Codex system module](../modules/codex/system.nix)과 [standalone activation](../modules/codex/system-activation.nix)의 `/etc/codex/skills` 관리도 제거한다. 생성 지침, `/etc/codex/config.toml`과 runtime 설정은 유지한다.
3. 각 모듈의 소유권 제거와 [flake source wiring assertion](../flake.nix)의 관련 변경을 같은 커밋에 넣는다. Coding-agent 지침과 extension assertion은 유지한다.
4. Host별 switch 전에 pinned CLI와 필요한 source의 취득 가능성을 확인하고 영향받는 파일 및 상태를 backup한다. Npm cache만으로 Git source의 재취득이 보장된다고 가정하지 않는다. 사용자가 Home Manager/NixOS switch를 실행한 뒤 CLI 설치 명령을 실행한다. 설치 실패 시 아래 rollback 순서로 기존 상태를 복구한다.
5. Standalone Codex의 `/etc/codex/skills`는 선언 제거만으로 남을 수 있으므로 별도로 점검한다. Legacy path는 repo 관리 대상임을 확인하고 승인한 경우에만 제거한다. 사용자 파일에 대한 포괄적인 recursive delete는 하지 않는다.
6. 모든 대상 runtime의 실제 이름 집합과 발견 경로, 중복 discovery를 확인한다. Host 접근이나 검증이 불가능하면 해당 작업과 4단계를 미완료로 남긴다.

완료 조건: 모든 적용 대상 host에서 설치 경로별 담당자가 하나뿐이며, legacy와 새 설치본이 중복 노출되지 않는다. `find-skills`를 포함해 Codex/Pi 공통 21개와 Claude 18개가 각 runtime에서 발견된다. 관련 없는 skill과 plugin은 보존되어야 한다.

### 4. Vendoring과 renderer 정리

1. 진입 조건은 3단계의 모든 대상 host/runtime 전환 완료와 기존 skill render output 소비자의 제거다. pylv-onyx 검증이 남아 있으면 이 단계도 진행하지 않는다. 조건 충족 후 9개 vendored 디렉터리를 제거하고 local 수정본 3개와 로컬 관리 8개, 필요한 license notice는 유지한다.
2. Agent-core의 skill 배포 tree 생성과 generated skill marker를 제거한다. 지침 문서 bytes는 변경하지 않는다. External dependency 내용은 지침 render에 합치지 않는다.
3. Source 검증, 선택 정책과 pure document render 테스트를 유지한다. External 취득/설치/discovery는 별도 integration test로 분리한다. Offline `agent-core check`가 없는 external local directory를 요구하지 않게 한다.
4. Runtime별 지침 파일 bytes를 2단계에서 고정한 기준과 직접 비교한다. 일치를 확인하고 의도된 output layout 변경을 검토한 뒤 Python의 [golden hash](schema/golden-hashes.toml)와 [Nix recursive hash](nix/runtime-golden-hashes.nix)를 각각 갱신한다. 전체 hash 갱신만으로 지침 보존을 입증하지 않는다. Skill 설치 검증은 CLI integration과 runtime smoke test로 옮긴다.
5. Agent-core와 각 runtime의 기존 README, root 문서의 관련 설명을 갱신한다. 최신 외부 skill의 content upgrade가 설치 도구 전환에 포함됨을 명시하고, native shared discovery와 copy 설치 및 latest 갱신과 기록한 SHA 재현의 차이를 설명한다.

완료 조건: Matt skill 9개의 원문 복사본과 별도 배포 구현이 없어지고, 지침 생성 및 로컬 source 검증은 계속 재현 가능하다. 두 external source의 revision과 취득 후 검증 책임이 명확하다.

## 검증과 rollback

### 검증 구분

- 기본 offline suite는 mixed source 선언, 갱신 정책, 중복 이름, 기록된 SHA와 경로, target별 선택 목록, ownership 비교와 지침 bytes 보존을 확인한다. [Nix package](nix/package.nix)의 `pytestCheckHook`과 기본 pytest 실행에 npm 설치, network 접근이나 실제 host 상태를 요구하지 않는다.
- 실제 CLI integration suite는 별도 opt-in 명령으로 실행한다. Latest 조회와 결정한 exact SHA 설치, 동일 작업 내 runtime별 SHA 일치, upstream 전진 후 갱신과 이전 SHA 재현을 확인한다. Local 수정 반영, 선택 목록 변경, 동명 충돌, 사용자 수정, 삭제, license 누락, 부분 실패 및 복구와 무관한 skill 보존도 검사한다. 실행하지 못한 경우 통과로 보고하지 않는다.
- Runtime smoke test의 기대 이름은 manifest에서 가져온다. 현재 설치 디렉터리에서 기대 목록을 역으로 만들지 않고, repo-managed 이름의 정확한 집합과 실제 발견 경로를 비교한다. 관련 없는 skill을 전체 개수 비교로 배제하지 않는다.
- `find-skills`는 각 runtime의 기본 목록과 실제 loader에서 이름, description 및 source 경로를 확인한다. Network integration에서는 pinned CLI로 검색해 후보 이름과 source를 얻을 수 있는지 확인하되 검색 순위나 개수를 고정하지 않는다. 검색 후보의 추가 설치는 이 검증에 포함하지 않으며, loader 및 검색 성공을 모델의 실제 skill 선택 행동 검증으로 보고하지 않는다.
- `uv run --project agent-core -m pytest agent-core/tests`, `uv run --project agent-core agent-core check`와 바뀐 범위의 runtime test를 실행한다. CLI integration과 실제 host discovery 검사는 각각 명시적으로 실행하고 결과를 구분한다. 변경 Nix 파일에 `nixfmt`를 적용하고 소유권 전환 커밋마다 필요한 source wiring targeted evaluation을 수행한다. Package build는 기본적으로 하지 않는다.
- 문서는 `uv run --no-project scripts/check-paths.py`로 검사하고 필요한 변경 Markdown을 명시한다. Model 호출이나 실제 서비스 재시작은 discovery 검사에 불필요하면 하지 않는다.

### 복구 순서

1. 실패한 작업을 멈추고 변경된 설치 경로, CLI lock 항목과 ownership 기록을 확인한다. 새 repo-owned 설치본은 확인 후 backup 위치로 이동한다. 사용자 수정이나 무관한 skill이 섞여 있으면 보존하고 충돌을 보고한다.
2. 이전 repo commit과 Nix generation으로 설치 소유권을 되돌린다. Claude의 ordinary directory가 기존 Home Manager symlink 복원을 막지 않도록 먼저 해당 경로를 정리한다. Codex/Pi shared pool의 새 repo-owned 항목도 기존 경로와 중복 노출되지 않도록 처리한다.
3. 필요한 이전 설치 파일과 해당 CLI lock 항목, ownership 기록을 backup에서 복원한다. External 파일을 다시 취득해야 하면 이전 설치 SHA를 사용하고 latest를 재조회하지 않는다. Global lock 전체나 unrelated user skill을 덮어쓰지 않는다.
4. 각 runtime의 이름과 source 경로, 사용자 skill 보존과 중복 discovery를 재검사한다. 이 순서의 복구 가능성을 integration fixture로 검증하며, 실제 host 복구가 필요하면 완료 여부를 별도로 보고한다.

CLI 설치 상태는 Nix generation 밖에 있으므로 Nix rollback만으로 복원되지 않는다. 구현은 격리 검증, source 선언과 CLI integration, runtime 소유권과 assertion 전환, obsolete render/vendoring 정리의 검증 가능한 단위로 나눠 local commit을 남긴다. Push, switch/rebuild와 실제 사용자 상태 변경은 자동 수행하지 않는다.

## 조사에서 검증한 것과 남은 확인

검증한 사항은 Matt의 11개 전체 file-set/bytes 비교, 보조 script 2개의 non-comment code 일치, Pi baseline의 34개 참고 문서 Git blob 비교, 달라진 Pi 본문과 모델 예제 및 license 비교, 최신 Pi snapshot의 변경 목록, npm CLI 배포본의 parser/installer 읽기, 두 full SHA의 shallow fetch다. Git/network 취득은 임시 디렉터리에서만 수행했다.

추가 코드 리뷰에서는 npm 배포본의 이름별 global lock, XDG lock 경로, 일부 성공 항목의 lock 갱신, 일반 출력과 JSON 모드의 실패 종료 처리 차이를 확인했다. 당시 repo의 module 소유권과 flake assertion 의존 관계, Nix의 기본 pytest 실행, 당시 OpenClaw smoke의 설치 디렉터리 기반 기대 목록도 대조했다. 이는 코드 읽기로 확인한 동작이며 실제 실패 재현 결과는 아니다.

`find-skills`는 조사 baseline인 Vercel commit `671e8c320810d36fed80fac5f1a2c1bf7e82d812`에서 `skills/find-skills/SKILL.md`와 root `LICENSE`를 읽어 경로와 탐색 workflow를 확인했다. 이 결과를 latest 내용 검증으로 간주하지 않는다. 실제 설치 시 결정할 두 source의 최신 SHA에 대한 전체 파일 검증, 설치 및 검색 명령은 1단계와 network integration의 검증 대상이다.

원래 조사 시점에 검증하지 않은 사항은 `skills@1.7.2`의 실제 설치 실행, lock/overwrite/retry와 복구의 end-to-end 동작, runtime에서의 실제 discovery와 copy/metadata 영향이었다. CLI 동작은 1단계에서, 실제 host 전환과 discovery는 3단계에서 확인한다. 구조나 bytes 비교를 skill 행동 검증으로 보고하지 않는다.

## 근거

- [현재 skill catalog와 import 기록](skills/README.md), [runtime 선택 manifest](manifest.toml), [renderer](src/agent_core/core.py), [renderer 테스트](tests/test_core.py).
- [Matt upstream 고정 tree](https://github.com/mattpocock/skills/tree/6654f6b60cd9d5be8b54c6fafe44346dabeb3b76), [code-review 원본](https://github.com/mattpocock/skills/blob/6654f6b60cd9d5be8b54c6fafe44346dabeb3b76/skills/engineering/code-review/SKILL.md), [writing-for-agents 원본](https://github.com/mattpocock/skills/tree/6654f6b60cd9d5be8b54c6fafe44346dabeb3b76/skills/productivity/writing-for-agents).
- [Pi 비교 baseline](https://github.com/K-Dense-AI/scientific-agent-skills/tree/ef1b2d8026bdccccf8a72f22bf3f2ea9eda76df4/skills/pi-agent), [baseline 모델 예제](https://github.com/K-Dense-AI/scientific-agent-skills/blob/ef1b2d8026bdccccf8a72f22bf3f2ea9eda76df4/skills/pi-agent/references/models.md), [baseline license](https://github.com/K-Dense-AI/scientific-agent-skills/blob/ef1b2d8026bdccccf8a72f22bf3f2ea9eda76df4/LICENSE.md), [최신 skill 변경 snapshot](https://github.com/K-Dense-AI/scientific-agent-skills/tree/a92663a0214b3b9fa1ba6b958284ffd919c00a0c/skills/pi-agent).
- [skills@1.7.2 npm 배포본](https://registry.npmjs.org/skills/-/skills-1.7.2.tgz), [해당 gitHead의 source parser](https://github.com/vercel-labs/skills/blob/671e8c320810d36fed80fac5f1a2c1bf7e82d812/src/source-parser.ts), [Git 취득 코드](https://github.com/vercel-labs/skills/blob/671e8c320810d36fed80fac5f1a2c1bf7e82d812/src/git.ts), [installer](https://github.com/vercel-labs/skills/blob/671e8c320810d36fed80fac5f1a2c1bf7e82d812/src/installer.ts), [CLI README](https://github.com/vercel-labs/skills/blob/671e8c320810d36fed80fac5f1a2c1bf7e82d812/README.md).
- [Global lock 구현](https://github.com/vercel-labs/skills/blob/671e8c320810d36fed80fac5f1a2c1bf7e82d812/src/skill-lock.ts), [설치 결과와 lock 갱신 및 JSON 처리](https://github.com/vercel-labs/skills/blob/671e8c320810d36fed80fac5f1a2c1bf7e82d812/src/add.ts).
- [find-skills 조사 baseline](https://github.com/vercel-labs/skills/tree/671e8c320810d36fed80fac5f1a2c1bf7e82d812/skills/find-skills), [Vercel snapshot license](https://github.com/vercel-labs/skills/blob/671e8c320810d36fed80fac5f1a2c1bf7e82d812/LICENSE).
- [Matt 최신 source](https://github.com/mattpocock/skills), [Vercel 최신 source](https://github.com/vercel-labs/skills).
