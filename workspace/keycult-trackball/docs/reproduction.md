# 재생성·검증

명령은 저장소 루트에서 실행한다. 모든 스크립트는 자신의 파일 위치를 기준으로 경로를 찾으며 개인 작업공간 절대 경로를 요구하지 않는다.

## 1. 좌표·벡터·HTML

```bash
uv run design/preliminary/build_artifacts.py
```

Python 표준 라이브러리만 필요하다. 입력은 생성기 안의 배열/상수 및 `sources/keyball-evidence.json`이다. 공식 CAD나 상세 윤곽 좌표 없이 공개 도면을 생성한다. 초록색 사각형은 285.75 × 108.25mm 전체 치수 참고 경계이며 실제 윤곽이나 케이스 개구부가 아니다. 출력은 `layout.json`, `validation.json`, 평면/대안/단면/인쇄 SVG 및 HTML이다. **기존 PNG/PDF나 플레이트 포함 결과는 이 명령만으로 새로 계산되지 않는다.**

설계를 바꿀 때 배열과 치수 상수를 먼저 편집한다. JSON은 모든 키 중심을 다른 CAD 도구로 가져가기 위한 교환 파일이지만 이 생성기의 입력은 아니므로 JSON에만 한 수정은 재생성 시 덮어쓴다. SVG는 벡터 편집기로 열 수 있지만 그 수정도 생성기에는 역반영되지 않는다.

`layout.json`은 65키 기준안, 14u 64키 비교안과 `rear_clearance` 62키 옵션의 좌표를 포함한다. 62키 옵션은 `rear-clearance.svg`와 `rear-clearance-print.svg`/`.html`로 생성하며, 볼 지름 34mm와 나머지 키를 유지하고 상단 중앙 Vol-/Mute/Vol+만 제거한다. 중앙 57.15 × 76.20mm 빈 명목 영역은 가공 구멍이나 확대된 홀더 예약 영역이 아니다.

## 2. PNG·1:1 PDF

