# 저장소 구조 및 리팩토링 검토

전면적인 디렉터리 재편보다 activation, skill 설치, 세션 기록의 파일 변경과 복구 절차를 바로잡는 것이 우선이다. 현재 `inventory -> builder -> profile -> module` 구조를 전면 교체할 근거는 찾지 못했다.

이 문서는 2026-10-30에 수행한 리뷰의 기록이다. 기준 commit은 `8040d610ca506be6ec43435d0fbdb2137c510236`이며, 당시 작업 트리에는 사용자가 수정한 `modules/claude/files/settings.json`이 있었다. 리뷰에서는 해당 수정을 보존했고 코드를 변경하지 않았다. 아래 줄 번호와 원인 설명은 리뷰 시점 기준이다. 이후 수정 여부와 검증 결과는 각 항목의 상태에 기록한다.

검토 대상은 개인용 Mac, WSL, NixOS 환경, 서버 서비스, 에이전트 설치와 기록 도구다. 개인 사용자가 여러 에이전트를 동시에 실행하는 부하를 기준으로 평가했다. Astra 에이전트 두 개와 Sol 에이전트 한 개가 독립적인 영역을 조사했고, 주 에이전트가 Nix 구성과 activation을 조사한 뒤 결정에 영향을 주는 근거를 원문 및 추가 재현과 대조했다. 구조, 유지보수, 예외 상황 순서로 검토하고 후보를 재검토했다. 모든 파일과 외부 의존성을 전수 검증한 것은 아니다.

판정은 `needs_work`, 신뢰도는 8/10이다. P1은 우선 수정할 운영 및 파일 보호 문제이고, P2는 조건부 오류, 성능 또는 유지보수 문제다. 합성 데이터에서 재현한 실패와 실제 운영 환경에서의 발생 여부를 구분한다.

## 확인한 문제

### 1. [P1] Sepia의 인증 없는 서비스가 Tailscale로 제한되지 않음

상태: 부분 해결 (2026-10-30). 사용하지 않는 code-server 서비스, 전용 빌드 패치, TCP 8080 방화벽 허용을 제거했다. Copyparty 접근 범위는 이번 수정 요청에 포함되지 않아 미해결이며, Sepia에 실제 NixOS 설정을 적용하지 않았다.

근거: [configuration.nix](../hosts/pylv-sepia/configuration.nix)의 39행과 116행.

code-server는 `0.0.0.0:8080`, `auth = "none"`으로 실행되지만 방화벽은 해당 포트를 모든 인터페이스에 허용한다. Copyparty도 모든 인터페이스에 바인딩하고 익명 읽기와 쓰기를 허용한다. code-server 주석의 Tailscale 접근 제한을 이 설정만으로는 보장하지 않는다.

호스트에 도달할 수 있는 비-Tailscale 네트워크에서도 접근 가능한 설정이므로, 의도한 접근 범위를 인터페이스별 방화벽 규칙으로 명시해야 한다. 실제 인터넷 노출 여부와 외부 네트워크의 추가 접근 제어는 확인하지 않았다.

### 2. [P1] AeroSpace 마이그레이션이 첫 Home Manager 적용을 막을 수 있음

상태: 해결 (2026-10-30). 실행 순서를 `linkGeneration` 이후로 명시하고, 전체 마이그레이션을 Home Manager의 `run`으로 실행한다. 실제 `devsisters-macbook` activation DAG를 offline으로 평가해 `linkGeneration` 다음에 `aerospaceLegacyConfig`가 실행되는 것을 확인했다. [Activation 회귀 테스트](../scripts/tests/test_config_activation.py)에서 순서 의존성, 첫 적용, 재적용, 관리 설정이 없는 dry-run과 기존 파일 보존을 검증했다. 서로 다른 legacy 파일과 백업이 동시에 있으면 두 파일을 보존하고 중단하는 기존 보호 동작은 유지한다.

근거: [AeroSpace 모듈](../modules/aerospace/default.nix)의 26행.

