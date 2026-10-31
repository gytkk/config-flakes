# Keycult65 and Keycult62 KiCad designs

**65-key DRAFT E and 62-key DRAFT H, not approved for fabrication or powered assembly.** Both variants use the same RP2040 circuit and stable key references. DRAFT H keeps the centred 48 x 48mm R1 opening and conditional Keyball module interface, moves every populated component to the top, and places the sensor above the main PCB on the raised cocot-inspired holder. Open the corresponding project in **KiCad 10.0.7**. The schematic and PCB are native editable CAD.

| Variant | Project | Schematic | PCB | Validation |
| --- | --- | --- | --- | --- |
| 65 keys | [keycult65.kicad_pro](keycult65.kicad_pro) | [Schematic](keycult65.kicad_sch) | [PCB](keycult65.kicad_pcb) | [Report](reports/validation.json) |
| 62 keys | [keycult62.kicad_pro](62/keycult62.kicad_pro) | [Schematic](62/keycult62.kicad_sch) | [PCB](62/keycult62.kicad_pcb) | [Report](62/reports/validation.json) |

Both variants pass strict native validation with zero ERC findings, DRC findings, unconnected items, schematic parity errors and independent netlist mismatches. The 62-key result follows fresh seed generation and saved SES replay, including local matrix, QSPI and power escape repairs recorded in its route manifest. The reports bind these results to the CAD hashes. Static electrical checks do not establish powered operation or case fit. The user-approved design assumption is an unmodified external UDB C5-JSH, represented by DB1 in the schematic and assembly BOM, with no footprint on the main PCB. J1 implements its 1:1 cable interface. The 62-key sensor interface uses the explicit draft assumption below; the actual module remains unidentified.

Historical DRAFT G preserved DRAFT F's nets and placements while shrinking the opening. DRAFT H replaces that 62-key placement and route to satisfy the shallow underside assumption. The 65-key CAD, exports and reports remain the delivered DRAFT E snapshot; shared-input hash refreshes do not change its geometry.

## Review entry points

- [Native schematic](keycult65.kicad_sch), [native PCB](keycult65.kicad_pcb), [project](keycult65.kicad_pro).
- [Schematic PDF](exports/schematic.pdf), [front copper SVG](exports/front.svg), [back copper SVG](exports/back.svg), [ground plane SVG](exports/ground.svg), [layer PDF](exports/board-layers.pdf).
- 62-key exports: [schematic PDF](62/exports/schematic.pdf), [front](62/exports/front.svg), [back](62/exports/back.svg), [ground plane](62/exports/ground.svg), [layer PDF](62/exports/board-layers.pdf), [BOM](62/bom.csv), [key/GPIO map](62/gpio-map.csv).
- [Real validation summary](reports/validation.json), [KiCad ERC](reports/erc.json), [KiCad DRC including native schematic parity](reports/drc.json), [actual exported netlist](reports/netlist.xml).
The layer PDF is auto-fitted for review, **not a 1:1 print or manufacturing template**.

- [BOM with candidates/uncertainties](bom.csv), [key/GPIO map](gpio-map.csv), [QMK pin reference](qmk-config.h.example), [source/license record](sources.md).

## Shallow 62-key prototype

DRAFT H uses the user's assumed clearance of 0.82mm below a nominal 1.6mm PCB, derived from the short Mill-Max 3305-0 reference rather than a case measurement. The switch footprints remain solder footprints. All 96 SMT parts, including the 62 diodes, are on F.Cu; the 62 switches remain top-mounted THT. Bottom test pads are copper features. Standard untrimmed MX leads project approximately 1.70mm below a 1.6mm PCB and exceed this assumption. All metal leads, plastic posts, solder and insulation must be trimmed or selected and physically checked against the 0.82mm limit. Top-side SMT placement alone is not a whole-board height pass.

The MCU, flash, power and service components occupy the empty rear central band. Diodes occupy the gaps between switch rows, with the bottom-row diodes offset horizontally from the preceding row. The copper/component reservation is now PCB x=155.75..204.25mm, y=75.75..124.25mm, a 0.25mm guard around the existing 48 x 48mm opening. It no longer reserves the entire empty key band. The 65-key snapshot keeps its original bottom electronics and central reservation.

