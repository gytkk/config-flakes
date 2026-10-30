# Source provenance

Downloaded 2026-10-29 from the official Keycult plate files linked by:
<https://resources.keycult.com/downloads_plate_files.html>

Files are released by Keycult for **personal, non-commercial use**; personal modifications permitted; no warranty; commercial projects/group buys require written permission. Those terms do not explicitly grant public redistribution of original or modified CAD. This repository publishes source links, checksums, non-shape-replicating measurement summaries, historical scalar results and an independently authored layout. Public SVG/HTML/PNG/PDF use only a **generic 285.75 × 108.25mm rectangular plate bounding reference**, not the official contour, tabs or corner geometry. Original STEP/DXF/Gerber files and contour replicas are not bundled. Raw upstream downloads belong in ignored `upstream/`; extracted contour coordinates and new contour-check results belong in ignored `local/`, both relative to this directory. No blanket open-source license is asserted for third-party geometry.

Archive SHA-256:

- No160-Plate.zip: `3be328eba11d3477b3b38582f3f05cd27e3659f932a92da84854634483111a6d`
- No160-Rev1-Plates.zip: `515c2f6b745b6cd816b4747c9cfb575d02b4db2ce0aa1bff7950f1d48eab2f53`

Both at `https://resources.keycult.com/plate_files/<archive-name>`.

## Original extracted file hashes (not bundled)

- `No160-Plate/No160-Plate.dxf`: `cf77d96bb3a390868e65b2a3c16c4968322974e50347dd253ace4be5debb0031`
- `No160-Plate/No160-Plate.step`: `9d0aa3e4e43d59a3c7ac1414ee1ea803b7cc62734f6eddc49ea931962168344d`
- `No160-Rev1-Plates/Plate-Leaf.step`: `b038825c91d1d2fc3b710a22fd29a635f573bbdcea330b7618c9236ea5e35846`
- `No160-Rev1-Plates/Plate-NoLeaf.step`: `bf236229c06eff56717782319b435723cf583641a0352cc77b17366bb97ccd48`
- `No160-Rev1-Plates/Plate-Partial-NoLeaf.step`: `2fc7ae692e9e290e2fe10ae7fd0c432129ba683af3f8fd38a823b4b17d3de3fa`

## Public summaries and local contour reproduction

- [`measured-official-step.json`](measured-official-step.json) remains public: it contains bounding minima, maxima and lengths, not replicated shapes. Measurement used CadQuery `importers.importStep(...).val().BoundingBox()` (OpenCascade kernel). STEP declares millimetres. Native model y is thickness, x/z are plate plan axes. All four STEP models measure 285.75 × 108.25 × 1.5mm.
- `local/measured-official-dxf.json` is ignored: it contains the exact-contour extraction, including native bulge vertices and flattened points. The initial measurement used ezdxf `readfile`, verified `$INSUNITS=4`, and selected the first closed LWPOLYLINE as the outer contour. The reproduction script reads the DXF directly from its local ZIP, verifies the same units, and selects the largest-area valid closed LWPOLYLINE. Both flatten the contour via `ezdxf.path.make_path(...).flattening(0.01)`. The plot transform is `x=-x_DXF-142.875`, `y=y_DXF-47.625`; its registration still needs comparison with the physical plate.
- [`../plate-containment.json`](../plate-containment.json) is a **historical scalar record**, not a result recomputed from the public rectangle. The original check used Shapely polygon `covers` for every 14mm switch proxy and 18mm keycap proxy, plus rectangle distance to the polygon boundary. For the 66-key layout the recorded switch/keycap minima were 2.525/0.326654mm; for the 64-key alternative, 2.525/0.525mm. It contains no contour coordinates.
- Current public layout validation uses the generic rectangular bounding reference and planar clearance calculations only. `validation.json` holds the planar layout calculations; `envelope-containment.json`, both in the parent directory, holds the independently checked rectangular-reference results. **Passing a bounding-envelope check does not prove containment within the official contour**, and even a contour check does not prove passage through the case opening or assembly fit.

To reproduce the separate official-contour check, review the upstream terms and fetch the official archive containing the DXF with the existing `python3 scripts/fetch_sources.py` procedure. Then run `uv run scripts/verify_official_contour.py` from the repository root. That script re-extracts the contour from the local official DXF and writes coordinates, fresh verification results and `layout-overlay.svg` only under ignored `design/preliminary/sources/local/`. Public generation/validation do not require those coordinates and do not silently refresh the historical public record. See [reproduction.md](../../../docs/reproduction.md) for the complete procedure.

## Keyball primary evidence

Repository pinned at `e1d1948e6e60a83f6b3566a209326508200d64ce`. The sensor Gerber outline is from the Keyball46 2022 source; the vertical installation/34mm ball guide is Keyball44 Rev.1. These are explicitly not assumed to be a single fully measured modern assembly.

Archive SHA-256 (`Trackball_PCB_gerber.zip`): `2aa537eb96839a2ab50eac6131db09bdbac6ccb62818b79a50b4a7183c5faf9e`.

- `Keyball/ball_sensor-Edge_Cuts.gbr`: `f4ba35d5204a47a20cc252e9c543d26dfe9031c7eea7ea833a6a5fb753b3c584`
- `Keyball/buildguide-excerpt.txt`: `0b3d5671324d91acb8f329876f7250deaea2663f43832767bd94741b8c3ded88`

The guide excerpt was used in the initial local investigation but is not redistributed here; use the pinned source URL in `keyball-evidence.json`. No standalone holder geometry is included or inferred from keyboard-case STL dimensions.