마이그레이션은 새 설정 파일이 존재해야 진행하지만, 실행 순서에는 `writeBoundary` 이후라는 조건만 선언한다. 실제 Mac 설정의 activation 순서를 평가한 결과 `aerospaceLegacyConfig`가 설정 symlink를 만드는 `linkGeneration`보다 먼저 실행됐다.

새 환경에서는 파일이 없어 적용이 중단될 수 있다. `linkGeneration` 이후 실행을 명시하고, 기존 설정이 없는 첫 적용을 검증해야 한다.

### 3. [P1] Activation의 dry-run과 파일 교체가 기존 설정을 보호하지 못함

상태: 해결 (2026-10-30). AeroSpace, Zed, Windows Terminal의 파일 변경 전체를 `run`으로 감쌌다. Zed는 설정을 임시 디렉터리에 복사하고 바이트가 같은지 검사한 뒤 교체하며, Windows Terminal은 임시 파일의 jq 변환이 성공한 뒤 교체한다. 변경 직전 설정을 백업하고, Zed 확장 교체 실패 시 이전 디렉터리를 복원하거나 복원 실패 위치를 보존한다. [Activation 회귀 테스트](../scripts/tests/test_config_activation.py) 17개가 통과했다. Dry-run, 원본 누락, 불완전한 복사, JSON 변환 실패, rename 실패, 백업 갱신을 임시 디렉터리에서 검증했다. 실제 WSL 파일시스템과 Home Manager switch는 검증하지 않았다.

근거: [AeroSpace 모듈](../modules/aerospace/default.nix)의 38행, [Zed 모듈](../modules/zed/default.nix)의 74행과 82행, [Windows Terminal 모듈](../modules/windows-terminal/default.nix)의 23행.

이 activation들은 Home Manager의 `run` 처리를 우회해 파일을 변경한다. 임시 환경에서 dry-run 중 AeroSpace 설정 삭제와 Zed 설정 덮어쓰기를 재현했다. Windows Terminal의 직접 쓰기 경로는 코드로 확인했다.

Zed는 원본 설정 파일을 읽을 수 없을 때 설치된 설정부터 삭제하고 복사에 실패했다. 한 번만 생성하는 백업에는 이후 변경한 설정이 포함되지 않을 수 있다.

기존 `run` 기능을 사용하고, 복사와 검증이 끝난 임시 파일을 마지막에 교체해야 한다. 파일 출력용 redirection도 dry-run에서 실행되지 않도록 처리해야 한다. 별도 activation 프레임워크는 필요하지 않다.

### 4. [P1] Skill 설치 실패를 기록하다가 복구 기능까지 실패함

상태: 해결 (2026-10-30). 설치 오류와 후속 상태 수집 오류를 분리해 원래 오류와 백업 위치가 실패 기록에 남는다. 손상된 owner와 lock JSON은 바이트로 기록하고, 복구 전 해당 바이트와 설치 파일이 그대로인지 확인한다. 손상된 lock은 설치 전 전체 바이트로 복원하며, 정상 lock은 기존처럼 선택한 항목만 복원한다. 읽을 수 없어 상태를 확인하지 못한 파일은 자동으로 덮어쓰지 않는다. [Installer 테스트](../scripts/tests/test_install_agent_skills.py) 51개가 통과했으며, owner와 lock 손상, 이후 변경 시 복구 거부, 첫 설치의 lock 제거, 기록 저장 실패 시 원래 오류 보존을 검증했다. 기록 디렉터리에 쓸 수 없는 경우 실패 기록 저장까지 보장하지는 못하며, 오류 메시지에 백업 위치를 남긴다. 자식 프로세스 timeout 수정은 5번에 기록한다.

근거: [install-agent-skills.py](../scripts/install-agent-skills.py)의 701행부터 704행과 355행의 `validate_receipt`.

`finally`에서 owner와 lock JSON을 읽다가 예외가 발생하면 실패 기록 저장이 생략된다. 설치 도중 lock JSON이 손상되는 상황을 주입하자 실제 skill은 변경됐지만 기록은 `prepared`에 머물렀고, `restore()`가 복구를 거부했다. 원래 오류에 포함된 백업 안내도 후속 예외에 가려졌다.

