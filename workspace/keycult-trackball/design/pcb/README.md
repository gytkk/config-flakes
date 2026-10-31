# Keycult65 and Keycult62 KiCad designs

**65-key DRAFT E and 62-key DRAFT G, not for fabrication or assembly.** Both variants use the same RP2040 circuit and stable key references. DRAFT G uses a centred 48 x 48mm R1 opening for an independent cocot-inspired holder with a horizontal PMW3360 module below the ball. It retains DRAFT F's conditional Keyball module circuit and host-side sensor harness connector J2. Open the corresponding project in **KiCad 10.0.7**. The schematic and PCB are native editable CAD.

| Variant | Project | Schematic | PCB | Validation |
| --- | --- | --- | --- | --- |
| 65 keys | [keycult65.kicad_pro](keycult65.kicad_pro) | [Schematic](keycult65.kicad_sch) | [PCB](keycult65.kicad_pcb) | [Report](reports/validation.json) |
| 62 keys | [keycult62.kicad_pro](62/keycult62.kicad_pro) | [Schematic](62/keycult62.kicad_sch) | [PCB](62/keycult62.kicad_pcb) | [Report](62/reports/validation.json) |

Both variants pass strict native validation with zero ERC findings, DRC findings, unconnected items, schematic parity errors and independent netlist mismatches. The 62-key result follows fresh seed generation and saved SES replay, including local matrix, QSPI and power escape repairs recorded in its route manifest. The reports bind these results to the CAD hashes. Static electrical checks do not establish powered operation or case fit. The user-approved design assumption is an unmodified external UDB C5-JSH, represented by DB1 in the schematic and assembly BOM, with no footprint on the main PCB. J1 implements its 1:1 cable interface. The 62-key sensor interface uses the explicit draft assumption below; the actual module remains unidentified.

DRAFT G preserves all 62-key electrical nets, footprint placements and saved SES copper from DRAFT F. Replay was audited against those unchanged inputs and checked again with strict native validation after shrinking the opening. The 65-key CAD, exports and reports remain the delivered DRAFT E snapshot. Its shared-input hashes were refreshed after isolated seed generation and saved-SES replay proved identical circuit metadata, all 180 native footprints, 1,589 segment/via records and drawing contours, apart from UUIDs, with zero strict validation findings. Both saved routes therefore accept the current shared generator and layout JSON.

## Review entry points

- [Native schematic](keycult65.kicad_sch), [native PCB](keycult65.kicad_pcb), [project](keycult65.kicad_pro).
- [Schematic PDF](exports/schematic.pdf), [front copper SVG](exports/front.svg), [back copper SVG](exports/back.svg), [ground plane SVG](exports/ground.svg), [layer PDF](exports/board-layers.pdf).
- 62-key exports: [schematic PDF](62/exports/schematic.pdf), [front](62/exports/front.svg), [back](62/exports/back.svg), [ground plane](62/exports/ground.svg), [layer PDF](62/exports/board-layers.pdf), [BOM](62/bom.csv), [key/GPIO map](62/gpio-map.csv).
- [Real validation summary](reports/validation.json), [KiCad ERC](reports/erc.json), [KiCad DRC including native schematic parity](reports/drc.json), [actual exported netlist](reports/netlist.xml).
The layer PDF is auto-fitted for review, **not a 1:1 print or manufacturing template**.

- [BOM with candidates/uncertainties](bom.csv), [key/GPIO map](gpio-map.csv), [QMK pin reference](qmk-config.h.example), [source/license record](sources.md).

## Implemented electrical content

65 or 62 MX PCB-mount solder switches and individual 1N4148W diodes. Cathode/pad 1 is
the ROW net; current path is COL → switch → diode anode → cathode → ROW. QMK
`COL2ROW`, five rows × fifteen columns; ten intersections are absent in the 65-key variant and thirteen in the 62-key variant. IDs, not labels, identify keys because labels repeat.

