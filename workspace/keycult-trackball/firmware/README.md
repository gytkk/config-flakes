# 62-key QMK prototype

This port builds an RP2040 USB keyboard and polled PMW3360 pointing device for the [62-key circuit](../design/pcb/62/circuit.json). Compilation, UF2 structure, electrical-map parity and host tests of scroll handling are checked. No board has been flashed or tested, and this firmware does not establish assembly, optical, power or USB compliance.

## Build and check

Run from the project root. The following command uses temporary Nix tools without changing the system configuration or repository flake. It requires network access on the first run, Git, Nix and uv. Python 3.12 and QMK CLI 1.1.8 are managed by uv; the selected Nix package supplies Arm GNU Toolchain 13.3.Rel1.

```bash
nix shell \
  github:NixOS/nixpkgs/885ba4d038f10c3d17d7a92e8c3cc185b99c9158#gcc-arm-embedded-13 \
  github:NixOS/nixpkgs/885ba4d038f10c3d17d7a92e8c3cc185b99c9158#gnumake \
  --command uv run --python 3.12 scripts/build_firmware.py
uv run scripts/test_firmware.py
```

[The build script](../scripts/build_firmware.py) checks all 62 matrix positions against the GPIO CSV, switch/diode connections and sensor J2, then fetches QMK 0.29.0 at commit `75a037d2f0f9c54bad8e2f68ae82134559c92f22` into ignored `.cache/qmk_firmware/`. It verifies that the compiled submodules match their pinned commits and contain no tracked edits, copies this keyboard to `keyboards/handwired/keycult62/`, and runs `make handwired/keycult62:default SKIP_VERSION=yes`. Skipping the embedded build version avoids date-dependent version strings; the receipt records the actual revision. Preserve and move an existing checkout if the script reports an unexpected revision or source edits.

The output is `.cache/firmware/keycult62_default.uf2`; `.cache/firmware/build.json` and [build-validation.json](build-validation.json) record the compiler, source and binary hashes. The UF2 checker verifies all block headers, addresses within the 2MiB flash and RP2040 family ID. The script never flashes hardware. `uv run scripts/build_firmware.py --check` performs only circuit parity checks; the host tests compile the actual keymap scroll functions against a small QMK type stub using `cc` and check signed motion, fractional accumulation, mouse buttons and mode release.

The build also creates `.cache/firmware/keycult62-corresponding-source.tar.gz` containing the complete matching QMK tree and checked-out libraries, this port and upstream license/NOTICE files. Git metadata and intermediate objects are excluded. After extracting the archive, use the same Nix toolchain, run from its `qmk_firmware/` directory and execute `QMK_HOME="$PWD" uv run --no-project --python 3.12 --with qmk==1.1.8 make handwired/keycult62:default SKIP_VERSION=yes`. The original script remains the normal repository build entry point; the archive command builds directly from the included source without fetching QMK.

## Wiring and assumptions

| Function | MCU pins or setting |
| --- | --- |
| Matrix rows 0-4 | GP4, GP5, GP6, GP7, GP8 |
| Matrix columns 0-14 | GP9, GP10, GP11, GP12, GP13, GP14, GP15, NC, GP17, GP18, GP19, GP20, GP21, GP22, GP26 |
| Diodes | COL2ROW: switch to diode anode, cathode to row |
| J2 contacts | 1 GP2/SCLK, 2 GP1/CS, 3 GND, 4 +3V3, 5 GND, 6 GP3/MOSI, 7 GP0/MISO |
| Sensor SPI | SPI0/SPID0, mode 3, divisor 64, nominal 125MHz/64 = 1.953125MHz |
| Sensor settings | 800 CPI, rotation 0 degrees, QMK default lift-off value 0x02 |
| MCU clock and flash | 12MHz crystal; W25Q16JVSSIQ 2MiB; generic serial 03h boot stage, nominal 125MHz/4 = 31.25MHz flash reads |
| USB descriptor | Prototype `FEED:0062`, version 0.0.1; 250mA after configuration |

Column 7 is `NO_PIN`, leaving GP16 untouched, and every absent key is `KC_NO` in QMK's generated layout. GP23 motion and GP24 reset are test pads only and are not used by this firmware or exported on J2. The driver polls motion and sends register `Power_Up_Reset=0x5a` through SPI; it does not require a wired sensor reset pin. RAM-backed transient EEPROM avoids firmware configuration writes to flash; settings are lost on power-off.