백업은 남아 있으므로 데이터가 사라진 것은 아니다. 후속 상태 수집 오류를 별도로 기록하고, 기록 디렉터리에 쓰기가 가능한 경우 원래 오류와 백업 위치를 보존하면서 실패 기록을 저장해야 한다. 복구 검증을 완화해 불명확한 상태를 무조건 덮어쓰는 방식은 피해야 한다.

### 5. [P1] Skill 설치 timeout 이후에도 자식 프로세스가 파일을 변경할 수 있음

상태: 해결 (2026-10-30). CLI와 Git 명령을 별도 프로세스 그룹에서 실행하고, 300초 timeout이나 예외가 발생하면 그룹에 SIGKILL을 보낸 뒤 launcher의 종료를 최대 5초간 기다린다. 출력 pipe도 닫은 뒤 실패 상태를 수집하므로 launcher가 먼저 종료되거나 자식이 SIGTERM을 무시해도 같은 그룹의 프로세스가 계속 파일을 쓰지 못한다. [Installer 테스트](../scripts/tests/test_install_agent_skills.py) 58개가 통과했으며, 실제 합성 자식 프로세스에서 수정 전 timeout 이후 쓰기를 재현하고 수정 후 timeout, 중단, pipe 오류 뒤의 쓰기가 없는 것을 검증했다. 별도 session이나 process group으로 이탈하는 프로세스, 실제 npm timeout, Linux 실행은 검증 범위 밖이다.

근거: [install-agent-skills.py](../scripts/install-agent-skills.py)의 89행.

`subprocess.run(..., timeout=300)`은 직접 실행한 프로세스의 timeout을 관리하지만, CLI가 생성한 자식 프로세스 전체의 종료를 보장하지 않는다. 임시 launcher가 생성한 자식은 설치 실패가 보고된 뒤에도 파일을 썼다. 재현에서는 timeout만 짧게 바꾸고 실제 `run()` 함수를 사용했다.

복구용 상태를 기록하고 잠금을 해제한 뒤에도 이전 작업이 계속될 수 있다. 프로세스 그룹 전체를 종료하고 종료를 확인한 다음 실패 상태를 수집해야 한다. 실제 pinned npm CLI의 timeout은 발생시키지 않았다.

### 6. [P1] 오래 걸리는 세션 업로드가 살아 있는 작업자의 잠금을 잃음

상태: 해결 (2026-10-30). 살아 있는 PID와 권한 때문에 확인할 수 없는 PID는 시간만으로 만료시키지 않는다. 잠금마다 소유권 token을 기록하고, 디렉터리 변경을 짧은 file lock으로 직렬화해 오래된 잠금을 회수하는 작업자끼리 새 잠금을 삭제하지 못하도록 했다. 원격 업로드는 전체 300초 deadline을 공유하고, timeout이면 로컬 전송 프로세스 그룹을 종료한 뒤 queue를 보존한다. [잠금과 전송 테스트](../modules/agent-session-record/tests/test_upload_worker.py)와 [세션 기록 테스트](../modules/agent-session-record/tests/test_agent_session_record.py)에서 살아 있는 오래된 잠금, 교체된 소유자의 잠금 보존, 동시 접근, 자식 프로세스 종료, timeout 뒤 재시도를 검증했다. 실제 SSH 장애와 Linux 실행은 검증하지 않았으며, 로컬 프로세스 종료가 이미 수행된 원격 쓰기를 되돌리지는 않는다.

근거: [agent_session_upload_worker.py](../modules/agent-session-record/files/agent_session_upload_worker.py)의 190행, 187행, 882행.

잠금 소유 프로세스가 살아 있어도 300초가 지나면 잠금을 오래된 것으로 판정한다. 반면 전송 실행에는 전체 작업 timeout이 없다. 현재 프로세스 PID를 넣은 301초 된 잠금도 만료되는 것을 확인했다. 이전 작업자는 종료 시 자신이 만든 잠금인지 확인하지 않고 현재 잠금 디렉터리를 제거한다.