Both variants omit bottom-row SW59/D59 and retain SW58/D58 and SW60/D60 as the 1.5u M1/M2 keys, preserving IDs r4c6/r4c8 and removing r4c7. Their layout centres are (-14.2875,38.1)mm and (14.2875,38.1)mm, occupying a combined nominal 3u. The M3 physical key has no replacement mapping yet. The 62-key variant also omits SW7-SW9 and D7-D9 without renumbering other parts. COL6 and COL8 remain connected to the bottom keys; COL7 has no key in the 62-key variant, so GP16 is marked NC in its schematic. The shared 15-column GPIO ordering is retained; all COL7 matrix cells must be KC_NO in eventual firmware. Removing only the three switches from a functioning 65-key board leaves open matrix cells; map those positions to `KC_NO` in the eventual firmware. Cutting the PCB is a separate mechanical and copper change, never implied by switch removal.

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
`qmk-config.h.example` is a mapping aid, **not a compiled QMK port**. It does not
select a sensor revision, flash boot stage, descriptor power or a final keymap.

## USB daughterboard interface and compatibility limits

**J1 is a JST SM04B-SRSS-TB(LF)(SN), a 4-contact, 1.00mm-pitch side-entry header on B.Cu.** The [Wilba WT60-D product documentation](https://salvun.com/products/wilba-60-pcb-wt60-d) explicitly identifies its JST connection as UDB-compatible and lists Keycult No.1/60 compatibility after removing the on-board USB connector and fitting JST. The [UDB designer specification](https://github.com/Unified-Daughterboard/UDB-C-Legacy/wiki/PCB-side-design-resources) and [pinned C3 schematic](https://github.com/Unified-Daughterboard/UDB-C-Legacy/blob/2ecdd7e0482ecbd3ffe9af04966ab3555f7fc33c/KiCad/C3/Unified-Daughterboard.sch) establish the connector and pinout. This supports the UDB electrical design target; it does not identify the user's uninspected daughterboard as a particular UDB revision.

| J1 contact | Signal | Existing test pad |
| --- | --- | --- |
| 1 | VBUS_USB, after C5-JSH protection and before main-board F1 | TP1 |
| 2 | USB_DM, D- | TP3 |
| 3 | USB_DP, D+ | TP4 |
| 4 | GND | TP2 |
| MP | Mechanical solder anchors, tied to GND locally | Not a cable contact |

Use a **1:1 contact-number cable**, 1-to-1 through 4-to-4. Wire color and apparent left-to-right order are not pin-number evidence. TP1-TP4 retain their earlier numbering and are test access, not the connector's contact order. The validator independently checks J1 against the UDB map, including rejection of reversed power and swapped data contacts.

J1's footprint origin is PCB **(40.6,112.0)mm**, B.Cu, rotation 90 degrees, with its mouth facing keyboard-left. Looking down from the switch side, pad centres are x=42.6mm and y=113.5,112.5,111.5,110.5mm for contacts 1-4. The [official WT60-A photo](https://keycult.com/products/wt60-a-pcb-60) supports a left-edge connector facing outward, but supplies no exact coordinates. Our position is an independent placement between the ortholinear switch/diode footprints, **not a measured WT60-A connector registration**. Case-channel alignment, mating clearance and the connector's full installed height below the PCB need physical checking. JST specifies a nominal 2.9mm body height; do not infer assembled clearance from the footprint courtyard alone.

J1's data pair runs on F.Cu along the left and top margins, above the In1.Cu ground pour, with short B.Cu escapes at J1, TP3/TP4 and R1/R2. Connector-side USB traces are 0.29mm wide; the paired F.Cu portions have a 0.2mm edge gap. VBUS retains its separate 0.4mm feed to the main-board fuse. The MCU-side USB traces and component placement are unchanged.

The 2026-10-30 resume review measured the saved native PCB segments in both variants. Their USB copper is identical. Lengths below are the sum of segment centreline lengths, including branches to test pads; they exclude pad traversal, via barrels and the external cable, so they are not end-to-end electrical lengths.

| Saved copper | D- | D+ |
| --- | --- | --- |
| J1-side net, before the series resistor | 151.4028mm | 157.3721mm |
| F.Cu portion of J1-side net | 146.5028mm | 150.4615mm |
| B.Cu portion of J1-side net | 4.9000mm | 6.9107mm |
| Vias on the J1-side net | 4 | 4 |
| MCU-side net, after the series resistor | 6.6253mm | 6.6253mm |

The J1-side segment-sum difference is 5.9693mm. These are measured routing properties, not acceptance limits or evidence of signal-integrity failure. A USB netclass records the 0.29mm width and 0.2mm gap for these two nets. The candidate dielectric model and the limits of the impedance calculation are described under [USB routing findings](#usb-routing-findings).

Strict validation refills zones and checks the full-width projection of every F.Cu USB segment against the saved In1.Cu GND polygons. Both variants have zero uncovered area outside explicit 0.51mm-radius disks around USB transition vias. Those disks exclude signal-via antipads, not unrelated voids elsewhere on the route. The measured minimum F.Cu pair edge gap is 0.2mm. The layer PDF supports inspection of the J1 escape, margin pair, test pads, resistor transitions and ground fill. These geometric checks do not qualify finite plane edges, transitions or cable discontinuities.

DRAFT E uses a regenerated base route for the current 1.5u M1/M2 coordinates; the 62-key DRAFT F base route also includes J2. After SES import, `scripts/usb_routing.py` applies the connector-side USB recipe and ground changes. Current validation uses KiCad 10.0.7 from the parent repository's pinned Nix input. The route manifest includes the USB script's hash. Native schematic parity, external DB1 netlist, coordinates and central reservations are checked again. Both variants have native PDF/SVG exports and validation reports.

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

**62-key DRAFT G implements a user-approved circuit assumption, not powered-assembly approval.** The horizontal PMW3360 module sits below the ball on the same rigid printed holder; its optical spacing and mounting dimensions remain prototype assumptions. J2 is a host-side JST GH 7-contact, 1.25mm-pitch harness header: `SM07B-GHS-TB(LF)(SN)`, stock footprint `Connector_JST:JST_GH_SM07B-GHS-TB_1x07-1MP_P1.25mm_Horizontal`, B.Cu, proposed origin `(136.5,111.3)mm`, rotation 0 degrees. Its cable housing is `GHR-07V-S`; the sensor-end connector remains unidentified. This does not claim that J2 mates directly with the sensor PCB. TP10-TP17 remain test access, while 65-key DRAFT E retains test pads without J2. The [pinned circuit review](sources.md#keyball-sensor-interface-evidence) covers the historical Keyball46 TPS73601/TXS0102 circuit and does not identify the user's module. No bare-PMW3360 circuit or cocot36plus main PCB is copied into this design.

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
- Keep GP23 / TP14 motion and GP24 / TP15 reset disconnected from this module. Its 7-pin interface exports neither signal; the published firmware polls motion and resets through SPI. Use SPI mode 3 with SCLK no greater than 2MHz, GP2/GP3/GP0 and GPIO CS GP1. Pin the chosen QMK revision and check its driver names, actual SPI divider, reset/SROM timing and sensor identification before claiming a working port. The existing example is only a pin-map aid.

The generator, BOM and interface validation record the assumed host connector and power route; regenerate the 62-key CAD and inspect native ERC/DRC and parity reports after changes. Bench acceptance includes approximately 1.9V sensor rails, safe host-side levels, startup/reconnect, sensor product ID and SROM, motion and USB suspend. None of these powered checks has been performed. The 70mA VDD and 60mA VDDIO supply-ramp limits may be summed only as a transient screening scenario; they exclude capacitor charging and are not a continuous module allowance. J1's USB routing remains electrically documented, with case and cable fit provisional.

## Geometry and construction assumptions

- Layout `(0,0)` maps to PCB **(180,100)mm**, +X right, +Y toward the bottom row. Coordinates come from `../preliminary/layout.json`, using `preferred.keys` for 65 keys and `rear_clearance.keys` for 62 keys.
- Stock `SW_Cherry_MX_1.00u_PCB` and `SW_Cherry_MX_1.50u_PCB` have their anchor at **electrical pin 1**, not the
  stem. The stem NPTH is local `(-2.54,+5.08)mm`; generation compensates and
  validation reads the actual stem holes. The base pitch is 19.05mm; the two bottom 1.5u key centres are 28.575mm apart.
- Both variants move the common MCU, flash, clock, power and interface-pad group 76.2mm left from the original draft, with decoupler positions adjusted separately. The original SW7-SW9 area contained essential electronics; the original board must not be cut to obtain the 62-key variant.
- The **285.0 × 94.6mm** working rectangle uses only scalar dimensions measured
  from the pinned GH60 PCB. It does **not** copy GH60's rounded corners/holes,
  any Keycult contour, or the 285.75×108.25mm plate reference rectangle.
- The 65-key all-copper-layer rule area is the centred **50 x 50mm** reservation. The 62-key rule area extends across the nominal empty key band, clipped at the board edge: PCB x=151.425..208.575mm, y=52.7..128.575mm, **57.15 x 75.875mm**. Both prohibit footprints, pads, tracks, vias and copper fill. The 62-key board separately has a **48 x 48mm, R1mm** rounded internal Edge.Cuts opening at PCB x=156..204mm, y=76..124mm, or layout x=-24..24mm, y=-24..24mm. The wider rule area is intentionally retained as a conservative reservation while preserving the existing signal routes. The square opening replaces DRAFT F's taller 48 x 60mm opening and follows the independent horizontal-module holder prototype, not a measured holder outline. The 65-key board has no internal opening. Holder passage, remaining-board support and fabrication suitability still need review.
- Four copper layers: F.Cu and B.Cu plus In2.Cu carry most signals. In1.Cu carries the GND reference pour outside the reserved centre, with short local +3V3 and COL12 routes beside the MCU. These routes stay away from the USB pair and the filled ground remains connected. Pad/via antipads and the central reservation still require return-path review; this is not an
  impedance-qualified stackup. Bottom SMT electronics coexist
  with top MX switch bodies, with actual drill/courtyard checks. Board thickness
  is nominally 1.6mm in the candidate model, not a verified case height requirement.
- Review rules are 0.15mm clearance/minimum trace, 0.55-0.6mm via copper diameter with 0.3mm drill, and 0.25mm
  hole/copper-edge clearance. These are **draft routing constraints, not a
  selected manufacturer's process approval**. 0.4mm pitch QFN fan-out, solder
  mask, assembly tolerances and stackup need explicit manufacturing review.
- USB routes need a paired 90Ω differential/return-path review against a chosen
  stackup. Mere routed connectivity or DRC clearance does not establish signal
  integrity. QSPI/crystal/decoupling placement and path lengths need layout
  engineering review before a fabrication release.
- [3D check record](reports/3d-review.json) describes the original draft only. Neither revised variant has component-height or case-assembly approval. The current CLI environment omits component 3D libraries.
- STEP models are linked from installed KiCad stock libraries; no Keycult case
  or sensor assembly STEP is redistributed. A valid 2D board does **not** prove
  Z-stack, actuator clearance, cable bend space, assembly access or case fit.

## Reproduction

The initial seed uses KiCad's maintained `pcbnew` API and installed stock
libraries. `sexpdata` serializes the native schematic with stock symbols and applies the deterministic USB overlay to native PCB data. No custom autorouter was written.
Freerouting performs local detailed routing via KiCad's Specctra DSN/SES API.
A small cleanup script removes only KiCad-reported dead-ended copper after zone refill; it never suppresses checks or unconnected nets. Native CAD retains the complete editable route. Each variant's saved SES records the base route for its current key coordinates. `scripts/usb_routing.py` applies the DRAFT E USB and ground changes after seed generation and SES import. Replaying the SES through `route_pcb.py` reproduces the combined route; rerunning the autorouter from a new seed does not replace layout review or guarantee completion.
See [sources](sources.md) for versions, licenses and hashes.

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
- Local In1.Cu exceptions use 0.15mm traces: +3V3 runs `(102.32,64) -> (103,64.25) -> (106.2,64.25) -> (106.979,65.6)`; COL12 escapes through `(102.35,67.25) -> (102.35,65.5) -> (101.8,64.8) -> (98.7,64.8)`, in PCB mm. The signal is a keyboard column, not USB or QSPI. These short routes trade local ground-plane area for feasible QFN escape routing; the connected pour and USB reference region must be preserved in later edits.
- Draft validation permits unfinished connections only. Any geometric DRC
  finding fails in both strict and draft modes.

## Release gates still open

Issues #1–#6 remain the mechanical/interface/bring-up gate. No Gerber fabrication
release, production BOM, identified sensor-end connector, qualified module-level translator, firmware
build, USB/keyboard/sensor bench test or assembly-fit approval is supplied.
Read the exact outstanding electrical/route counts in each variant's `reports/validation.json`;
a zero ERC or a matching netlist is not a manufacturing-ready assertion.