시스템 Chromium과 [uv](https://docs.astral.sh/uv/)가 필요하다. 스크립트 내 PEP 723 메타데이터로 Playwright 1.56.0을 선언했다. 브라우저를 추가 다운로드하지 않고 시스템 Chromium을 사용한다.

```bash
uv run scripts/render.py
```

`chromium` 또는 `chromium-browser`가 PATH에 없으면 `KEYCULT_CHROMIUM=/절대/경로/chromium`을 지정한다. 명령은 평면 PNG, 단면 PNG, A3 PDF와 62키 옵션의 `rear-clearance.png`, `rear-clearance-print.pdf`를 다시 만든다. SVG/HTML에 외부 폰트·스크립트·이미지 요청은 없다. PNG 렌더는 브라우저/설치 폰트에 따라 작은 차이가 날 수 있다.

## 3. 독립 검증

```bash
uv run scripts/validate.py
```

Shapely 2.1.2와 PyMuPDF 1.26.7을 사용한다. 다음을 검사한다.

- Python AST, JSON, SVG/XML, HTML 파싱과 문서/HTML 로컬 링크.
- 65/64/62키 개수, 중복 좌표, 19.05mm 기본 피치와 하단 1.5u 두 키의 중심, 중앙 정렬, 문자 키, 키캡 쌍 충돌. 62키 옵션은 기준안에서 상단 중앙 세 키만 제거했는지도 확인.
- 원–사각형 거리로 볼 간섭을 다시 계산하고 저장된 결과와 비교.
- 임시 50mm 영역의 양수/음수 간격 및 일반적인 사각 치수 경계에 대한 Shapely 포함·최소 거리. 결과는 `envelope-containment.json`에 저장한다. 공개 SVG에 상세 윤곽이 아닌 사각 치수 경계가 있는지도 확인한다.
- 미확정 케이스 개구부·깊이·홀더 치수가 JSON에서 계속 `null`인지 확인.
- 기준안과 62키 옵션의 PDF가 각각 한 페이지 A3 가로인지, 벡터상의 100mm 기준선 길이가 맞는지 확인. 결과는 `print-validation.json`과 `rear-clearance-print-validation.json`에 기록.

공개 검증은 공식 윤곽을 사용하지 않으며 실제 윤곽 포함을 증명하지 않는다. `plate-containment.json`은 초기 66/64키 공식 윤곽 조사에서 얻은 스칼라 결과 기록이며 이 명령으로 재계산하지 않고 현재 65키/62키 배열도 다루지 않는다. 상세 윤곽 재현은 아래 로컬 전용 절차로 수행한다. 이 검증은 PCB DRC, 실제 케이스 개구부, 3D 수납, 소켓·다이오드·배선·하중 검증을 대체하지 않는다.

종이 출력은 **A3 가로 / 100% 실제 크기 / 페이지 맞춤 끄기**로 하고 100mm 선을 자로 확인한다. PDF 내부 길이가 정확해도 프린터 배율까지 보증하지는 않는다.

## 4. 원본 자료 취득과 측정 재확인

원본 STEP/DXF/Gerber는 저장소에 포함하지 않는다. 먼저 [라이선스·사용 조건](alternatives-and-licenses.md)을 확인한다. 아래 명령은 고정 출처 세 ZIP을 git-ignored 디렉터리 `design/preliminary/sources/upstream/`에만 저장하고, 조사 당시 SHA-256과 일치해야 성공한다. 자동 압축 해제나 외부 파일 실행은 하지 않는다.

```bash
uv run scripts/fetch_sources.py
```

[원본 해시와 측정 방법](../design/preliminary/sources/README.md)에 개별 파일 체크섬도 있다. 원본이 바뀌면 해시 실패를 무시하지 말고 새 리비전으로 별도 검토한다.

### 로컬 전용 공식 윤곽 재검증

```bash
uv run scripts/verify_official_contour.py
```

ezdxf 1.4.3와 Shapely 2.1.2를 사용한다. 로컬 ZIP 및 내부 DXF의 고정 SHA-256을 재확인하고, 가장 큰 유효 폐곡선에서 공식 외곽을 복원한다. 키캡/스위치 포함·최소 거리를 재계산해 초기 `plate-containment.json` 수치와 비교한다. 배치 변경 후 차이가 생기면 결과를 검토한 뒤 초기 보고서를 의도적으로 갱신해야 한다.

상세 좌표 `measured-official-dxf.json`, 현재 포함 결과 `plate-containment.json`, 정밀 오버레이 `layout-overlay.svg`는 모두 **git-ignored `design/preliminary/sources/local/`**에만 생성한다. 공개 PNG/PDF는 수정하지 않는다. 원본 윤곽이 들어 있는 로컬 파생물은 공개용 산출물과 구분하며, 배포 조건은 [대안·라이선스 문서](alternatives-and-licenses.md)를 따른다.

초기 측정은 ezdxf와 CadQuery/OpenCascade를 사용했다. 재확인 시:

- DXF: `$INSUNITS=4`; 바깥 폐곡선을 `ezdxf.path.make_path(...).flattening(0.01)`로 샘플링한다. `x=-x_native-142.875`, `y=y_native-47.625`로 변환한다. 네이티브 bulge 꼭짓점도 로컬 전용 `sources/local/measured-official-dxf.json`에 생성한다.
- STEP: `cadquery.importers.importStep(path).val().BoundingBox()`로 x/z 평면과 y 두께를 읽는다. 원본 및 Rev.1 3종 결과는 `measured-official-step.json`에 있다.
- Gerber: 고정 Keyball46 센서 `ball_sensor-Edge_Cuts.gbr`는 mm/4.6 좌표 형식이며 x=100…122, y=−100…−75mm이다. 현재 판매 부품 전체 크기를 뜻하지 않는다.

이 윤곽 검사는 원본 CAD 측정 전체를 자동화하지 않는다. 아래 FreeCAD 초안은 공식 STEP에 보강판과 중앙 프레임을 구성하며, 실물 홀더의 높이와 체결부는 아직 검증하지 않는다.


## FreeCAD 보강판 초안

macOS에서 설치된 FreeCAD 1.1.4의 Python 3.11과 OpenCascade를 사용한다. `uv`는 기존에 사용하던 `ezdxf==1.4.3`을 별도 환경에 준비하며 FreeCAD 앱 자체를 수정하지 않는다. 저장소 루트에서 실행한다.

```bash
uv run scripts/fetch_sources.py
uv run scripts/verify_official_contour.py
uv run --python /Applications/FreeCAD.app/Contents/Resources/bin/python scripts/build_plate.py
```

설치 위치가 다르면 `FREECAD_ROOT`를 `Contents/Resources` 경로로 지정하고 `--python`도 해당 앱의 Python으로 바꾼다. Linux 실행 환경에서는 FreeCAD와 Python 바인딩 경로를 별도로 확인해야 하며 이 절차를 Linux에서 시험하지는 않았다. 생성된 FCStd는 FreeCAD 앱에서 열고 객체의 표시 여부, 색상과 투명도를 직접 조정한다. PNG가 필요하면 GUI에서 저장한다. 외부 Python에서 `FreeCADGui.showMainWindow()`를 호출하는 방식은 이 Mac에서 종료 시 비정상 종료가 발생하여 사용하지 않는다.

결과는 Git에서 제외되는 `design/preliminary/sources/local/plate-study/` 아래에 생성된다. `65/`와 `62/`는 각각 65키와 62키 결과를 저장한다. 각 디렉터리에 `plate-<keys>.FCStd`, `plate-<keys>.step`, `carrier-<keys>.step`, `plate-<keys>-DRAFT.dxf`, `plan.svg`, `checks.json`을 둔다. 재생성은 이 초안을 덮어쓰므로 수동 CAD 변경은 별도 이름으로 먼저 저장한다. 공개 저장소에는 생성 스크립트와 형상을 복제하지 않는 설명만 둔다.

배열 원본은 기존 `design/preliminary/build_artifacts.py`이고, 새 기구 검토 치수는 `scripts/build_plate.py`의 `STUDY`다. FCStd의 `Study` 속성은 사용한 값을 기록한 것으로 형상을 자동 구동하지 않는다. `Plate`는 원본 외곽의 판과 검토용 절삭 객체를 연결한 Boolean cut이며, 재현 가능한 치수 변경은 생성기에 반영한다. 생성 검사는 원본 해시, PCB 좌표 및 CAD 해시, 단일 솔리드, 구멍 수, 외곽 보존, STEP/FCStd/DXF 재열기를 확인한다. `review.json`의 PCB-볼 간섭은 알려진 미해결 항목이며, 스크립트 성공을 실물 조립 통과로 해석하지 않는다.

## 홀더 받침 CAD 실험

이 실험은 생산 홀더 CAD 없이 가정한 외피와 높이를 비교한다. [조건부 결과와 미확정 항목](../design/preliminary/design.md#순정-홀더를-받치는-브래킷의-조건부-cad-실험)을 먼저 읽는다. 설치된 macOS FreeCAD의 Python으로 실행한다.

```bash
uv run scripts/fetch_sources.py
uv run --python /Applications/FreeCAD.app/Contents/Resources/bin/python scripts/build_holder_study.py
```

생성기는 필요한 Keyball44 호스트 원본 ZIP이 없으면 고정 커밋에서 받아 SHA-256을 확인하고, 있으면 해시를 대조한다. DXF 단위 헤더의 불일치는 KiCad 장착점 7개와 대조해 기록한다. 현재 배열과 두 PCB의 stem 좌표, 사각 `Edge.Cuts`를 직접 읽으며 실행 중 입력 파일이 바뀌면 실패한다. 기존 보강판 도구의 공식 외곽 추출 함수를 사용하지만 기존 보강판 초안과 KiCad 파일은 덮어쓰지 않는다.

Git에서 제외하는 `design/preliminary/sources/local/holder-study/65/`와 `62/`에 기본 시나리오의 `holder-study.FCStd`, `Basket-EXPERIMENT.step`, `Plate-EXPERIMENT.step`, `ProposedOpenPCB-EXPERIMENT.step`, `section.svg`를 생성한다. FCStd는 FreeCAD 앱에서 열어 검토하며 표시 설정과 PNG 저장은 GUI에서 직접 수행한다. 상위 `review.json`에는 높이와 외피 크기를 바꾼 18개 비교 결과, 입력 해시와 가정을 기록한다. 재생성은 이 실험 파일을 덮어쓰므로 수동 편집은 다른 이름으로 먼저 저장한다. FreeCAD의 `Assumptions` 속성은 기록용이며 형상을 구동하지 않는다. 가정 변경은 생성기에서 수행한다.

생성기는 브래킷 단일 솔리드, 공식 보강판 외곽 보존, 플랜지 접촉면, 구성품별 현재/개구 PCB 간섭, 키 영역 및 체결부 간섭, STEP/FCStd 재열기를 검사한다. 볼과 센서의 실제 광학 간격, 케이스 공간 및 강성은 이 검사에 포함되지 않는다. 나사산과 실제 커넥터 결합 형상을 모델링하지 않았으므로 단순 형상 사이 간섭이 없어도 체결·조립이 검증된 것은 아니다.

## KiCad 65키·62키 회로·PCB 초안

[PCB 재현 안내](../design/pcb/README.md#reproduction)를 별도로 따른다. 키 배열을 변경하면 예비 도면 생성기와 PNG/PDF 렌더를 먼저 실행한다. Python은 설치된 KiCad 10.0.7의 `pcbnew` 바인딩과 버전이 일치해야 한다. 기존 Nix 환경은 Python 3.14, 아래 공식 Docker 환경은 Python 3.13을 사용한다. 65키 원본은 `design/pcb/keycult65.*`, 62키 원본은 `design/pcb/62/keycult62.*`이며, 각 디렉터리에 BOM·핀맵·배선과 `reports/`, `exports/`를 둔다.

아래 명령은 저장된 배선을 재적용한 뒤 실제 ERC/DRC, netlist, 키 좌표와 예약 영역을 검사하고 SVG/PDF를 내보낸다. 생성기는 현재 CAD를 덮어쓰므로 수동 편집을 보존한 뒤 실행한다. `--keys`를 생략하면 65키를 선택한다.

```bash
uv run --python "$(command -v python3)" scripts/build_pcb.py --keys 65
uv run --python "$(command -v python3)" scripts/route_pcb.py --keys 65 --import-saved
uv run --python "$(command -v python3)" scripts/validate_pcb.py --keys 65
uv run --python "$(command -v python3)" scripts/build_pcb.py --keys 62
uv run --python "$(command -v python3)" scripts/route_pcb.py --keys 62 --import-saved
uv run --python "$(command -v python3)" scripts/validate_pcb.py --keys 62
```

각 SES는 현재 키 좌표에 맞춰 다시 생성한 기본 배선을 저장한다. `scripts/usb_routing.py`는 seed 생성과 SES 가져오기 이후 J1 측 USB 배선을 교체하고 DRAFT E의 접지 비아·접지면 및 후보 stackup을 적용한다. `route-manifest.json`은 회로, 배열, 생성기와 이 스크립트의 해시를 검사하므로 입력을 변경하면 기존 배선을 그대로 가져올 수 없다. 회로나 배열을 변경하면 `--import-saved` 대신 `--route`로 다시 배선하고 전체 결과를 검토한다.

검증은 기본적으로 미배선이나 DRC 위반이 있으면 실패한다. `--draft`는 미배선만 허용하고 실제 수를 보고하는 검토 모드이며, 기하 DRC 위반은 그대로 실패한다. 새로 채운 In1.Cu 접지면이 F.Cu USB 배선 아래를 덮는지 검사하며, 신호 비아 중심 반경 0.51mm는 접지면의 절연 간격(antipad) 때문에 별도로 제외한다. 선폭 0.29mm, 최소 간격 0.2mm와 신호 비아에서 접지 비아까지 3mm 이내라는 설계 조건도 검사한다. 이 수치는 프로젝트의 배선 검토 조건이며 USB 적합성 판정 기준이 아니다. 검사 상태는 [65키 보고서](../design/pcb/reports/validation.json)와 [62키 보고서](../design/pcb/62/reports/validation.json)를 기준으로 하며 제조 승인으로 해석하지 않는다. KiCad 없이 실행하는 검사 회귀 테스트는 `uv run scripts/test_pcb_validation_gate.py`다.

### 전원과 USB의 실물 없는 검토

실물 없이 수행하는 전원·USB 계산은 아래 명령으로 재현한다. 기존 `sexpdata` 의존성만 사용하며 KiCad 실행 파일은 필요 없다. `design/pcb/reports/electrical-review.json`에 조건별 전압 강하·발열, 후보 stackup의 근사값과 두 native PCB에서 읽은 USB 층별 길이·비아 좌표를 기록한다. 입력 CAD 해시를 함께 저장하고, 계산 대상 부품 번호가 달라지거나 필요한 USB net의 배선이 없으면 실패한다. 테스트는 USB 배선 교체를 반복해도 결과가 같고 기존 MCU 측 배선을 보존하는지도 확인한다. 이 보고서는 ERC/DRC나 실물 신호 품질 통과 보고서가 아니다.

```bash
uv run scripts/analyze_electrical.py
uv run scripts/test_electrical_analysis.py
```

### Docker에서 KiCad 검증

호스트에 KiCad가 없으면 공식 `kicad/kicad:10.0.7`의 Linux amd64 이미지를 사용한다. Docker 실행 환경이 필요하며 Apple Silicon에서는 amd64 에뮬레이션을 사용한다. 아래 이미지는 기존 스크립트 의존성을 KiCad의 Python 3.13 가상 환경에 설치한다. 3D 모델과 Java는 포함하지 않으므로 저장된 SES 배선 재적용과 회로 검증에 사용하고, 새 자동 배선에는 별도의 Java/Freerouting 환경이 필요하다.

```bash
docker build --platform linux/amd64 -t keycult-kicad-runtime:10.0.7 - <<'DOCKERFILE'
FROM kicad/kicad:10.0.7@sha256:3224d1bf143185ecc1bd3a1082a36f37f0f4e59d707983a16515a845e3737fc6
USER root
COPY --from=ghcr.io/astral-sh/uv:0.9.13 /uv /usr/local/bin/uv
RUN uv venv --python /usr/bin/python3 --system-site-packages /opt/keycult-venv && uv pip install --python /opt/keycult-venv/bin/python sexpdata==1.0.2 shapely==2.1.2 pymupdf==1.26.7
ENV PATH="/opt/keycult-venv/bin:${PATH}"
DOCKERFILE

docker run --rm --platform linux/amd64 --entrypoint /bin/sh \
  -v "$PWD:/work" -w /work \
  -e KICAD10_SYMBOL_DIR=/usr/share/kicad/symbols \
  -e KICAD10_FOOTPRINT_DIR=/usr/share/kicad/footprints \
  -e PYTHONPATH=/usr/lib/python3/dist-packages \
  -e VIRTUAL_ENV=/opt/keycult-venv \
  keycult-kicad-runtime:10.0.7 -c '
    uv run --no-project --active python scripts/build_pcb.py --keys 65 &&
    uv run --no-project --active python scripts/route_pcb.py --keys 65 --import-saved &&
    uv run --no-project --active python scripts/validate_pcb.py --keys 65
  '
```

62키는 위 실행 명령의 `--keys 65` 세 곳을 `--keys 62`로 바꾼다. `--no-project --active`는 미리 설치한 가상 환경과 시스템 KiCad 바인딩을 함께 사용하기 위한 옵션이다. USB 배선 및 배선 정리 하위 스크립트는 자체 메타데이터에 따라 `uv` 캐시를 만들 수 있으므로 첫 실행에는 네트워크가 필요하다. DRAFT E의 두 버전은 이 환경에서 생성하고 ERC/DRC와 연결 일치를 검사했다.