새 작업자가 잠금을 얻은 뒤 이전 전송이 재개되면 같은 기록을 경쟁적으로 변경할 수 있다. 전송 시간을 제한하고, 살아 있는 소유자의 잠금을 시간만으로 제거하지 않으며, 해제 시 소유권을 확인해야 한다. 실제 네트워크 정지 후 덮어쓰기까지는 재현하지 않았다.

### 7. [P2] 세션 식별 기준이 달라 다른 subagent 기록을 버릴 수 있음

근거: [agent_session_provider.py](../modules/agent-session-record/files/agent_session_provider.py)의 205행, [agent_session_upload_worker.py](../modules/agent-session-record/files/agent_session_upload_worker.py)의 251행과 913행.

Provider는 Claude child를 `parent_session_id:agent_id`로 구분하고 서로 다른 `run_id`를 부여하지만, 상태 파일과 잠금은 bare `agent_id`를 사용한다. 서로 다른 부모가 같은 child ID를 쓰는 합성 데이터에서 `run_id`는 두 개였지만 archive는 하나만 남고 queue는 비었다. 수정 시각이 더 이른 다른 child를 이미 대체된 기록으로 판정해 삭제했다.

상태, 잠금, 중복 제거를 `(scope, provider, run_id)`로 통일해야 한다. 기존 상태와 queue를 읽는 경로도 함께 검토해야 한다. 실제 provider의 ID 재사용 빈도는 확인하지 않았다.

### 8. [P2] Codex 시작 시 전체 기록을 반복해서 읽고 파일 하나의 오류가 queue 재시도도 중단함

근거: [agent_session_provider.py](../modules/agent-session-record/files/agent_session_provider.py)의 70행, 104행, 390행, [agent_session_upload_worker.py](../modules/agent-session-record/files/agent_session_upload_worker.py)의 1118행과 1150행.

Metadata 탐색 함수는 transcript 전체를 리스트로 읽는다. 두 번째 줄에 필요한 model이 있어도 5,002개 레코드를 대상으로 5,004번 JSON을 해석했다. 시작 시 과거 기록 전체에서 이 작업을 수행하고, 중복 판정 전에 각 파일 전체를 해시한다. 동시 시작은 독립 작업자에서 같은 처리를 반복한다.

Discovery는 capture별 예외 처리 밖에서 실행된다. 정상 metadata 뒤에 불완전한 UTF-8 바이트가 있는 transcript를 넣자 `UnicodeDecodeError`로 중단됐고, 호출 구조상 뒤의 queue 재시도에도 도달하지 못한다.

Metadata 탐색을 streaming으로 바꾸고, 파일별 실패를 격리하며, 변경되지 않은 파일을 비싼 처리 전에 제외해야 한다. 동시에 시작하는 복구 탐색은 중복 실행을 줄이되, 명시적인 전체 replay 기능은 보존한다. 운영 환경의 기록 크기, 메모리 사용량, 지연 시간은 측정하지 않았다.

### 9. [P2] 테마 생성기의 입력 검증이 앱마다 다름

상태: 해결 (2026-10-30). Neovim과 Zellij loader가 기존 override validator를 호출하고, canonical theme의 ID와 variant가 일치하는지 확인한 뒤 반환한다. [생성기 회귀 테스트](../themes/test_generate.py)에서 잘못된 속성, component, player, app, theme, variant를 거부하고 모든 기존 export를 보존하는 것을 확인했다. Headless Neovim에서 기존 6개 테마를 모두 로드했다. Zellij 실행 파일은 없어 실제 앱 로드는 검증하지 않았다.

근거: [generate.py](../themes/generate.py)의 580행과 587행.

Zellij와 Neovim override loader는 YAML을 바로 반환하지만 다른 loader는 validator를 호출한다. 잘못된 Neovim highlight 속성 `made_up_attribute`는 validator에서는 거부됐지만 생성은 성공했고, headless Neovim에서 `invalid key` 오류가 발생했다. 해당 경로에서는 override의 theme 및 variant와 canonical theme의 일치 여부도 확인하지 않는다.

기존 validator와 theme identity 검사를 각 loader의 반환 조건으로 만들어야 한다. Exporter 전체를 다시 설계할 필요는 없다. 현재 저장된 테마들은 기존 검사에 통과했고, 실패는 지원하는 override 작성 경로에서 재현했다.