J1 is now F.Cu, origin (153,64)mm, rotation 90 degrees; J2 is F.Cu, origin (177,58)mm, rotation 180 degrees. J1 has 0.825mm nominal clearance from the conservative 15.6mm switch proxy. Existing GND vias stay in place, with short F.Cu bridges to the moved pads; USB starts are shortened and a 1mm supply bridge preserves the power route. Their contact-number maps and external harness requirements are unchanged. Stock model heights above the mounting plane are 2.96mm for J1, 4.245mm for J2, 2.16mm for flash, 1.55mm for the LDO, 0.9mm for the MCU and 0.64mm for the crystal. These are library geometry, not supplied-part measurements. J2 needs the candidate plate's rear electronics relief because the PCB-top-to-plate-bottom gap is only 3.5mm. Connector insertion, cable routing and service-button access still need assembly inspection. The old J1 location and long USB route documented below apply to the 65-key DRAFT E snapshot.

The 62-key USB pair follows a short F.Cu route in the rear band with 0.29mm traces and a local 0.15mm D+ escape between the series-resistor pads. TP3 has a short B.Cu test branch; TP4 is accessed through a via. Ground stitches provide nearby returns. The uniform 0.29mm impedance estimate below does not qualify these local transitions or the cable. Read the current report for actual width, pair gap and reference-plane checks.

The [prototype CAM archive](62/keycult62-prototype-cam.zip) and [manifest](62/prototype-cam/manifest.json) contain four copper Gerbers, both masks, silkscreens and pastes, the outer/inner Edge.Cuts profile, separate PTH/NPTH Excellon drills, drill maps/report, the candidate BOM, all-component positions and top-SMT positions. The SMT file excludes test pads, external DB1 and THT switches. Coordinates are absolute mm; position CSV Y is positive upward, so PCB Y is negated. Bottom X is not mirrored. The assembler must approve native KiCad rotations and the QFN via/paste process.

Regenerate the bundle only after strict checks pass:

```bash
uv run --python "$(command -v python3)" scripts/validate_pcb.py --keys 62 --prototype-cam
```

Export checks compare all four copper X2 layer functions, drill-hit counts and placement references/positions/rotations/sides against the native PCB. The manifest binds CAM and CAD hashes and lists the remaining gates. Its stackup records F.Cu / In1.Cu / In2.Cu / B.Cu in order, material and dielectric constants. The copper/dielectric sum is 1.5862mm against nominal 1.6mm; mask construction and finish remain unspecified. The Gerber job's size includes the profile stroke, so use the native 285.0 x 94.6mm outline dimensions. This bundle permits local prototype review; it does not authorize an order or establish manufacturer approval.

## Implemented electrical content

65 or 62 MX PCB-mount solder switches and individual 1N4148W diodes. Cathode/pad 1 is
the ROW net; current path is COL → switch → diode anode → cathode → ROW. QMK
`COL2ROW`, five rows × fifteen columns; ten intersections are absent in the 65-key variant and thirteen in the 62-key variant. IDs, not labels, identify keys because labels repeat.

Both variants omit bottom-row SW59/D59 and retain SW58/D58 and SW60/D60 as the 1.5u M1/M2 keys, preserving IDs r4c6/r4c8 and removing r4c7. Their layout centres are (-14.2875,38.1)mm and (14.2875,38.1)mm. The 62-key variant also omits SW7-SW9 and D7-D9 without renumbering other parts. COL6 and COL8 remain connected to the bottom keys; COL7 has no key, so GP16 is NC. The [compiled prototype firmware](../../firmware/README.md) uses `NO_PIN` for that column, `KC_NO` for absent cells, M1/M2 for mouse buttons and Fn+M1 for middle click. Cutting a populated older PCB is never implied by removing switches.

RP2040 is directly mounted in its 7×7mm QFN, not represented by a development
board. The design includes AP2112K-3.3, input PTC, input/output bulk capacitors,
individual IOVDD/ADC/USB/core decoupling, the internal 1.1V regulator's 1µF caps,
W25Q16JVSSIQ QSPI flash, QSPI clock resistor, CS pull-up, BOOTSEL through 1kΩ,
12MHz ABM8-272-T3 oscillator with 15pF load caps and 1kΩ drive resistor, USB 27Ω
series resistors, RUN pull-up/reset button, and individual SWD pads. TESTEN is
GND. Unused GPIOs and regulator NC have explicit no-connect marks.

**W25Q16JVSSIQ uses the 208mil/5.3mm SOIC footprint**, not a similarly named
150mil SOIC. Passive MPNs, capacitor bias derating, crystal drive/startup and
supply thermal/load budgets remain a procurement/bring-up review, not claims of
qualification. No power LED was added. PTC hold rating does not authorize a
500mA USB device or prove USB descriptor/inrush compliance.

### GPIO budget

