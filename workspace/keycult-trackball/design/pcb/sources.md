# PCB source and tool provenance

Checked 2026-10-29. This is an independently authored electrical design and a
scalar working-envelope reference. **No Keycult restricted CAD, contour, STEP,
DXF, mounting-tab geometry, sensor Gerber or third-party original PCB is bundled.**
Original GH60 downloads are only in ignored `../preliminary/sources/upstream/gh60/`.

## GH60 scalar working envelope

- Author/project: komar007, GH60.
- Fixed commit: [`e7948cf44dbd3d43eb70296023a6fe3f471b44ba`](https://github.com/komar007/gh60/tree/e7948cf44dbd3d43eb70296023a6fe3f471b44ba).
- [PCB source](https://raw.githubusercontent.com/komar007/gh60/e7948cf44dbd3d43eb70296023a6fe3f471b44ba/keyboard.kicad_pcb): SHA-256 `a99aafb9f1ca77c5cac67037d161fa849b9b200de5dec9f63964520af3ff3daf`.
- [Schematic source](https://raw.githubusercontent.com/komar007/gh60/e7948cf44dbd3d43eb70296023a6fe3f471b44ba/keyboard.sch): SHA-256 `659cbbf3c524710a617a34f7e9e0e1f667a0dfc7f4fc812cc7136119a2a5d4a5`.
- [README](https://raw.githubusercontent.com/komar007/gh60/e7948cf44dbd3d43eb70296023a6fe3f471b44ba/README.md): SHA-256 `6167e6710f057082b5d1223cb28b95cfe735bf11d132adafb27dbcbe0bfd7946`.
- **License evidence and limitation:** the [designer's official project page](https://komar.blog/projects/gh60-programmable-keyboard/#downloads) labels Rev A/B/C design downloads **CC BY-SA 3.0**. The pinned repository has **no root LICENSE**, and README/CAD headers contain no explicit license notice. There is no invented LICENSE-file hash. This project does not redistribute the downloaded board/schematic or copy its footprint layout, traces, rounded contour or mounting holes; only its scalar envelope dimensions inform the new rectangle.
- Measurement: four Edge.Cuts lines and four arcs, centreline extent excluding
  graphic stroke, X 62.29…347.29mm and Y 64.62…159.22mm → **285.000×94.600mm**.
  Source thickness is 1.6002mm. Native source origin is not the new design origin.
- New board: a centred **285.0×94.6mm rectangle**, stock switch coordinates from
  the independent 65-key layout; no inherited GH60 mounting holes/USB cutout.
  This is distinct from the **285.75×108.25mm plate bounding reference**.

## Electrical references

- [Raspberry Pi, Hardware design with RP2040](https://datasheets.raspberrypi.com/rp2040/hardware-design-with-rp2040.pdf), §§2.1–2.4: power rails, decoupling, flash, USB, BOOTSEL and ABM8-272-T3 oscillator. Facts and recommended circuit values were consulted; no source CAD copied.
- [RP2040 datasheet](https://datasheets.raspberrypi.com/rp2040/rp2040-datasheet.pdf): pin functions, SPI0 GP0/1/2/3, TESTEN, USB and GPIO electrical limits.
- [W25Q16JV Rev D](https://www.winbond.com/resource-files/w25q16jv%20spi%20revd%2008122016.pdf): 2.7–3.6V, 16Mbit/2MiB, SPI/QSPI pinout; **SS = 208mil SOIC**, different from SI = 150mil. QE/boot-stage/part-ordering suffix must be checked before firmware/assembly.
- [AP2112 datasheet](https://www.diodes.com/assets/Datasheets/AP2112.pdf): 3.3V variant, enable tied to input, 1µF ceramic input/output support. Thermal dissipation and USB load remain unresolved system-level checks.
- [Bourns MF-PSMF datasheet](https://bourns.com/docs/product-datasheets/mf-psmf.pdf): MF-PSMF050X-2 is an 0805 resettable fuse. Both variants use `Fuse_0805_2012Metric`; USB budget and thermal behavior remain to be qualified.
- [QMK RP2040](https://docs.qmk.fm/platformdev_rp2040), [pointing-device driver](https://docs.qmk.fm/features/pointing_device#pmw-3360-and-pmw-3389-sensor): pin-map reference, not a copied firmware or claimed build result.
- Keyball published interface evidence and restrictions remain in the existing [design document](../preliminary/design.md) and [license comparison](../../docs/alternatives-and-licenses.md). The old Keyball46 connector is **not asserted as the actual sensor's pin order**.

## USB daughterboard evidence

Checked 2026-10-29. These sources establish an electrical design target, not the identity or measured fit of the user's installed daughterboard.

- [Wilba WT60-D at Salvun](https://salvun.com/products/wilba-60-pcb-wt60-d): describes its JST footprint as Unified Daughterboard-compatible and explicitly lists Keycult No.1/60 support after removing USB-C and soldering JST. This is the primary bridge between No.1/60 and the UDB electrical interface.
- [Keycult WT60-A](https://keycult.com/products/wt60-a-pcb-60) and its official product photograph: a left-edge, outward-facing main-board connector is visible. No exact connector coordinates or cable length are provided. The new `(40.6,112.0)mm` position is independently selected for the ortholinear PCB, not measured from this photograph.
- [NovelKeys No.1/60 replacement PCB](https://novelkeys.com/products/keycult-no-1-60-pcb): its pictured board differs from the older red WT60-A. No owned-revision identification or pinout guarantee is inferred from the replacement product name.
- [UDB PCB-side specification](https://github.com/Unified-Daughterboard/UDB-C-Legacy/wiki/PCB-side-design-resources): JST `SM04B-SRSS-TB(LF)(SN)`, 1:1 contact-number cable and identical main-board/daughterboard pinout. This archived specification is used for legacy Wilba/Keycult compatibility, not to propose replacing the installed daughterboard with a newly manufactured C3.
- [UDB C3 schematic](https://github.com/Unified-Daughterboard/UDB-C-Legacy/blob/2ecdd7e0482ecbd3ffe9af04966ab3555f7fc33c/KiCad/C3/Unified-Daughterboard.sch), commit `2ecdd7e0482ecbd3ffe9af04966ab3555f7fc33c`: J2 maps 1=VCC, 2=DN, 3=DP, 4=GND. Original Git blob `86f8bea12f75fce34009c3bc10643d38e6eaba47`; downloaded bytes SHA-256 `7d150e1e955eca3ec802049248848f61f5315a891d0e1c15e4abd5037f3fe6fb`. Local original is ignored under `../preliminary/sources/upstream/udb-c3/`. The repository is [MIT, copyright 2019 ai03](https://github.com/Unified-Daughterboard/UDB-C-Legacy/blob/2ecdd7e0482ecbd3ffe9af04966ab3555f7fc33c/LICENSE). No source CAD, artwork or layout is redistributed; the new connector uses stock KiCad symbols/footprints and independently authored copper.
- [JST SH series datasheet](https://www.jst-mfg.com/product/pdf/eng/eSH.pdf), pages 1-3: 1.00mm pitch, side-entry header and mounting-surface pin numbering; `SHR-04V-S` mating housing and nominal 2.9mm header body height. Installed height and cable clearance still require a case model or measurements.
- C3's CC/ESD/overvoltage/fuse/shield implementation is evidence about **C3 only**. It is not evidence that the unidentified Keycult daughterboard carries those components. The main-board fuse remains, and protection coordination is an open assembly requirement.

### WT compatibility and revision-specific protection

Checked 2026-10-30. Cable reach is not a design blocker: a sufficiently long replacement cable can be sourced. Contact numbering, connector type, insertion clearance and the installed daughterboard's identity still need to match the assembly.

- [WT60-C official product FAQ](https://dailyclack.com/products/wt60-c-kate-60-pcb) explicitly identifies the UDB interface and `SM04B-SRSS-TB(LF)(SN)` connector. Together with the WT60-D No.1/60 statement above, this supports the current main-board JST-SH interface as a WT-family-compatible target.
- [Keycult's FAQ](https://resources.keycult.com/faq_keycult_faq.html) recommends WT60-G; [Keycult Graphite PCBs](https://keycult.com/products/graphite-pcbs) includes a daughterboard, and [Salvun's Graphite product page](https://salvun.com/products/wilba-graphite-pcb) explicitly lists Keycult 60% compatibility. These product statements do not publish the factory daughterboard's protection schematic or establish that it is a UDB C3/C5.

The following summarizes inspected official sources, not measurements of the installed board. J2 in both the C3 and C5-JSH schematics maps **1=protected USB power, 2=DN/D-, 3=DP/D+, 4=GND** (`VCC` in C3, `VCOM` in C5). The cable connects contact numbers 1:1; visual left-to-right order changes with the connector viewing direction.

| Revision and pinned evidence | Connector and protection circuit |
| --- | --- |
| [C3 schematic, `2ecdd7e0482ecbd3ffe9af04966ab3555f7fc33c`](https://github.com/Unified-Daughterboard/UDB-C-Legacy/blob/2ecdd7e0482ecbd3ffe9af04966ab3555f7fc33c/KiCad/C3/Unified-Daughterboard.sch) | JST-SH `SM04B-SRSS-TB(LF)(SN)`; R1/R2 5.1k CC pull-downs; U1 `TPD4E05U06DQAR` data-line ESD array; F1 `ASMD1206-150` resettable PTC; D1 `SMF9.0CA` bidirectional shunt TVS after the fuse; L1 `60R@100MHz` between shield/chassis ground and signal ground. |
| [C4-era schematic, `6be39c7f925237a2e7d53eb9ec0c053175fde3e7`](https://github.com/Unified-Daughterboard/UDB-C-JSH/blob/6be39c7f925237a2e7d53eb9ec0c053175fde3e7/UDB.kicad_sch) | Pico-EZmate-family interface, with XKB `X1224WRS-04-LPV01` specified in this file; U1/F1 retained, D1 becomes `SMF6.0A`, and the ferrite bead remains. The source title says `C4-RC1`, so this commit/file identifies the inspected circuit more precisely than its title. |
| [C5-JSH release schematic, `2b6c52aa2de6f9f60ca99c6250e4bdf01e8f4eea`](https://github.com/Unified-Daughterboard/UDB-C-JSH/blob/2b6c52aa2de6f9f60ca99c6250e4bdf01e8f4eea/UDB.kicad_sch) | JST-SH `SM04B-SRSS-TB(LF)(SN)`; 5.1k CC pull-downs, `TPD4E05U06DQAR`, `ASMD1206-150` and `SMF6.0A`; no ferrite bead. The release is C5 although the schematic title still says `V4.2`. |

These circuits provide transient clamping and PTC overcurrent protection. **None of the inspected schematics includes an active sustained-overvoltage disconnect switch.** The legacy README's term "overvoltage protection" describes its shunt TVS; it does not establish a regulated 5V ceiling or guaranteed isolation from a sustained incorrect supply. TI describes [TPD4E05U06](https://www.ti.com/product/TPD4E05U06) as a TVS-based ESD protection diode array, not a power isolation controller. The main-board fuse does not establish those additional protections either.

The [C5 release announcement, documentation commit `238c6fd99a00859b751757422ce0158f7cb11438`](https://github.com/Unified-Daughterboard/unified-daughterboard.github.io/blob/238c6fd99a00859b751757422ce0158f7cb11438/announcements/announcement-20231022.md) explains the shield change: C3/C4 used a ferrite bead, whereas C5 directly joins shield/chassis and signal ground through a controlled ground region. C5 also offers both JST-SH and Pico-EZmate variants. Therefore, the older legacy page's claim that C3 was the last JST version is superseded by the C5 release. C5-JSH is the relevant current standard candidate for the existing JST interface; C5-EZM requires a different mating connector/cable. This is electrical-interface evidence, not confirmed Keycult case fit.

The [pinned legacy README](https://github.com/Unified-Daughterboard/UDB-C-Legacy/blob/2ecdd7e0482ecbd3ffe9af04966ab3555f7fc33c/README.md) states that C3 retains C1/C2 dimensions and mounting-hole positions. That backward-compatibility statement does not establish C1/C2 protection circuitry; no C1/C2 schematic or exact factory Keycult daughterboard schematic was established in this investigation.

Downloaded schematic originals are ignored under `../preliminary/sources/upstream/udb-protection/`; no original CAD is redistributed. SHA-256: `c3.sch` = `7d150e1e955eca3ec802049248848f61f5315a891d0e1c15e4abd5037f3fe6fb`; `c4.kicad_sch` = `30e8cc0061e69c6cf81bade6fb7cbf2365a4e2df2c5b66107631c401e8d067eb`; `c5.kicad_sch` = `91b0aa1959e195d3565556cdd9dfcd7efa5a4db78779df2ce7dc54f18d92a552`. URLs and revisions are the linked table entries. The current repository's [license](https://github.com/Unified-Daughterboard/UDB-C-JSH/blob/2b6c52aa2de6f9f60ca99c6250e4bdf01e8f4eea/LICENSE) and the legacy license are MIT; only independently worded interface/circuit summaries are added here.

## Maintained tools and stock libraries

- DRAFT E was regenerated and validated on 2026-10-30 with official `kicad/kicad:10.0.7`, Linux amd64, Python 3.13.5 and its stock symbol/footprint libraries. The image is pinned by manifest digest `sha256:3224d1bf143185ecc1bd3a1082a36f37f0f4e59d707983a16515a845e3737fc6`; the [Docker procedure](../../docs/reproduction.md#docker에서-kicad-검증) records the runtime dependencies. This image has no 3D model package. The following Nix versions record the earlier environment.
- **KiCad 10.0.7** GUI/CLI and its installed Python `pcbnew` binding, Python 3.14.7.
  Native ERC, DRC, zone refill, schematic parity, XML netlist, PDF/SVG exports and
  Specctra transfer are performed by KiCad, not fabricated report text.
- Installed **kicad-symbols revision `1b2b42d49b`**, **kicad-footprints revision
  `25a0b01dd7`**, **kicad-packages3d revision `dd37eac8d2`** (Nix package labels).
  Project tables use `${KICAD10_SYMBOL_DIR}` / `${KICAD10_FOOTPRINT_DIR}`; stock
  symbols are embedded in the schematic and stock footprints in the PCB.
  No global library configuration is modified.
- [KiCad library license](https://www.kicad.org/libraries/license/): CC BY-SA 4.0
  with the KiCad library exception for use in electronic designs. Embedded stock
  content is attributable to the KiCad library contributors; this does not
  relicense third-party CAD or assert a blanket license over this repository.
- `sexpdata 1.0.2`: maintained S-expression serialization; `shapely 2.1.2`:
  independent geometry checks; `pymupdf 1.26.7`: readback/raster inspection of the
  actual KiCad-generated PDF. Their dependencies are declared in scripts.
- [Freerouting **v2.0.1**](https://github.com/freerouting/freerouting/releases/tag/v2.0.1), GPL-3.0 tool, run locally with Java 21. The installed Java21 cannot execute the newer Java25-built v2.5.0, so v2.0.1 is explicitly pinned. The JAR is ignored, not bundled. Usage analytics are disabled via `--usage_and_diagnostic_data.disable_analytics=true`; no cloud routing service is used. Routing output is our board data, not copied router source.

The exact router artifact checksum and route-snapshot checksums are recorded in
[route-manifest.json](route-manifest.json). Saved SES files contain only this
independent design's editable copper connectivity. Hash mismatches must be
investigated; do not silently replace pinned inputs.

The 62-key route has its own [manifest](62/route-manifest.json). The router wrapper rejects empty/invalid SES output before replacing a saved route and retries up to three times for the v2.0.1 CLI export race: its CLI observes `COMPLETED` before the scheduler has necessarily populated the output bytes. The upstream [CLI](https://github.com/freerouting/freerouting/blob/v2.0.1/src/main/java/app/freerouting/Freerouting.java) and [scheduler](https://github.com/freerouting/freerouting/blob/v2.0.1/src/main/java/app/freerouting/management/RoutingJobScheduler.java) establish this ordering. The tool JAR is unchanged.

## Electrical desk review sources

### DRAFT D power evidence and calculation boundaries

Checked 2026-10-30 against `circuit.json` and the unmodified external C5-JSH circuit. These are source limits and engineering calculation inputs, not measured consumption or assembly qualification. Main-board F1 and external daughterboard F1 are in series. The sensor remains unspecified, the sensor pads expose +3V3, and no sensor 5V connector or CC current detection circuit is implemented.

| Primary source and revision | Confirmed numerical inputs and limits |
| --- | --- |
| [Diodes AP2112, DS39724 Rev. 2-2, June 2017](https://www.diodes.com/assets/Datasheets/AP2112.pdf), pp. 3, 8, 10 | SOT25 junction-to-ambient thermal resistance is 184 C/W, described only as "No Heatsink"; board copper and airflow are not specified. Junction absolute maximum is 150 C; recommended ambient is -40 to +85 C. AP2112-3.3 has maximum dropout 200mV at 300mA and 400mV at 600mA, with the electrical table referenced to VIN=4.3V. Output accuracy is +/-1.5% at 1-30mA. Maximum no-load quiescent current is 80uA; loaded ground current has a typical graph, not a guaranteed maximum. The 600mA output rating does not establish acceptable dissipation in this PCB. |
| [Bourns MF-PSMF, Rev. Q 09/26](https://bourns.com/docs/product-datasheets/mf-psmf.pdf), pp. 1-2, 5 | MF-PSMF050X: 6V rating, hold 0.50A and trip 1.00A at 23 C; resistance Rmin=0.15ohm and R1max=0.90ohm at 23 C. Hold current decreases to 0.40/0.37/0.33/0.29/0.23A at 40/50/60/70/85 C. This is a resettable PTC, not a 500mA precision current limiter. |
| [JDT Fuse ASMD1206, revised 2025-09-29](https://www.jdtfuse.com/upload/PDF/ASMD1206.pdf), printed pp. 3-4 of 9, PDF pp. 4-5 | ASMD1206-150: 6V, hold 1.50A and trip 3.0A at 25 C; Rimin=0.025ohm, R1max=0.13ohm. The source defines R1max one hour after reflow. Hold is 1.17A at 50 C and 0.77A at 85 C. Matching an external assembly to this manufacturer's part remains a procurement check. |
| [Winbond W25Q16JV, Rev. D, 2016-08-12](https://www.winbond.com/resource-files/w25q16jv%20spi%20revd%2008122016.pdf), sections 9.2, 9.4, printed pp. 56, 58 | Supply is 2.7-3.6V. Maximum read currents are 10/15/18/20mA at 1/50/80/104MHz with open DO and a checkerboard pattern; program, erase and status-write maximum is 25mA. Standby maximum is 50uA; power-down maximum is 15uA. The 25mA allowance does not establish a maximum at uncharacterized read frequencies or output loads. |
| [RP2040 datasheet, build 2025-02-20](https://datasheets.raspberrypi.com/rp2040/rp2040-datasheet.pdf), sections 5.6-5.7, printed pp. 622-624 | USB_VDD requires 3.135-3.63V, setting a higher minimum than the flash's 2.7V requirement. Table 637 maximum-average currents apply to its particular software cases and average over seconds. Figure 172 characterizes a typical device running FFT. Neither establishes a maximum for this keyboard firmware, GPIO loads or sensor operation. Internal core-regulator input current must be included in a +3V3 MCU allowance. |

For an initial calculation, 150mA total +3V3 load is a provisional engineering envelope. Allocating 80mA to the MCU, 25mA to flash and 5mA to other main-board loads leaves 40mA for a future 3.3V sensor. The MCU and miscellaneous allocations are assumptions that require firmware-specific validation; they are not datasheet maxima or evidence that any particular sensor fits. USB enumeration, startup and suspend require separate budgets. A 110mA main-board allocation already exceeds the unconfigured USB 2.0 limit.

For thermal screening at VIN=5.50V and nominal VOUT=3.30V, neglecting upstream resistance gives LDO dissipation of 0.220/0.330/0.440/0.660/1.100W at 100/150/200/300/500mA, before ground-current power. Using the source's 184 C/W yields rises of 40.5/60.7/81.0/121.4/202.4 C. At an assumed 50 C ambient, 150mA predicts approximately 111 C, whereas 200mA predicts 131 C. A chosen 125 C planning ceiling permits approximately 185mA before ground-current power. This ceiling is an engineering margin below the absolute maximum, and the thermal estimate must be replaced by board-specific evidence before release.

The series PTC reference-temperature R1max sum is 1.03ohm, giving calculated drops of 0.103/0.155/0.206/0.309/0.515V at those currents. This sum is not a guaranteed hot resistance. Cable, connector and PCB drops are additional. At a 4.40V low-power input and 100mA, the reference PTC estimate leaves 4.297V before those additional drops. At a 4.75V high-power input and 150mA it leaves 4.596V. Regulated-operation headroom is plausible, but neither result proves transient regulation or a guaranteed hot dropout margin.

[USB-IF USB 2.0 specification package, 2025-06-03](https://www.usb.org/sites/default/files/usb_20_20250603.zip) supplies the base specification and ECNs. Base sections 7.2.1 and 7.2.1.4 require at most 100mA before configuration and permit up to 500mA only with the appropriate configured power budget. Section 7.2.4.1 defines the attach load as at most 10uF in parallel with 44ohm, including capacitance visible through regulators. Suspend Current ECN, p. 2, permits 2.5mA total averaged over any 1s interval, including USB pull-up/pull-down current. VBUS Max Limit ECN raises the maximum to 5.50V. The two 5.1k CC resistors do not establish permission to consume 1.5A or 3A; without current detection, this analysis retains USB 2.0 power limits.

The [USB-IF electrical compliance updates](https://compliance.usb.org/index.asp?UpdateFile=Electrical), "Inrush Current Test Description" effective June 2021 and "(DEPRECATED) ECN: Minimum Capacitance Required on Vbus", supersede the older Device Capacitance ECN's 1uF minimum. That minimum was deprecated effective January 2019 and is not treated as a current defect criterion. Current inrush assessment captures at least 100ms after attach at at least 1MS/s and analyzes charge above 100mA in each region. Nominal capacitor addition alone cannot prove compliance: regulator startup, active load and effective capacitance still need evaluation. C1's effective value remains relevant to the AP2112's local input decoupling requirement.

Downloaded originals and text extracts remain ignored under `../preliminary/sources/upstream/electrical-review/`. SHA-256 pins: `ap2112.pdf` = `ef8d376f2ec356e29172eb9e053819a0ebdcc576dba7fc9ab0505c568427920f`; `bourns.pdf` = `8169931cf315787976c52f4531ac4db63af171ab19083f2d8bf5f8597d5fbf63`; `jdt-current.pdf` = `1c06a6485310e9753fabc4628628bce43a79aff847f5f8abbc3fff0bf9f93a4d`; `winbond.pdf` = `97430b7ae9fc375867bdbc6ad4782c48e21d15bbe12cb3293ae0b331e3ad1a5c`; `rp2040.pdf` = `be56fbb75ba0ae9e26558a73c93ac3e75c2ad4e6878d3b6703de2a76d886ea8c`; `usb20.zip` = `5fe9c53c04033818af396e8852b3acbca5c3a76ba92fab549fd81cd0ea7b3692`; extracted `usb_20.pdf` = `d39698a33486c399124af92bd02e4f978fd9a836b5cf4e52e6e4633eb1d89f61`. An earlier [manufacturer-authored JDT mirror](https://www.fuse-tech.com/uploads/pdf/JDT-ASMD1206.pdf), revised 2018-06-21, was also retrieved as `jdt.pdf`, hash `4d47577f072fbfed41f2816754e37fc177bbaaa3704f218ec1f36b3671eb7160`; the findings above use the current manufacturer-hosted source.

### USB stackup screening

Checked 2026-10-30. The [JLCPCB controlled-impedance stackup table](https://jlcpcb.com/impedance) publishes these 1.6mm, four-layer candidates. DRAFT E records the 7628 copper/dielectric model in both native PCBs; 3313 remains a comparison. Neither is an approved manufacturing order. The model omits solder mask construction and is not an impedance qualification.

| Candidate | Outer copper | Inner copper | Outer-to-inner dielectric | Inner-to-inner dielectric | Prepreg dielectric constant |
| --- | --- | --- | --- | --- | --- |
| JLC04161H-7628 | 0.035mm | 0.0152mm | 0.2104mm | 1.065mm | 4.4 |
| JLC04161H-3313 | 0.035mm | 0.0152mm | 0.0994mm | 1.265mm | 4.1 |

[TI SLVA531A, September 2013, section 7.1, p. 15](https://www.ti.com/lit/an/slva531a/slva531a.pdf) supplies the screening equations `Z0 = 87/sqrt(Er+1.41) * ln(5.98H/(0.8W+T))` and `Zdiff = 2Z0 * (1-0.48exp(-0.96S/H))`. W is trace width, S edge gap, H dielectric height and T copper thickness, in consistent units. These are approximate microstrip equations from an Ethernet layout guide, not USB compliance criteria. Here they screen a uniform outer F.Cu pair above In1.Cu. They exclude mask, etched trapezoid, finite plane edges, coplanar copper, tolerance and transitions, including the short B.Cu escapes. Their result does not qualify the actual routed path.

[Raspberry Pi Hardware design with RP2040, release 2, build 2026-08-20, section 2.4.1, p. 11](https://datasheets.raspberrypi.com/rp2040/hardware-design-with-rp2040.pdf) specifies 27ohm resistors near the chip, approximately 90ohm differential traces and uninterrupted reference ground for full-speed USB. It does not make the entire path compliant solely because its data rate is 12Mbps. The candidate calculations, native layer-length measurements and remaining limitations are in the [electrical review](reports/electrical-review.json) and [PCB guide](README.md#power-and-usb-desk-review).