### 10. [P2] 제공된 Ghostty override 템플릿이 잘못된 설정을 생성함

상태: 해결 (2026-10-30). Ghostty palette 항목의 index와 색상을 분리하고 색상 부분만 기존 resolver로 해석한다. [Ghostty 회귀 테스트](../themes/test_ghostty_generator.py)에서 제공된 템플릿의 role 참조, palette 참조, 직접 지정한 hex 색상과 index 보존을 검증했다. 존재하지 않는 참조는 생성에 실패한다. 실제 Ghostty의 `+validate-config`에서도 템플릿으로 생성한 설정을 검증했다. 관련 테마 테스트 29개와 canonical, override, template 검사가 통과했고, 전체 export 54개를 재생성한 결과 기존 내용과 동일했다.

근거: [Ghostty override 템플릿](../themes/overrides/ghostty/TEMPLATE.yaml)의 10행과 [generate.py](../themes/generate.py)의 845행.

템플릿의 `0={roles.ansi.black}`은 검증을 통과하지만 resolver는 문자열 전체가 참조일 때만 값을 치환한다. 유효한 theme metadata로 템플릿을 복사한 재현에서 출력은 `palette = 0={roles.ansi.black}`이었다.

Palette의 index와 색상 부분을 분리해 색상만 해석하면 된다. 이 형식을 위해 범용 문자열 템플릿 계층을 추가할 필요는 없다.

### 11. [P2] 일부 패키지 updater가 macOS 기본 도구에서 실행되지 않음

근거: [Agent Browser updater](../packages/apps/agent-browser/update.sh)의 24행과 48행, [Claude Code updater](../packages/apps/claude-code/update.sh), [Codex updater](../packages/apps/codex/update.sh), [CodexBar updater](../packages/apps/codexbar/update.sh), [Pantry updater](../packages/apps/pantry/update.sh).

이 updater들은 Bash 4 이상의 associative array를 요구하지만 리뷰 환경의 `bash`는 macOS 기본 3.2로 해석된다. Agent Browser, Claude Code, Codex는 GNU 방식의 `sed -i`도 사용한다. 임시 파일과 명령으로 Bash의 `declare -A` 실패와 BSD sed의 `-i` 실패를 확인했다. 실제 release를 조회하거나 패키지를 갱신하지 않았다.

이미 있는 Notion CLI, Pi, Herdr의 portable updater 패턴에 맞추거나 지원하는 실행 환경을 명시해야 한다. 범용 updater 프레임워크를 추가할 이유는 없다.

### 12. [P2] Skill 내용이 같아도 Nix store 경로가 바뀌면 설치를 반복함

근거: [install-agent-skills.py](../scripts/install-agent-skills.py)의 502행, [skill_sources.py](../agent-core/src/agent_core/skill_sources.py)의 152행, [agent-core package](../agent-core/nix/package.nix)의 11행.

Cache key에는 절대 source 경로를 포함한 실행 plan 전체가 들어간다. 동일한 skill 내용을 다른 경로로 옮기는 재현에서 CLI 재실행과 새 백업이 발생했다. Agent-core package가 전체 source를 포함하므로 지침이나 테스트 변경으로 store 경로만 달라져도 영향을 받는다.

실제 실행 경로와 내용, revision, 선택 목록에 기반한 cache identity를 구분해야 한다. 파일 내용과 권한, source ID, CLI version 등 설치 결과에 영향을 주는 검사는 유지한다. 재현을 위해 Nix package를 다시 빌드하지는 않았다.

### 13. [P2] 중요한 테스트가 자동 검증 경로에 연결되지 않음

근거: [App Packages CI](../.github/workflows/apps-ci.yml)의 5행과 44행, [agent-core package](../agent-core/nix/package.nix)의 11행, [flake.nix](../flake.nix)의 `checks` 정의.

현재 workflow는 패키지 중심으로 실행되며, agent-core package의 테스트 범위에는 `scripts/tests`의 실제 skill installer 테스트가 없다. Root render check는 activation shell fixture를 검사하지만 installer 자체 실행을 대체한다. 테마와 runtime 테스트도 현재 CI 경로에 연결되지 않는다.