The [PCB interface conditions](../design/pcb/README.md#sensor-interface-remains-unresolved) still apply. J2 +3V3 is only the documented conditional feed for the reviewed historical Keyball circuit; identify the actual module and harness before power-on. RP2040 pins are not 5V tolerant. The nominal SPI clock is a source-derived calculation, not a cable timing measurement. Zero rotation and the lack of axis inversion are initial assumptions: check physical ball-right and ball-up movement, then set QMK's `POINTING_DEVICE_ROTATION_90/180/270` or `POINTING_DEVICE_INVERT_X/Y` in `config.h` as required and rebuild. Optical spacing, CPI and lift-off need adjustment on the assembled holder.

`FEED:0062` is a temporary local prototype identifier, not an assigned USB vendor/product claim or a Keycult product identifier. Obtain suitable identifiers before distribution. Declaring 250mA does not limit actual current or establish startup, pre-configuration or suspend-current compliance. This port does not implement sensor rail switching or sensor suspend power management; measure and resolve these before release.

## Controls and bootloader

The base layer follows the labels in the GPIO CSV. M1 is left click and M2 is right click. Either Fn key holds the second layer:

- Fn+M1 is middle click. Hold Fn+M2 and roll the ball to scroll horizontally and vertically, at 16 motion counts per wheel tick. Release M2 to return to cursor mode.
- Fn+1 through Fn+0 send F1-F10; Fn+Q/W send F11/F12. Fn+Esc sends grave, Fn+U/I send minus/equal, Fn+O/P send left/right bracket and Fn+semicolon sends quote.
- Fn+H/J/K/L send left/down/up/right. Fn+bottom Left/Down/Right send Home/Up/End.
- Fn+the top-right Backspace enters the UF2 bootloader. Holding Esc while connecting USB invokes bootmagic. Double-tapping RESET within 200ms also enters the bootloader once this firmware is installed.

For the first flash or recovery, hold BOOTSEL SW67, connect USB or press and release RESET SW68, then release BOOTSEL. The ROM bootloader should expose `RPI-RP2`. Copy the UF2 there only after the circuit, module supply and harness have passed the hardware bring-up checks. No automatic flashing is provided.

## Driver evidence and licensing

The port uses QMK's existing driver without copying cocot or Keyball firmware. Version-matched sources are [RP2040 configuration](https://github.com/qmk/qmk_firmware/blob/75a037d2f0f9c54bad8e2f68ae82134559c92f22/docs/platformdev_rp2040.md), [SPI divisor handling](https://github.com/qmk/qmk_firmware/blob/75a037d2f0f9c54bad8e2f68ae82134559c92f22/platforms/chibios/drivers/spi_master.c), [PMW33xx reset, timing and product-ID checks](https://github.com/qmk/qmk_firmware/blob/75a037d2f0f9c54bad8e2f68ae82134559c92f22/drivers/sensors/pmw33xx_common.c), [PMW3360 driver](https://github.com/qmk/qmk_firmware/blob/75a037d2f0f9c54bad8e2f68ae82134559c92f22/drivers/sensors/pmw3360.c), [boot stage bytes](https://github.com/qmk/qmk_firmware/blob/75a037d2f0f9c54bad8e2f68ae82134559c92f22/platforms/chibios/vendors/RP/stage2_bootloaders.c) and the pinned Pico SDK's [clock setup](https://github.com/qmk/pico-sdk/blob/d0c5cac430cc955b65efa0e899748853d9a80928/src/rp2_common/hardware_clocks/clocks.c) and [generic 03h boot stage](https://github.com/qmk/pico-sdk/blob/d0c5cac430cc955b65efa0e899748853d9a80928/src/rp2_common/boot_stage2/boot2_generic_03h.S).

QMK [removed optical sensor SROM blobs because of licensing ambiguity](https://docs.qmk.fm/ChangeLog/20241124#adns9800-and-pmw33xx-sensor-firmware-rom-removal-24428). This build follows QMK 0.29.0's default and uploads no SROM. The driver validates product IDs 0x42/0xBD, but this does not validate SROM, optics or tracking. Startup, power-cycle/reconnect, product ID, raw motion in both axes, tracking quality, clicks, scrolling and USB power states remain bench acceptance tasks. If the actual module needs a vendor SROM, obtain a suitably licensed image and implement QMK's `pmw33xx_srom_get_length`/`pmw33xx_srom_get_byte` hooks before claiming sensor readiness.

New C/configuration files carry `GPL-2.0-or-later`. QMK's [GPL text](https://github.com/qmk/qmk_firmware/blob/75a037d2f0f9c54bad8e2f68ae82134559c92f22/LICENSE) is copied next to the local UF2 as `QMK-LICENSE.txt`; upstream libraries keep their individual license notices. The binary and full upstream checkout are ignored local outputs. If distributing a UF2, provide its complete corresponding source, this port and build recipe, the exact upstream/submodule revisions, and applicable license/NOTICE files. Do not treat a UF2 and a bare upstream link as a complete distribution package.