Checksums describe the bytes examined during the initial investigation. The source archives can be fetched and hash-checked using `uv run scripts/fetch_sources.py` from the repository root. Upstream terms are summarized in [alternatives-and-licenses.md](../../../docs/alternatives-and-licenses.md).

## Plate revision research for local FreeCAD adaptation

Checked 2026-10-30. The [official download index](https://resources.keycult.com/downloads_plate_files.html) still links the [original No.1/60 ZIP](https://resources.keycult.com/plate_files/No160-Plate.zip) and [No.1/60 Rev.1 ZIP](https://resources.keycult.com/plate_files/No160-Rev1-Plates.zip). The index describes Rev.1 variants with leaf springs, without leaf springs and partial plates. The file hashes above identify the examined CAD; the index does not identify a newer NovelKeys-specific CAD revision. Personal modification and non-commercial restrictions remain applicable, so adapted FreeCAD documents and exports that reproduce upstream geometry belong under ignored `local/`.

The [official historical specification](https://resources.keycult.com/specs_no_1_60_65.html#no-160-and-165-rev-1) states that original and Rev.1 plates are compatible while other case components are not. Its Rev.1 changes include PTFE plate-alignment pins and leaf-spring plates. This compatibility statement covers those historical revisions; it does not establish compatibility with the owner's current NovelKeys case.

The [NovelKeys No.1/60 product page](https://novelkeys.com/products/keycult-no-1-60-keyboard) describes a re-release with separately configurable case, PCB and plate components. The [separate plate listing](https://novelkeys.com/products/keycult-no-1-60-plate) currently states sold out with no active restocks ordered and labels its images as renders. Neither examined product page supplies plate CAD, a numbered plate revision, or an explicit compatibility statement connecting the re-release to the historical ZIP files. The exact owned revision and its mounting compatibility therefore remain unconfirmed; product availability can change after this check.

For a local 65-key/62-key ortholinear adaptation, `Plate-NoLeaf.step` is a candidate donor for the case interface. Preserve its exact external mounting boundary and any alignment or mounting features when replacing the internal switch cutouts, then compare the resulting boundary against the imported solid. This is a modeling strategy, not a completed topology or fit verification: the donor must be inspected for internal mounting features before filling old cutouts. The STEP describes a plate, so it cannot establish the surrounding case opening, gasket compression, internal depth or assembly path. Central 34mm ball, holder and sensor clearance and support must be checked separately; the existing 50 x 50mm reservation is not a holder outline or an approved machining cutout. No new dimensions or physical-fit measurements were obtained in this source review.

## Keyball holder host interface review

Checked 2026-10-30 at the same pinned Keyball commit `e1d1948e6e60a83f6b3566a209326508200d64ce`. The complete repository tree contains keyboard-shell STL files, host PCB/plate files and the older sensor Gerber, but no standalone production-holder STEP/STL was found. The reuse conclusion and assembly constraints are recorded in [feasibility.md](../../../docs/feasibility.md#순정-keyball-홀더를-가공하지-않고-재사용하는-조건).

Additional inputs from [Keyball44 design_data](https://github.com/Yowkees/keyball/tree/e1d1948e6e60a83f6b3566a209326508200d64ce/keyball44/design_data) were checked against the Git blob hashes before extraction. Downloads and extracted files remain under ignored `upstream/keyball-holder-review/`.

| Input | SHA-256 |
| --- | --- |
| `keyball44_Right_KiCad.zip` | `d7a7cb8af5bb429e740ddf0d56cb31a22419f0ebe14da25a7fb1b41e09dca694` |
| `keyball44_right.kicad_pcb` | `8ec1fe263e89f5a22c1c7fcaaba4f2b67c5ffa1140859f7f82fa8016b3a93db1` |
| `Keyball44_BottomAcryl_2mm_156x186mm.zip` | `391a28ea344900111d1775bd3c570c16fa44d05eb5d253f4f366faad9bad1275` |
| `KB44_Bottom_2mm_156x186.dxf` | `46ec248eca814244d48ebbff85ca1ec88cf1413322ea06323f82526648768e8e` |

Measurement used ezdxf 1.4.3 `recursive_decompose` and entity bounding-box centers, and sexpdata 1.0.2 to read KiCad module centers in mm. The DXF declares `$INSUNITS=1` (inch), but its numerical dimensions match the mm-scale host PCB. Seven corresponding mounting centers, J5 through J11, were fitted with `x_DXF = s*x_PCB + tx` and `y_DXF = -s*y_PCB + ty`; the fitted scale is `1.0001977816` with a maximum center residual below `0.0001` drawing units. This cross-check establishes the nominal source geometry scale for comparison; it does not repair the upstream DXF or make it fabrication-ready.

The two bottom-plate holes next to the large circular access hole correspond to the holder attachment locations shown in [guide image kb44_132](https://github.com/Yowkees/keyball/blob/e1d1948e6e60a83f6b3566a209326508200d64ce/keyball44/doc/rev1/images/kb44_132.jpg). After registration their nominal diameter is about 2.5mm and center spacing about 16mm. These are host clearance holes, not holder thread dimensions. The larger access hole is about 12mm; its center is not a published ball-center datum. Exact extracted positions, registration and rendered source geometry remain in ignored `local/keyball-holder-review/host-interface-check.json` and `bottom-reference.svg`. No production-holder envelope, mounting-to-ball-center height, screw length or full optical stack was measured.