파일과 설치 상태를 변경하는 코드가 회귀해도 package CI는 통과할 수 있다. 기존 offline 테스트를 실행하고 관련 경로 변경에 반응하는 workflow를 추가하는 편이 우선이다. 테스트를 연결하기 위해 순수 renderer와 사용자 파일을 변경하는 installer를 합칠 필요는 없다.

### 14. [P2] Claude 동시 세션에 다른 저장소의 branch가 표시됨

근거: [statusline-command.sh](../modules/claude/files/statusline-command.sh)의 86행.

모든 작업 디렉터리가 `/tmp/claude-statusline-git-cache` 하나를 공유하고 cache의 나이만 확인한다. 서로 다른 branch의 임시 저장소에서 두 번째 세션이 첫 번째 branch를 표시하는 것을 재현했다. 실제 사용자 cache는 변경하지 않았다.

Private cache를 canonical 작업 디렉터리별로 구분하거나, 직접 Git 조회 비용을 확인한 뒤 cache 자체를 제거하면 된다.

## 다시 결정해야 할 기록 정책

[Redaction 실패 처리](../modules/agent-session-record/files/agent_session_upload_worker.py)의 938행은 비밀 정보 제거에 실패하면 원본 transcript를 그대로 archive한다. 이는 `test_redaction_failure_archives_raw_but_marks_ineligible`로 고정된 의도된 동작이다. `eligible_for_derivation=false`는 이후 활용을 막을 뿐 원본 전송과 저장을 막지 않는다. 잘못된 JSONL 한 줄 때문에 transcript 전체가 원본으로 저장될 수 있다.

[Root README](../README.md)의 세션 기록 설명은 전송 전에 비밀 정보와 개인 정보를 가린다는 설명과 검사 실패 snapshot도 보관한다는 설명을 함께 포함하지만, 실패 시 원본 바이트를 보관한다는 점은 명시하지 않는다. Devsisters profile에서는 capture를 끄며, 다른 환경의 기본 scope는 `personal`이고 archive 파일에는 제한된 권한을 사용한다.

실패한 원본을 로컬에 격리하고 안정된 입력으로 재시도할지, 원격 archive에도 원본 비밀 정보가 저장될 수 있음을 명시적으로 받아들일지 결정해야 한다. 이 항목은 우발적인 parser 결함으로 분류하지 않는다.

## 권장하는 리팩토링 범위

| 대상 | 권장 경계 | 완료 조건 |
| --- | --- | --- |
| Skill installer | 프로세스 종료, 설치 후 상태 수집, 실패 기록을 일관된 복구 절차로 관리 | Timeout과 손상된 lock에서도 후속 쓰기가 없고 복구 가능 |
| Session recorder | 하나의 실행 식별자, 안정된 snapshot, 제한된 전송 작업 | 다른 실행을 중복으로 버리지 않고 파일 하나의 실패가 queue 전체를 막지 않음 |
| Theme generator | Override loader가 검증된 문서만 반환 | 허용된 템플릿은 유효한 출력을 생성하고 잘못된 입력은 출력 전에 거부 |
| Activation | Home Manager의 실행 순서와 dry-run 계약 사용 | 첫 설치, 재적용, dry-run, 복사 실패를 임시 파일로 검증 |

Skill installer는 공개 호출 형태를 유지하면서 `run()`이 자식 프로세스의 종료까지 책임지고, 설치 함수가 수집 실패에도 복구 기록을 남기도록 바꾸는 것이 우선이다. 파일 길이를 기준으로 여러 모듈로 나누는 것만으로는 확인한 문제가 해결되지 않는다.

Session recorder의 상태 조회와 잠금 호출은 현재 `session_id` 대신 이미 할당한 `run_id`를 사용하도록 맞춘다. Provider는 외부 transcript 형식 해석을 계속 담당하고, 공통 queue가 실행 식별자와 전송 상태를 관리한다. 기존 상태 및 queue의 전환을 포함해 같은 child ID를 가진 서로 다른 부모와 오래 걸리는 전송을 검증해야 한다. 정리만을 위해 daemon이나 database를 도입할 필요는 없다.

