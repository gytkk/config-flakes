# 중앙 트랙볼의 실현 가능성과 대안

확인일: 2026-10-29. 대상은 사용자가 보유한 NovelKeys 최신 판매 버전의 Keycult No.1/60 WK다. 정확한 리비전과 내부 치수는 아직 확인되지 않았다. G/H 사이 중앙 통합은 필수 조건이며, 사용자가 이미 사용 경험이 충분한 외부 트랙볼은 대안에서 제외한다. 작은 볼과 터치패드는 중앙에 통합하는 조건으로 비교한다. 이 문서는 후보 비교이며 대체품 채택을 확정하지 않는다. 34mm Keyball은 65키 기준안과 상단 중앙 세 키를 제거한 별도 62키 옵션으로 검토한다.

## 판단

**34mm Keyball 중앙 통합은 기구 시제품으로 검토할 가치는 있지만, 현재 근거로 PCB와 보강판 제작에 바로 착수할 만큼 확실한 계획은 아니다.** 평면 배치의 가능성은 확인했지만 실제 홀더, 수직 공간, 장착 강성, 사용자 손의 접근을 함께 검증하지 않았다. 다음 판단은 34mm 조립체와 중앙 터치패드를 실제 설치 높이와 접근성으로 비교하는 것이다. 34mm 안의 기구적 어려움만으로 중앙 통합이라는 목적을 완화하지 않는다.