| Purpose | RP2040 GPIO |
|---|---|
| SPI0 MISO / CS / SCLK / MOSI | GP0 / GP1 / GP2 / GP3 |
| Rows 0–4 | GP4, GP5, GP6, GP7, GP8 |
| Columns 0–14 | GP9–GP22, GP26; GP16/COL7 is NC in the 62-key variant |
| Optional sensor motion / reset | GP23 / GP24 |
| Explicit unused/NC | GP25 / GP27 / GP28 / GP29; also GP16 in the 62-key variant |

The 65-key variant allocates 26 of 30 GPIOs, including two optional sensor controls; four remain
unused. The 62-key variant allocates 25, leaving five NC. Dedicated QSPI, USB and SWD pins do not consume these 30 GPIOs.
`qmk-config.h.example` remains a historical mapping aid. The [62-key firmware](../../firmware/README.md) supplies the compiled prototype port, selected boot stage, power declaration and keymap; powered operation is untested.

## USB daughterboard interface and compatibility limits

**J1 is a JST SM04B-SRSS-TB(LF)(SN), a 4-contact, 1.00mm-pitch side-entry header, on B.Cu for 65 keys and F.Cu for 62 keys.** The [Wilba WT60-D product documentation](https://salvun.com/products/wilba-60-pcb-wt60-d) explicitly identifies its JST connection as UDB-compatible and lists Keycult No.1/60 compatibility after removing the on-board USB connector and fitting JST. The [UDB designer specification](https://github.com/Unified-Daughterboard/UDB-C-Legacy/wiki/PCB-side-design-resources) and [pinned C3 schematic](https://github.com/Unified-Daughterboard/UDB-C-Legacy/blob/2ecdd7e0482ecbd3ffe9af04966ab3555f7fc33c/KiCad/C3/Unified-Daughterboard.sch) establish the connector and pinout. This supports the UDB electrical design target; it does not identify the user's uninspected daughterboard as a particular UDB revision.

| J1 contact | Signal | Existing test pad |
| --- | --- | --- |
| 1 | VBUS_USB, after C5-JSH protection and before main-board F1 | TP1 |
| 2 | USB_DM, D- | TP3 |
| 3 | USB_DP, D+ | TP4 |
| 4 | GND | TP2 |
| MP | Mechanical solder anchors, tied to GND locally | Not a cable contact |

Use a **1:1 contact-number cable**, 1-to-1 through 4-to-4. Wire color and apparent left-to-right order are not pin-number evidence. TP1-TP4 retain their earlier numbering and are test access, not the connector's contact order. The validator independently checks J1 against the UDB map, including rejection of reversed power and swapped data contacts.

The 65-key J1 origin is PCB **(40.6,112.0)mm**, B.Cu, rotation 90 degrees, with its mouth facing keyboard-left. Looking down from the switch side, pad centres are x=42.6mm and y=113.5,112.5,111.5,110.5mm for contacts 1-4. The [official WT60-A photo](https://keycult.com/products/wt60-a-pcb-60) supports a left-edge connector facing outward, but supplies no exact coordinates. This is not a measured WT60-A registration. The 62-key top-side relocation is described above. Case-channel alignment, mating clearance and installed connector height need physical checking.

The 65-key J1 data pair runs on F.Cu along the left and top margins above In1.Cu GND, with short B.Cu escapes at J1, TP3/TP4 and R1/R2. These connector-side traces are 0.29mm wide with a 0.2mm edge gap, and VBUS has a separate 0.4mm fuse feed. The 62-key route is described under the shallow prototype above.

The following 2026-10-30 measurements describe the 65-key DRAFT E USB route, which was also used by historical 62-key DRAFT G. DRAFT H has a different short route. Lengths are segment centreline sums including test branches; they exclude pad traversal, via barrels and the external cable, so they are not end-to-end electrical lengths.

| Saved copper | D- | D+ |
| --- | --- | --- |
| J1-side net, before the series resistor | 151.4028mm | 157.3721mm |
| F.Cu portion of J1-side net | 146.5028mm | 150.4615mm |
| B.Cu portion of J1-side net | 4.9000mm | 6.9107mm |
| Vias on the J1-side net | 4 | 4 |
| MCU-side net, after the series resistor | 6.6253mm | 6.6253mm |

The J1-side segment-sum difference is 5.9693mm. These are measured routing properties, not acceptance limits or evidence of signal-integrity failure. A USB netclass records the 0.29mm width and 0.2mm gap for these two nets. The candidate dielectric model and the limits of the impedance calculation are described under [USB routing findings](#usb-routing-findings).

Strict validation refills zones and checks the full-width projection of every F.Cu USB segment against the saved In1.Cu GND polygons. Both variants have zero uncovered area outside explicit 0.51mm-radius disks around USB transition vias. Those disks exclude signal-via antipads, not unrelated voids elsewhere on the route. The measured minimum F.Cu pair edge gap is 0.2mm. The layer PDF supports inspection of the J1 escape, margin pair, test pads, resistor transitions and ground fill. These geometric checks do not qualify finite plane edges, transitions or cable discontinuities.

DRAFT E uses its saved base route for the current 1.5u M1/M2 coordinates. After SES import, `scripts/usb_routing.py` applies the 65-key USB recipe and ground changes. For top-side J1 in DRAFT H it preserves the short seed/SES USB copper and applies only the candidate stackup. Current validation uses KiCad 10.0.7 from the parent repository's pinned Nix input. Route manifests include generator, layout and USB-script hashes. Native schematic parity, external DB1 netlist, coordinates and reservations are checked again; each variant has native PDF/SVG exports and reports.

DB1 is the unmodified [C5-JSH at revision `2b6c52aa2de6f9f60ca99c6250e4bdf01e8f4eea`](https://github.com/Unified-Daughterboard/UDB-C-JSH/blob/2b6c52aa2de6f9f60ca99c6250e4bdf01e8f4eea/UDB.kicad_sch), adopted as a design assumption on 2026-10-30. Its USB-C connector, two 5.1k CC pull-downs, TPD4E05U06DQAR data-line ESD array, ASMD1206-150 PTC and SMF6.0A TVS stay on the external daughterboard. Its shield joins signal ground on that board; no extra shield wire, CC resistors or duplicate ESD/TVS array is added to the main PCB. This assumption is not a measurement of the factory Keycult board.

Main-board F1 remains a 500mA-hold PTC for local protection, and R1/R2 remain the RP2040's 27-ohm USB series resistors. The two PTCs are in series, so their total voltage drop, trip coordination and actual load/inrush need verification. The TVS suppresses transients; this design has no active sustained-overvoltage disconnect. DB1's BOM row is an external assembly requirement, not a component to populate on the main PCB.

The WT compatibility research supports continuing with the existing J1 pin map. [WT60-C's product documentation](https://dailyclack.com/products/wt60-c-kate-60-pcb) explicitly specifies the same JST part and UDB compatibility; WT60-D does likewise for the interface, and the [Keycult FAQ](https://resources.keycult.com/faq_keycult_faq.html) lists WT60-G for No.1/60. This evidence establishes the design interface without requiring a measurement of the stock cable. It does not establish that every WT-family or Keycult daughterboard contains the same protection components. [Pinned circuit evidence](sources.md#usb-daughterboard-evidence) records the UDB revision differences.

As confirmed by the user on 2026-10-30, a suitable cable length can be sourced; stock-cable reach is no longer a design constraint. Before powered assembly, verify the selected cable's four contact paths and that the assembled external board matches the assumed C5-JSH design. Connector insertion, routing and bend clearance remain mechanical checks in issue #3. Factory-board identification is not a prerequisite for circuit design under this explicit assumption.

## Power and USB desk review

The 2026-10-30 [calculated report](reports/electrical-review.json) records conditional power estimates and native measurements for both boards. It is separate from ERC/DRC and grants no electrical or manufacturing pass. [Primary sources](sources.md#electrical-desk-review-sources) identify the component revisions and public stackups. DRAFT E applies the 7628 routing-study geometry; the provisional power allocations remain assumptions and do not change components or authorize a particular sensor.

### Power allocation

Allocate a provisional 110mA on +3V3 to the host: 80mA for RP2040, 25mA for flash and 5mA for other board loads. Compare sensor allocations of 40/50/70mA, giving 150/160/180mA total and modeled junction temperatures of 111.73/115.78/123.88 C at 50 C ambient under the assumptions below. These are engineering scenarios, not guaranteed component maxima or measured module current. Flash's 25mA allowance requires checking the cited frequency/loading conditions and a read clock no greater than 104MHz. A module fed from 5V needs a separate power-path assessment.

The model reserves another 1mA for LDO ground current, giving 151/161/181mA input before external-board leakage for the three sensor scenarios. This is an assumption; AP2112's 80uA maximum is a no-load specification. The configured USB power declaration must cover the final whole-device load. The 110mA main-board allocation does not demonstrate compliance with the 100mA pre-configuration limit. Startup and the 2.5mA suspend limit need separate firmware budgets; no power-switching or suspend implementation is claimed here.

Heat screening assumes 5.5V input, nominal 3.3V output, no upstream voltage drop, 1mA LDO ground current, 50 C local ambient and the datasheet's 184 C/W junction-to-ambient resistance. This resistance is a reference condition, not a measurement of this PCB or the closed case.

| +3V3 load | Calculated LDO loss | Estimated junction temperature |
| --- | --- | --- |
| 100mA | 0.2255W | 91.5 C |
| 150mA | 0.3355W | 111.7 C |
| 160mA | 0.3575W | 115.8 C |
| 180mA | 0.4015W | 123.9 C |
| 200mA | 0.4455W | 132.0 C |
| 300mA | 0.6655W | 172.5 C |

Values beyond device limits indicate unsuitable modeled operating points, not predictions of continued operation. A chosen 125 C planning ceiling permits approximately 183mA under these assumptions; the 180mA scenario has little modeled margin. At 60 C ambient the 150mA estimate is 121.7 C and the 180mA estimate is 133.9 C. Higher final load or temperature requires reconsidering the regulator and layout. The 500mA-hold fuse and 600mA regulator labels are not an available continuous power budget.

The two PTC reference-temperature R1max values sum to 1.03ohm. At 151mA, adding 0.5/1.0/2.0ohm for the remaining internal supply/return path gives drops of 0.231/0.307/0.458V. From an assumed 4.4V at the daughterboard receptacle, the LDO sees approximately 4.169/4.093/3.942V. This is a low-voltage stress scenario, not permission to draw 151mA from a low-power port. External USB cable drop is already represented by receptacle voltage; hot PTC resistance is not bounded by the reference values. Nominal 3.3V plus 0.2V dropout is only a screening threshold. Actual regulation must satisfy the RP2040 USB_VDD minimum of 3.135V and flash supply maximum of 3.6V, among the other component conditions.

Native capacitor sums are 1.0uF on VBUS_FUSED, 2.9uF on +3V3 and 1.2uF on +1V1. They omit external-board/sensor capacitance and do not model tolerance, DC bias, regulator startup or active current. Nominal capacitor sums cannot establish an inrush pass.

### USB routing findings

Both variants use identical connector-side USB geometry and an explicit JLC04161H-7628 candidate copper/dielectric model: nominal 1.6mm board, 0.035mm outer copper, 0.0152mm inner copper, 0.2104mm outer-to-inner prepreg with Er=4.4, and a 1.065mm core with Er=4.6. The long pair now runs on F.Cu, 0.2104mm above In1.Cu GND. B.Cu escapes total 4.9mm for D- and 6.91mm for D+ and still lack a continuous adjacent reference plane. Their discontinuities remain part of the signal-integrity review.

Three added GND vias at PCB `(38.5,113.2)`, `(113.4,55.7)` and `(118.5,54.7)mm` join local B.Cu ground patches to In1.Cu. The resistor-side transitions use the existing GND via near `(117.312,66.595)mm`. All eight USB vias are within 2.295mm of a GND via. Validation enforces a 3mm project study criterion, not a USB specification limit. Three B.Cu ground patches and the In1.Cu top-boundary extension from y=53.2mm to y=52.96mm are included in the native zone refill and DRC.

The **JLC04161H-7628 model is applied as a routing-study assumption**, not an approved manufacturing order. JLC04161H-3313 remains a comparison only. Approximate uncoated microstrip calculations at 0.2mm edge gap give:

| Candidate | F.Cu width 0.2mm | Trial F.Cu width | Trial differential impedance |
| --- | --- | --- | --- |
| JLC04161H-7628 | 108.7ohm | 0.29mm | 90.3ohm |
| JLC04161H-3313 | 76.9ohm | 0.16mm | 89.2ohm |

The 90.3ohm result applies to an idealized uniform F.Cu section. It omits solder mask, etched trace shape, finite plane edges, nearby copper, tolerances and transitions. The top outer trace edge is only about 0.395mm inside the modeled ground boundary, so finite-plane effects need review. The CAD stackup records copper and dielectric layers only; it does not specify a fabrication mask construction. Obtain fabricator-approved geometry before treating 0.29mm as a final controlled-impedance width. Strict DRC and ground projection checks passed after routing; they do not measure impedance or establish USB compliance.

The remaining electrical work includes fabricator review and firmware startup/suspend budgets. Sensor integration, actual PTC/cable resistance, thermal measurements, inrush and full-speed waveform tests remain open. The eventual cable construction and length must be included in signal-quality validation.

## Sensor interface remains unresolved

**62-key DRAFT H retains the user-approved circuit assumption, not powered-assembly approval.** The horizontal PMW3360 module sits below the raised ball on the same rigid printed holder and above the main PCB; optical spacing and mounting dimensions remain prototype assumptions. J2 is a host-side JST GH 7-contact, 1.25mm-pitch harness header: `SM07B-GHS-TB(LF)(SN)`, stock footprint `Connector_JST:JST_GH_SM07B-GHS-TB_1x07-1MP_P1.25mm_Horizontal`, F.Cu, origin `(177,58)mm`, rotation 180 degrees. Its cable housing is `GHR-07V-S`; the sensor-end connector remains unidentified. This does not claim that J2 mates directly with the sensor PCB. TP10-TP17 remain test access, while 65-key DRAFT E retains test pads without J2. The [pinned circuit review](sources.md#keyball-sensor-interface-evidence) covers the historical Keyball46 TPS73601/TXS0102 circuit and does not identify the user's module. No bare-PMW3360 circuit or cocot36plus main PCB is copied into this design.

J2 contacts are 1=SCLK/GP2, 2=CS/GP1, 3=GND, 4=+3V3, 5=GND, 6=MOSI/GP3, 7=MISO/GP0; MP anchors connect to GND. These numbers define the host harness. Confirm both cable ends and the actual module before assembly. GP23/TP14 motion and GP24/TP15 reset are retained as test access and are not exported on J2.

The published sensor board's J1, distinct from this project's USB J1, has the following contact numbers. They are schematic numbers, not an assumed left-to-right order viewed from either mating face.

| Published sensor J1 contact | Signal | Existing host allocation |
| --- | --- | --- |
| 1 | SCLK_5 | GP2 / TP12, SPI0 SCLK |
| 2 | NCS_5 | GP1 / TP11, GPIO chip select |
| 3 | GND | TP16 / GND |
| 4 | Supply labelled 5V upstream | Conditional +3V3 feed, TP17 identifies the existing rail |
| 5 | GND | Same GND as contact 3 |
| 6 | MOSI_5 | GP3 / TP13, SPI0 TX |
| 7 | MISO_5 | GP0 / TP10, SPI0 RX |

The published circuit uses TPS73601 to generate approximately 1.9V for PMW3360 VDD and VDDIO and the A sides of two TXS0102 translators. Their B sides share the connector supply. **A module powered at the published 5V therefore exposes a 5V host-facing MISO and cannot connect directly to RP2040.** A 3.3V host output is also below TXS0102's specified B-side high threshold when B is powered at 5V. The sensor IC's ability to use 3.3V VDDIO does not change this module's 1.9V wiring.

The implemented assumption for this exact historical circuit feeds sensor contact 4 from host +3V3, retaining the module's regulator and translators and connecting the four SPI signals and both grounds as above. TPS736's 1.7-5.5V input range and TXS0102's A=1.65-3.6V, B=2.3-5.5V ranges permit the resulting 1.9V/3.3V rails. This is an inference from component limits, not a vendor-approved 3.3V operating mode or a released wiring instruction. No separate translator or 5V feed is selected. Sensor-side connector pitch, orientation, mounting height and mating part still require hardware identification and mechanical review.

Before powered assembly or fabrication release:

- Identify the actual PCB revision, parts, contact numbering and rail connections, including the 1.9V regulator feedback network. A different circuit requires a new electrical review. Do not apply this map to an unidentified sensor.
- Recalculate +3V3 and USB budgets, LDO heating and startup/suspend behavior. PMW3360's 37mA RUN4 figure is typical, not a maximum; it nearly consumes the provisional 40mA sensor allowance before module losses. Its separate supply-ramp limits are 70mA on VDD and 60mA on VDDIO, excluding capacitor charging; they are not a measured simultaneous module input current. The published board adds nominal 10uF at its input and two 10uF downstream capacitors. Check effective capacitance, ramp timing, regulator dropout and the actual shared 1.9V load before approving the power path.
- Verify loaded DC levels and SPI waveforms with the intended cable. TXS0102's input-low limit is 0.15V; RP2040's 0.4V rated-load output-low bound alone does not prove this margin. Check the PMW-to-TXS side too. TXS0102 includes 10k pull-ups and edge accelerators; cable/connector capacitance, reflections and extra pulls affect operation. Keep the connection short with an adjacent ground return and disable unnecessary RP2040 pulls. A nominal 2MHz clock alone does not establish timing margin.
- Keep GP23 / TP14 motion and GP24 / TP15 reset disconnected from this module. Its 7-pin interface exports neither signal; motion is polled and reset through SPI. The [prototype port](../../firmware/README.md) pins QMK 0.29.0, SPI mode 3 and divisor 64, nominally 1.953125MHz at 125MHz, on GP2/GP3/GP0 and GPIO CS GP1. Compilation and source parity do not establish actual SPI timing, SROM load or sensor identification on hardware.

The generator, BOM and interface validation record the assumed host connector and power route; regenerate the 62-key CAD and inspect native ERC/DRC and parity reports after changes. Bench acceptance includes approximately 1.9V sensor rails, safe host-side levels, startup/reconnect, sensor product ID and SROM, motion and USB suspend. None of these powered checks has been performed. The 70mA VDD and 60mA VDDIO supply-ramp limits may be summed only as a transient screening scenario; they exclude capacitor charging and are not a continuous module allowance. J1's USB routing remains electrically documented, with case and cable fit provisional.

## Geometry and construction assumptions

- Layout `(0,0)` maps to PCB **(180,100)mm**, +X right, +Y toward the bottom row. Coordinates come from `../preliminary/layout.json`, using `preferred.keys` for 65 keys and `rear_clearance.keys` for 62 keys.
- Stock `SW_Cherry_MX_1.00u_PCB` and `SW_Cherry_MX_1.50u_PCB` have their anchor at **electrical pin 1**, not the
  stem. The stem NPTH is local `(-2.54,+5.08)mm`; generation compensates and
  validation reads the actual stem holes. The base pitch is 19.05mm; the two bottom 1.5u key centres are 28.575mm apart.
- The 65-key common circuit remains 76.2mm left of the original draft. The 62-key DRAFT H common circuit is mirrored onto the top in the rear central gap, with decouplers and connector positions adjusted. The original board must not be cut to obtain the 62-key variant.
- The **285.0 × 94.6mm** working rectangle uses only scalar dimensions measured
  from the pinned GH60 PCB. It does **not** copy GH60's rounded corners/holes,
  any Keycult contour, or the 285.75×108.25mm plate reference rectangle.
- The 65-key all-layer rule area remains the centred **50 x 50mm** reservation. The 62-key area is **48.5 x 48.5mm**, PCB x=155.75..204.25mm, y=75.75..124.25mm, with a 0.25mm guard around the **48 x 48mm, R1mm** inner Edge.Cuts opening at PCB x=156..204mm, y=76..124mm. Both areas prohibit footprints, pads, tracks, vias and copper fill. DRAFT H frees the rear central band for top electronics. This independent opening is not a measured holder outline; the 65-key board has no inner opening. Holder passage, support and fabrication suitability remain prototype checks.
- Four copper layers: F.Cu, B.Cu and In2.Cu carry most signals; In1.Cu provides the GND reference outside the central reservation, with any local escape traces recorded by net in each validation report. Pad/via antipads and the opening still require return-path review. The 65-key SMT is bottom-side; 62-key SMT is top-side between or behind the switch bodies. Actual native drill/courtyard checks and independent body checks apply. Nominal thickness is 1.6mm; this is not an impedance-qualified or measured case stackup.
- Review rules are 0.15mm clearance/minimum trace, 0.55-0.6mm via copper diameter with 0.3mm drill, and 0.25mm
  hole/copper-edge clearance. These are **draft routing constraints, not a
  selected manufacturer's process approval**. 0.4mm pitch QFN fan-out, solder
  mask, assembly tolerances and stackup need explicit manufacturing review.
- USB routes need a paired 90Ω differential/return-path review against a chosen
  stackup. Mere routed connectivity or DRC clearance does not establish signal
  integrity. QSPI/crystal/decoupling placement and path lengths need layout
  engineering review before a fabrication release.
- The [original 3D check record](reports/3d-review.json) is historical. Current pinned stock models are installed. Native STEP export succeeds with U1/U2/U3/J1/J2/Y1; including the unreadable service-switch model crashes the exporter. The selected six bodies are also readable independently in FreeCAD. Their library geometry and an assumed case model do not establish physical assembly fit.
- STEP models are linked from installed KiCad stock libraries; no Keycult case
  or sensor assembly STEP is redistributed. A valid 2D board does **not** prove
  Z-stack, actuator clearance, cable bend space, assembly access or case fit.

## Reproduction

The initial seed uses KiCad's maintained `pcbnew` API and installed stock
libraries. `sexpdata` serializes the native schematic with stock symbols and applies the deterministic USB overlay to native PCB data. No custom autorouter was written.

The 62-key [controller-seed.json](62/controller-seed.json) is an additional editable routing input. It records locked controller/flash/crystal copper derived from the strict-clean 65-key snapshot, the source PCB hash, the mirror transform and clipped bounds. Coordinates use the DSN/SES 0.0001mm grid. The ROW1 via is moved upward to clear the new opening. The generator reads back every controller record and verifies its geometry and locked state; the route manifest hashes this seed and the cleanup script. Review or regenerate the seed when changing controller placement or nets. Saved SES files omit user-fixed copper, so retain the controller seed for complete replay.
Freerouting performs local detailed routing via KiCad's Specctra DSN/SES API.
A small cleanup script splits reported dangling segments at same-net T junctions before removing the unused tails. It restores the preceding checked board and fails if any net gains unconnected items; it never suppresses DRC findings. Native CAD retains the complete editable route. Each variant's saved SES supplies routing around its fixed seed. After seed generation and SES import, `scripts/usb_routing.py` applies DRAFT E USB/ground changes for 65 keys; for DRAFT H top-side J1 it preserves the short route and applies only the candidate stackup. Replaying the SES through `route_pcb.py` reproduces that variant's route; rerunning the autorouter does not replace layout review or guarantee completion.
See [sources](sources.md) for versions, licenses and hashes.

For a local STEP containing the PCB and the six verified stock component models, run the following with the installed KiCad model library. The service switches, diodes, passives, MX switches, sensor, cable and case are omitted. This STEP is assembly-review geometry only and remains under the ignored local directory.

```bash
mkdir -p design/preliminary/sources/local/pcb-62
kicad-cli pcb export step --force --component-filter U1,U2,U3,J1,J2,Y1 \
  --output design/preliminary/sources/local/pcb-62/keycult62-electronics-six.step \
  design/pcb/62/keycult62.kicad_pcb
```

Run these commands from the project root, `workspace/keycult-trackball/`, with the Python version matching KiCad's installed binding (the original Nix runtime uses Python 3.14; the official 10.0.7 Docker image uses Python 3.13). Scripts require Python 3.11 or newer, but `pcbnew` must match the chosen interpreter. Use the [Docker procedure](../../docs/reproduction.md#docker에서-kicad-검증) when KiCad is not installed on the host.

```bash
uv run --python "$(command -v python3)" scripts/build_pcb.py --keys 65
uv run --python "$(command -v python3)" scripts/route_pcb.py --keys 65 --import-saved
uv run --python "$(command -v python3)" scripts/validate_pcb.py --keys 65
uv run --python "$(command -v python3)" scripts/build_pcb.py --keys 62
uv run --python "$(command -v python3)" scripts/route_pcb.py --keys 62 --import-saved
uv run --python "$(command -v python3)" scripts/validate_pcb.py --keys 62
uv run scripts/test_pcb_validation_gate.py
uv run scripts/validate.py
```

**Regeneration overwrites the current CAD seed.** The delivered native CAD is the
review/edit surface; preserve manual edits before rebuilding. The preliminary
layout generator remains the source for key coordinates. A changed layout
requires a new seed, route and inspection, not reuse of an old route snapshot.
The saved route is a reproducible initial draft, not a promise that arbitrary
edits autoroute the same way. Validation writes actual check reports and native
SVG/PDF exports; it does not mask or exclude violations. Running validation
without `--draft` returns nonzero for *any* DRC finding or unconnected item.

## Independent layout review findings

- The original draft had 57 unconnected items across 27 nets. DRAFT E completed the internal circuit and matrix routing; read each current report for the results after subsequent changes, including DRAFT F's added J2.
- The original C13/C14 placement was 16.60/17.52mm from the MCU supply pads; these capacitors and the core decouplers have been repositioned beside their actual rotated supply pins.
- F1's MF-PSMF050X-2 candidate requires an 0805 land pattern; the original 1206 footprint has been corrected.
- USB impedance and return paths still require review against a selected stackup and the actual daughterboard/cable before bring-up. Connectivity alone is not signal-integrity qualification.
- The exposed MCU ground pad has a 0.3mm ground via. Via tenting/filling and solder-paste process must be agreed with the assembler to control solder loss; static DRC does not qualify QFN assembly.
- Historical DRAFT E local In1.Cu exceptions use 0.15mm +3V3 and COL12 escapes beside the MCU. Current per-net counts are recorded in each report. These routes trade ground-plane area for QFN escape routing; the connected pour and USB reference region must be preserved in later edits.
- Draft validation permits unfinished connections only. Any geometric DRC
  finding fails in both strict and draft modes.

## Release gates still open

Issues #1-#6 remain the mechanical/interface/bring-up gate. Prototype CAM and [compiled QMK firmware](../../firmware/README.md) are available. Final procurement, sensor-end connector identity, conditional translator validation, fabricated stackup approval, trimmed switch/solder height, and USB/keyboard/sensor bench tests remain open. Read the actual route counts in each variant's `reports/validation.json`; zero ERC/DRC does not establish manufacturing or assembly approval.