Theme generator에서는 호출자가 별도 validator 실행 여부를 알아야 하는 현재 상태를 없애고, 기존 `load_*_override()`가 검증과 identity 확인을 끝낸 문서만 반환하도록 한다. 지원하는 앱별 출력 차이는 exporter에 유지한다.

수정 순서는 접근 범위와 activation의 파일 보호, skill 설치 복구 및 세션 잠금, 세션 식별과 탐색 처리, 테마 입력 검증, updater 실행 환경과 cache identity 순서를 권장한다. 각 수정의 회귀 테스트를 추가하고 기존 offline 테스트의 CI 연결도 함께 진행한다.

## 유지할 구조와 보류할 정리

- `inventory`, Home Manager 및 NixOS builder, profile, module의 구분은 유지한다.
- 순수한 agent-core renderer와 실제 사용자 파일을 변경하는 installer는 분리한다.
- 테마의 canonical source, template, override, export 구분은 유지한다.
- Codex, Claude, Pi의 lifecycle 처리는 분리한다. 작은 I/O 기능은 공유할 수 있지만 상태 전이까지 합치면 runtime 차이를 숨기게 된다. Pi를 transcript watcher로 바꾸거나 tmux와 Herdr 표시를 합칠 근거는 없다.
- 문서에 정한 host 검증이 끝나기 전에는 legacy renderer와 배포 경로를 삭제하지 않는다. 특히 [Sepia README](../hosts/pylv-sepia/README.md)는 Menu release와 rollback 검증 전까지 이전 배포 경로를 유지하도록 명시한다.
- 큰 theme generator나 여러 package updater가 존재한다는 사실만으로 범용 framework를 만들지 않는다. 확인한 입력 계약과 실행 환경 문제부터 수정한다.

## 검증 결과

기존 테스트 248개가 통과했다. 별도의 임시 재현으로 위 실패 조건을 확인했으므로, 기존 테스트 통과만으로 해당 경로가 검증됐다고 볼 수는 없다.

| 검증 범위 | 결과 |
| --- | --- |
| Agent-core 및 skill installer Python 테스트 | 76개 통과 |
| Session recorder Python 테스트 | 27개 통과 |
| Herdr, Codex, Claude, Pi Bun 테스트 | 57개 통과 |
| Tmux 상태 Python 테스트 | 4개 통과 |
| Theme Python 테스트 | 26개 통과 |
| Zsh Python 및 Bun 테스트 | 각각 19개, 14개 통과 |
| Darwin agenix runner 테스트 | 7개 통과 |
| Menu deploy 테스트 | 8개 통과 |
| Launchd backport 테스트 | 10개 통과 |
| Package fixture | `detect-changed-apps`, `update-all`, `check-nix-patches`, `notion-cli-update`, `pi-update`, `codex-check-package` 통과 |
| 추가 shell 검사 | Tmux session manager와 Pi settings 동기화 통과 |
| Theme 검사 | Canonical 및 override validator, template 검사 통과 |
| 문서 검사 | 기본 entry-point 문서의 경로 검사 통과 |
| Nix 평가 | Offline으로 pinned Home Manager source와 Mac activation 실행 순서 확인 |

Python 검사는 `uv run`으로 실행했다. 파일을 변경하는 재현은 임시 디렉터리와 합성 입력을 사용했으며, 실제 설치된 skill, 세션 기록, 사용자 cache, 비밀 정보는 사용하지 않았다. 재현용 코드를 repository에 영구 테스트로 추가하지는 않았다. 리뷰 후 작업 트리에는 기존 `modules/claude/files/settings.json` 수정만 남았다.

검증하지 않은 범위는 전체 Nix 빌드, 실제 switch와 배포 및 로그인, 실서버 접근 범위, 실제 전송 장애, 모든 에디터 기능과 외부 plugin 내부다. 합성 데이터에서 확인한 실패의 실제 발생 빈도와 운영 환경 영향은 아직 미확인이다.