중앙 위치를 유지할 때는 **Cirque의 기존 Pinnacle 기반 23mm 또는 35mm 터치패드가 수직 공간 문제를 줄일 근거가 가장 구체적인 후보**다. 제조사가 부품을 포함한 두께를 공개하며 QMK도 해당 컨트롤러를 명시적으로 지원한다. 다만 같은 TM023023/TM035035 제품군의 신형 Gen6s는 통신 방식이 다르므로, 주문할 전체 부품번호와 컨트롤러를 확인해야 한다. 작은 트랙볼의 완성 조립체 치수를 확보하지 않은 상태에서 볼 지름만 줄여 공간 문제가 해결된다고 판단할 수 없다. [Cirque 문서 목록](https://www.cirque.com/documentation), [QMK 포인팅 장치 문서](https://docs.qmk.fm/features/pointing_device#cirque-trackpad)

## 34mm 안에서 아직 풀지 못한 문제

**보유 케이스와 과거 도면의 일치 여부.** NovelKeys 판매 페이지는 No.1/60 재출시와 부품 선택 구성을 설명하지만, 확인한 설명에서 초기형 및 Rev.1 공개 CAD와의 내부 치수 호환을 명시하지 않는다. 기존 저장소의 285.75 x 108.25mm 플레이트 측정값과 과거 윤곽 검사를 보유 케이스의 장착 검증으로 사용할 수 없다. WK라는 정보는 기록하되 내부 개구부와 체결 기준은 실측해야 한다. [NovelKeys 제품 페이지](https://novelkeys.com/products/keycult-no-1-60-keyboard), [기존 측정 출처](../design/preliminary/sources/README.md)

**볼과 완성 모듈의 크기 차이.** Keyball 공식 가이드는 34mm 볼과 홀더 뒤쪽에 세우는 센서 PCB를 설명한다. 조사한 공개 파일에서 생산 홀더의 독립 CAD나 전체 조립 외곽 치수는 확보하지 못했다. 기존 22 x 25mm 값은 센서 기판에만 해당한다. [고정 리비전의 Keyball 조립 가이드](https://github.com/Yowkees/keyball/blob/e1d1948e6e60a83f6b3566a209326508200d64ce/keyball44/doc/rev1/buildguide_en.md)

비교 사례로 BastardKB의 독립 `34-mm-trackball-holder`는 STEP/STL을 제공하고 반경 21mm, 높이 2mm의 통합용 링을 지정한다. 고정 커밋 `1bb7b1a49d6d5beca8eb48987bdb35460ba3529e`의 상부와 하부 binary STL 꼭짓점 범위를 계산하면 원본 X/Z 방향의 최대 외곽은 약 45.6mm다. 이는 플라스틱 부품만의 치수이며 Keyball 홀더 치수나 완성 모듈의 설치 깊이가 아니다. 저장소명은 34mm지만 README는 35mm라고 적으므로, 해당 부품을 채택하려면 실제 볼 호환도 확인해야 한다. [BastardKB 홀더와 원본 파일](https://github.com/Bastardkb/34-mm-trackball-holder/tree/1bb7b1a49d6d5beca8eb48987bdb35460ba3529e)

**높이를 줄이면 돌출이 커지는 제약.** 플레이트 아래에서 볼에 쓸 수 있는 깊이를 D, 플레이트 위 볼 중심을 c로 두면, 지름 34mm 볼 자체만으로도 `c - 17 >= -D`, 따라서 볼 최상단은 `c + 17 >= 34 - D`다. 예를 들어 D가 10mm라면 볼 위끝은 최소 24mm 위로 올라온다. 이는 설명용 기하 계산이며 실제 케이스 깊이가 10mm라는 뜻이 아니다. 홀더, 센서, 커넥터, 절연과 휨 여유를 넣으면 제약이 더 생긴다. 모듈을 위로 올리는 방법은 수납을 도울 수 있지만 손가락 접근과 타건 간섭을 함께 시험해야 한다.

**볼을 조작할 때의 강성과 접근성.** 기존 Keycult 설명의 플레이트는 가스켓으로 케이스에서 분리된다. 중앙 절개와 모듈 질량을 추가한 뒤에도 같은 타건감과 강성을 유지한다고 가정할 수 없다. 볼과 센서를 같은 강체에 고정한 모듈, 충분한 중앙 보강, 또는 실물의 기존 체결점을 쓰는 브래킷을 검토할 수 있으나 어느 방식도 아직 설계되지 않았다. 기존 3u 중앙안의 G/H 중심 간격은 76.20mm이므로 사용자 손에 맞는지도 별도 시험해야 한다. [Keycult의 기존 장착 구조 설명](https://keycult.com/pages/no-1-60-no-1-65), [현재 장착 검토](../design/preliminary/design.md)

### 34mm를 유지하며 뒤쪽 공간을 확보하는 62키 옵션

15u 65키 기준안에서 상단 중앙 Vol-/Mute/Vol+만 제거한 `rear_clearance` 옵션은 중앙 빈 명목 영역을 57.15 × 57.15mm에서 57.15 × 76.20mm로 늘린다. 볼 중심 `(0, 0)`, G/H 중심 x=±38.10mm와 나머지 키는 유지하며, 추가한 공간은 뒤쪽(-y) 한 행 19.05mm다. [62키 배치도](../design/preliminary/rear-clearance.svg)와 [1:1 A3 인쇄물](../design/preliminary/rear-clearance-print.pdf)로 비교한다.

센서와 배선을 뒤쪽으로 배치하거나, 재료를 남겨 지지 브리지를 만들고 체결점 간격을 넓힐 수 있는지 검토한다. 실제 체결 구조가 가능할 때 얻을 수 있는 선택지이며, 키를 제거했다는 이유만으로 장착이 안정적이라고 판단할 수 없다. 3열 × 4행 전체를 절개하면 지지 재료가 줄어 오히려 강성이 낮아질 수 있다. 볼 지름은 34mm, 임시 조립 예약 영역은 50 × 50mm로 유지한다. 이 옵션은 수직 공간 증가, 실제 홀더 수납 또는 제조 적합성을 증명하지 않으며 과거 66/64키 공식 윤곽 검사도 새 옵션의 검증 근거로 쓰지 않는다.

## 확인된 치수와 구분해야 할 외곽

| 후보 | 제조사 또는 설계자 자료에서 확인한 값 | 확인되지 않은 값과 설치 조건 |
| --- | --- | --- |
| Cirque TM023023, 기존 Pinnacle | 모듈 지름 23.20 +/- 0.25mm, 감지 영역 지름 21.60mm. PCB 0.74mm, PCB와 접착층 0.89mm, PCB와 접착층 및 부품 5.04 +/- 0.320mm. [23mm 사양서, v1.1, p.6](https://www.dropbox.com/scl/fi/0cdhpythfobsarivtsosw/GP-DS-170805-TM023023-SPI-I2C-PINN-Trackpad-Spec.pdf?rlkey=331uxyu8esyjjo20zn1kjw944&dl=0) | 5.04mm는 별도 상부 오버레이와 장착 지지체까지 포함한 완성 설치 높이가 아니다. FFC 케이블의 굴곡 공간도 확보해야 한다. |
| Cirque TM035035, 기존 Pinnacle | 모듈 지름 35.0mm, 감지 영역 지름 31.00mm. PCB 1.00 +/- 0.125mm, PCB와 오버레이 1.25mm, PCB와 오버레이 및 부품 5.40 +/- 0.20mm. [35mm 사양서, v1.3, p.6](https://www.dropbox.com/scl/fi/fl600un1qv1cupbfjng95/GP-DS-170409-TM035035-SPI-I2C-PINN-Trackpad-Spec.pdf?rlkey=1pl0tw13air87pt92mkj72igx&dl=0) | 지름이 34mm 볼보다 작지는 않지만 볼과 홀더를 설치할 깊이가 필요하지 않다. 하부 지지체와 케이블은 표의 두께 외에 필요하다. |
| Pimoroni Trackball Breakout, PIM447 | 현재 제조사 페이지의 외곽 치수는 25 x 22 x 11mm다. 볼, 사출 홀더, Hall 센서와 MCU가 있는 기판의 제품이며 I2C, 3.3V 또는 5V를 사용한다. [Pimoroni 제품 사양](https://shop.pimoroni.com/products/trackball-breakout) | 25mm는 볼 지름이 아니다. 별도 장착체와 배선까지 포함한 치수가 아니며, 볼 지름과 광학 트랙볼에 대응하는 정확도는 해당 사양에 명시되지 않는다. 구형 자료의 9.5mm 높이를 현재 제품에 그대로 적용하지 않는다. |
| Azoteq TPS43 | 43 x 40mm 직사각형 터치패드로 IQS572 컨트롤러와 6핀 I2C 연결을 사용한다. PCB 두께는 1mm이며 오버레이 선택에 따라 적층이 달라진다. [제조사 제품 설명](https://www.azoteq.com/product/i2c-trackpad-modules/), [사양서 v1.06](https://www.azoteq.com/images/stories/pdf/proxsense_i2c_trackpad_datasheet.pdf) | PCB 1mm를 부품, 오버레이와 장착체가 포함된 높이로 사용하지 않는다. 43mm와 40mm 어느 방향도 2u 비교안의 39.15mm 키캡 사이 통로보다 크므로 3u 중앙안에서 비교할 후보다. |
| Oddball v2의 25mm 광학 트랙볼 구조 | 설계자가 25mm 볼, 3.175mm 지지구 3개, ADNS9800 또는 PMW3360 센서와 별도 지지체를 사용한다. 조립 가이드에 CAD 링크와 QMK 빌드 명령이 있다. [Oddball v2 가이드](https://atulloh.github.io/oddball/v2/build-guide.html) | 25mm는 볼 지름이다. 중앙에 이식할 전체 조립체 W/H/Z를 이 조사에서 확정하지 않았다. 원 설계자는 센서 높이와 각도 때문에 기울어진 케이스가 필요하다고 설명하므로, 낮은 No.1/60 중앙에 그대로 맞는 모듈로 볼 수 없다. [설계자 README](https://github.com/atulloh/oddball) |

## 중앙 너비에 미치는 영향

다음 값은 현재 저장소의 19.05mm 피치와 18mm 키캡 가정을 이용한 계산이며 실물 측정값이 아니다. 14u 비교안의 G/H 중심이 x = +/-28.575mm이면 두 키캡 안쪽 면의 간격은 `2 x 28.575 - 18 = 39.15mm`다. 현재 15u 추천안에서는 같은 간격이 `2 x 38.10 - 18 = 58.20mm`다. [현재 배치의 기준](project-context.md)

| 장치 외곽을 14u 중앙에 놓는 가정 | 양쪽에 남는 명목 간격 | 해석 |
| --- | --- | --- |
| Cirque 23.20mm 모듈 | 각각 7.975mm | 하부 지지체, 베젤, 케이블을 이 간격 안이나 다른 높이에 배치할 수 있는지 추가 확인 필요 |
| Cirque 35.0mm 모듈 | 각각 2.075mm | 모듈 자체의 평면 배치는 가능성이 있지만 키캡 스트로크, 손가락 접근과 장착 테두리의 여유가 작음 |
| Pimoroni의 25mm 변을 중앙 폭으로 사용 | 각각 7.075mm | 실제 장착체와 배선을 제외한 제품 치수에 대한 계산 |
| 지름 25mm 볼만 사용 | 각각 7.075mm | 홀더, 지지점, 렌즈, 센서와 나사의 여유를 뜻하지 않으므로 조립체 수납 판단에 사용 불가 |

따라서 작은 터치패드와 PIM447은 현재 50 x 50mm 임시 예약 영역보다 작은 제품 외곽을 제시한다. 이로부터 14u안을 다시 검토할 이유는 생기지만, 실제 케이스 개구부 통과나 전체 키 수를 유지하는 새 배열의 성립은 별도 문제다. 키캡 사이의 명목 공간이 있다고 해서 인접 스위치 하우징, 소켓, 부품 및 케이블과 같은 높이에서 충돌하지 않는다는 뜻은 아니다.

## 펌웨어와 사용성의 타협

- **Cirque Pinnacle**: QMK는 Pinnacle 1CA027을 사용하는 TM023023, TM035035, TM040040을 명시하고 I2C 및 SPI 드라이버를 제공한다. 탭과 원형 스크롤 등의 기능을 설정할 수 있으나 이를 노트북의 모든 멀티터치 기능과 동일시하지 않는다. 23mm의 실제 감지 지름은 21.60mm이므로 짧은 손가락 이동 거리와 커서 감도 사이의 타협을 시제품으로 평가해야 한다. [QMK 지원 범위와 설정](https://docs.qmk.fm/features/pointing_device#cirque-trackpad)
- **Cirque Gen6s**: 제조사 문서 목록은 기존 Pinnacle 사양을 Deprecated로 분류한다. 현재 23mm Gen6s 사양은 I2C 주소 0x2C의 HID over I2C이며 6핀 커넥터를 사용한다. 기존 Pinnacle의 드라이버와 배선을 그대로 적용할 근거가 없으므로 기존 부품 확보 여부 또는 새 드라이버 필요성을 먼저 확인한다. 23mm Gen6s는 모듈 지름 23.00 +/- 0.15mm, 감지 영역 지름 21.00mm, PCB와 Mylar 오버레이 및 부품 높이 5.05 +/- 0.320mm를 명시한다. 파일명은 GP-DS-230612이지만 문서 본문은 GP-DS-240605 v1.3이다. [제조사 문서 목록](https://www.cirque.com/documentation), [Gen6s 사양서, p.5-8, 12](https://www.dropbox.com/scl/fi/jqbt0uy83zhshvf3oyvsa/GP-DS-230612-TM023023-Gen6s-Touchpad-Specification.pdf?rlkey=h2e66gosfo6gbo7pbuf0hxpqj&dl=0)
- **Pimoroni**: QMK의 `pimoroni_trackball` 드라이버가 있다. 제조사가 설명하는 방식은 회전하는 자석을 Hall 센서로 읽는 구조이므로 PMW3360 광학 센서와 같은 정밀도나 빠른 이동 추적 성능을 보장할 수 없다. 공간 검증용 후보로는 구체적이지만 주 포인팅 장치를 대체할 수 있는지는 직접 사용해 확인해야 한다. [QMK 지원](https://docs.qmk.fm/features/pointing_device#pimoroni-trackball), [제조사 구조 설명](https://shop.pimoroni.com/products/trackball-breakout)
- **Azoteq TPS43**: QMK는 `azoteq_iqs5xx`와 TPS43 프로파일, 두 손가락 탭 및 스크롤 설정을 제공한다. holykeebs에도 TPS43 기반 모듈의 조립 사례가 있다. 다만 제조사 목록은 IQS572-B000을 새 설계에 권장하지 않는 NRFND로 표시하므로 확보할 부품과 공급 지속성을 확인해야 한다. 기존 3u 중앙 공간을 유지하면서 넓은 입력 면적을 원할 때의 비교 후보이며 노트북 터치패드와 같은 사용감을 보장하지 않는다. [QMK 지원](https://docs.qmk.fm/features/pointing_device#azoteq-iqs5xx-trackpad), [모듈 사례](https://holykeebs.com/products/touchpad-module), [제조사 부품 상태](https://www.azoteq.com/design/datasheets/)
- **Oddball 25mm**: 광학 센서와 볼을 유지할 수 있는 설계 사례다. 그러나 전용 텐팅 구조와 설계자 QMK fork를 No.1/60용 새 기구 및 펌웨어에 맞춰야 하므로 즉시 사용할 소형 완성 모듈로 추천하지 않는다. [설계자 자료](https://github.com/atulloh/oddball), [조립 및 펌웨어 가이드](https://atulloh.github.io/oddball/v2/build-guide.html)

Cirque 설치는 하부에서 지지해 탭 동작 시 과도한 휨을 막고, 베젤에 도전성 재료를 사용하지 않도록 하는 제조사 권고를 따라야 한다. 금속 Keycult 케이스와 금속 플레이트에 직접 밀착시키는 방식의 적합성을 가정하지 않는다. 이 조건은 비도전성 베젤과 지지체를 실제 높이에 포함해야 하는 이유다. [35mm 사양서, p.12](https://www.dropbox.com/scl/fi/fl600un1qv1cupbfjng95/GP-DS-170409-TM035035-SPI-I2C-PINN-Trackpad-Spec.pdf?rlkey=1pl0tw13air87pt92mkj72igx&dl=0)

TrackPoint 방식도 중앙 포인팅의 별도 후보다. holykeebs는 센서와 드라이버가 분리된 3.3V `SK8707-01-004`를 제공하고, QMK는 PS/2 포인팅 장치를 지원한다. 다만 기존 분리형 키보드용 완성 어댑터를 No.1/60에 그대로 넣을 수 있다는 뜻은 아니다. 고정 구조, 스틱과 키캡 사이 간섭, 배선과 초기화 회로를 검토해야 하며 힘으로 커서를 움직이는 조작감도 트랙볼과 다르다. 키캡 사이에 보이는 스틱 크기만으로 하부 조립체의 수납을 판단할 수 없어 이번 우선 후보에서는 제외한다. [센서 제품과 사양서](https://holykeebs.com/products/sk8707-01-trackpoint-sensor), [QMK PS/2 지원](https://docs.qmk.fm/features/ps2_mouse)

## 다음 선택과 검증

1. **중앙 통합에 쓸 수 있는 공간을 확정**한다. 보유 케이스의 중앙 깊이, 개구부, 장착 기준과 키캡 높이를 측정하고, 볼 및 터치패드의 허용 돌출 높이를 실제 타건 자세에서 확인한다.
2. **34mm 조립체와 중앙 터치패드를 비교**한다. Keyball 홀더 전체 외곽을 확보하고, Cirque Pinnacle 23mm와 35mm는 확보 가능한 전체 부품번호를 확인한다. 같은 중앙 위치에서 지지체, 케이블과 설치 높이를 포함해 비교한다. 시제품의 임시 외부 컨트롤러는 검증용이며 완성품의 외부 포인팅 장치를 대안으로 삼지 않는다.
3. **중앙에서 작동하는 기구 시제품을 검증한 뒤 PCB를 설계**한다. 키를 끝까지 누르거나 포인팅 장치를 조작할 때 바닥 접촉, 센서 상대 변위, 손가락 간섭이 없어야 한다. 34mm 안이 조건을 충족하지 못하면 작은 볼이나 터치패드의 중앙 통합을 검토한다.

어떤 중앙 후보를 선택하든 케이스의 중앙 깊이, 개구부, 장착 기준, 모듈과 지지체의 조립 외곽, 케이블 경로를 같은 좌표계로 측정해야 한다. 새 배열을 만들기 전에는 종이에 실제 크기의 감지 면적을 출력해 G/H 사이에서 접근과 이동 거리를 확인하고, 센서 평가용 시제품으로 커서 이동과 클릭 및 스크롤을 확인한다. 요구사항을 실제 변경하면 [프로젝트 맥락](project-context.md), 생성기와 관련 검증을 함께 갱신한다.

이 조사는 제품 구매, CAD 형상 이식, PCB 설계, 펌웨어 빌드 또는 실물 사용 시험을 수행하지 않았다. 치수 표는 공개 사양의 해당 세대에만 적용하며 현재 판매 재고와 배송을 보장하지 않는다. 제3자 CAD와 사양서 원본을 저장소에 복제하지 않았으며, 설계 파일을 재사용할 때의 권한 확인은 [기존 자료 관리 기준](alternatives-and-licenses.md)을 따른다.

## 순정 Keyball 홀더를 가공하지 않고 재사용하는 조건

추가 확인일: 2026-10-30. **순정 34mm 홀더와 센서 조합을 유지하고 새 중앙 마운트를 만드는 방향은 검토할 근거가 있다.** 공식 Keyball39 교체용 키트는 Keyball44 등에서 쓰던 순정 홀더와 센서 부품을 옮겨 쓰도록 판매한다. 이는 부품의 분리와 재사용을 뒷받침하지만 No.1/60의 중앙 공간이나 임의 마운트와의 호환을 보장하지 않는다. [공식 Keyball39 교체용 키트](https://shiroganelab.com/products/keyball39-white-pcb-upgrade-set-no-trackball-parts)

공식 Keyball44 Rev.1 조립 가이드의 구조는 다음과 같다. 아래 절차는 홀더를 새로 설계하는 대신 순정 조립 관계를 보존할 때 필요한 인터페이스다. [고정 리비전 가이드 4절과 7절](https://github.com/Yowkees/keyball/blob/e1d1948e6e60a83f6b3566a209326508200d64ce/keyball44/doc/rev1/buildguide_en.md#anchor7)

- 홀더는 상부와 하부로 나뉘며, 세라믹 지지구 3개가 미리 고정되어 있다. 상하부의 핀과 구멍을 맞춘 뒤 작은 머리 M1.7 태핑 나사 2개로 결합한다.
- 센서 PCB는 메인 PCB의 7핀 L자 커넥터에 수직으로 꽂힌다. 완성 홀더를 위에서 내리면 홀더 뒤쪽 수납부에 센서 PCB가 들어간다. 렌즈는 정해진 방향으로 센서 IC에 결합한다.
- 홀더를 본체 아래에서 별도의 평두 M1.7 나사 2개로 고정한다. 조립할 때 센서 PCB와 메인 PCB 사이 간격이 사라지고 나사 구멍이 맞아야 하며, 과도한 힘은 L자 커넥터를 손상할 수 있다. 홀더만 떼어 임의 높이에 고정해도 센서 위치가 자동으로 맞는 구조로 해석하지 않는다.

리비전과 색상별 부품을 구분해야 한다. 같은 고정 커밋의 Keyball+ 가이드도 수직 센서와 같은 조립 순서를 설명하지만, 홀더의 치수와 리비전 간 완전한 호환을 명시하지 않는다. 현재 Keyball44 판매 페이지는 상하부 결합용 작은 머리 나사를 흰색 세트 M1.7, 검은색 세트 M1.4로 구분하고, 본체 고정용 평두 나사는 M1.7로 기재한다. 따라서 실제 확보한 홀더의 나사 규격을 확인한다. [Keyball+ 가이드 8절](https://github.com/Yowkees/keyball/blob/e1d1948e6e60a83f6b3566a209326508200d64ce/keyballplus/doc/rev1/buildguide_en.md#anchor8), [Keyball44 동봉품](https://shiroganelab.com/products/keyball44)

홀더 확보 경로도 구분한다. 공식 FAQ는 Keyball 구매자에게 교체 홀더를 문의 방식으로 판매할 수 있다고 안내한다. 2026-01-22 공지는 일부 부품의 일반 판매를 알리지만 홀더를 특정하지 않으며, 조사한 전자부품 목록에서는 독립 홀더 상품을 확인하지 못했다. 별도 판매되는 납땜 완료 센서/렌즈 세트는 홀더 포함 상품으로 명시되지 않는다. 구매 이력이 없는 경우의 홀더 판매 조건과 공급 리비전은 미확정이다. [공식 FAQ](https://shiroganelab.com/pages/faq), [부품 판매 공지](https://shiroganelab.com/en/blogs/news-blog/keyball-parts-now-available), [전자부품 목록](https://shiroganelab.com/en/collections/electronic-components), [센서/렌즈 세트](https://shiroganelab.com/products/keyball-pre-soldered-trackball-sensor-lens-set)

**홀더 무가공 재사용을 확정하려면 전체 외곽, 본체 고정 구멍 좌표, 접촉면 높이, 나사 길이와 물림 깊이, 센서 및 커넥터 위치를 같은 리비전에서 확보해야 한다.** 이번 조립·판매 자료 조사에서는 이를 갖춘 생산 홀더 CAD나 치수 도면을 확인하지 못했다. 2021년 개발기는 34mm 볼을 유지하는 개구를 33.3mm로 정했다고 설명하지만, 이는 당시 볼 유지부의 값이며 현재 홀더 외경, 장착 구멍 또는 중앙 개구의 치수가 아니다. 새 지지체가 순정 홀더와 센서를 같은 강체에 고정하면서 원래 광학 관계를 보존하고 케이스 안에 들어가는지는 별도로 검증한다. [공식 개발기 4절](https://shiroganelab.com/en/blogs/news-blog/deverop_keyball46)

같은 고정 커밋의 Keyball44 바닥 아크릴 DXF와 오른쪽 KiCad PCB를 대조하면, 조립 사진의 홀더 체결부에 해당하는 바닥판 구멍은 **지름 약 2.5mm 두 개, 중심 간격 약 16mm**다. 이는 원래 호스트 바닥판에서 계산한 참고값이며 홀더의 나사산 지름이나 실측 공차가 아니다. DXF는 단위 헤더가 inch인데 수치 형상은 mm 크기의 PCB와 대응하므로 그대로 가공에 사용하지 않는다. PCB 장착점 7개로 반사·이동·균일 배율을 대조한 과정과 원본 해시는 [출처 기록](../design/preliminary/sources/README.md#keyball-holder-host-interface-review)에 남긴다. 바닥판의 원형 접근 구멍 중심을 볼의 공식 중심 기준으로 확정하지 않는다. [공식 바닥판 및 PCB 파일](https://github.com/Yowkees/keyball/tree/e1d1948e6e60a83f6b3566a209326508200d64ce/keyball44/design_data), [아래쪽 체결 사진](https://github.com/Yowkees/keyball/blob/e1d1948e6e60a83f6b3566a209326508200d64ce/keyball44/doc/rev1/images/kb44_132.jpg)

현재 설계에 대한 결론은 **순정 홀더와 맞는 센서·렌즈를 유지하고, 보강판에서 지지하는 아래쪽 받침과 센서 커넥터 장착부를 새로 설계하는 안이 타당한 후속 검토 대상**이라는 것이다. 이는 조립 자료에서 도출한 설계 방향이며 장착 검증 결과가 아니다. 기존 FreeCAD의 42mm 정사각 네 점 패턴은 어댑터와 보강판 사이의 가정이고, 순정 홀더의 두 점 패턴을 대신하지 않는다. 지름 40mm 개구와 평평한 프레임에는 이 아래쪽 받침이 없으므로 그대로 순정 홀더를 고정할 수 있다고 보지 않는다. 실제 홀더와 센서가 같은 지지체에 고정되도록 원래 접촉면과 커넥터 높이를 재현하고, PCB 중앙 개구 및 나사 접근 공간을 따로 검토해야 한다.

공개 저장소 전체 트리에서 독립된 생산 홀더 STEP/STL을 확인하지 못했으므로, 지금 단계에서 홀더가 중앙 50 x 50mm 예약 영역이나 뒤쪽 확장 공간에 들어간다는 판정은 하지 않는다. 필요한 추가 정보는 확보할 홀더의 최대 외곽, 바닥 접촉면에서 볼 중심까지의 높이, 센서·렌즈·커넥터를 결합한 외곽과 나사 길이이며, 이 값으로 실제 케이스 내부 깊이와 대조해야 한다. 이번 확인은 CAD 치수와 자료 조사이며 기존 FreeCAD 및 KiCad 형상은 변경하지 않았다.
