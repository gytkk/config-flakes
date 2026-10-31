# /// script
# requires-python = ">=3.11"
# ///
"""Check circuit parity and exercise the actual keymap's scroll handling on the host."""

from pathlib import Path
import subprocess
import tempfile

from build_firmware import SOURCE, validate_map


validate_map()
with tempfile.TemporaryDirectory(prefix="keycult62-firmware-") as temporary:
    directory = Path(temporary)
    (directory / "qmk_stub.h").write_text("""
#include <stdbool.h>
#include <stdint.h>
#define SAFE_RANGE 0x5f00
#define PROGMEM
#define MATRIX_ROWS 5
#define MATRIX_COLS 15
#define LAYOUT(...) {{0}}
typedef struct { struct { bool pressed; } event; } keyrecord_t;
typedef struct { uint8_t buttons; int8_t x, y, v, h; } report_mouse_t;
""")
    (directory / "test.c").write_text(f'''
#include <assert.h>
#define QMK_KEYBOARD_H "qmk_stub.h"
#include "{SOURCE / 'keymaps/default/keymap.c'}"

int main(void) {{
    keyrecord_t press = {{.event.pressed = true}}, release = {{0}};
    report_mouse_t report = {{.buttons = 1, .x = 8, .y = -8}};
    assert(process_record_user(42, &press));
    report_mouse_t out = pointing_device_task_user(report);
    assert(out.x == 8 && out.y == -8 && out.buttons == 1);
    assert(!process_record_user(SCROLL, &press));
    out = pointing_device_task_user(report);
    assert(out.h == 0 && out.v == 0 && out.x == 0 && out.y == 0);
    out = pointing_device_task_user(report);
    assert(out.h == 1 && out.v == 1 && out.buttons == 1);
    report.x = -127; report.y = 127;
    out = pointing_device_task_user(report);
    assert(out.h == -7 && out.v == -7);
    report.x = -1; report.y = 1;
    out = pointing_device_task_user(report);
    assert(out.h == -1 && out.v == -1);
    assert(!process_record_user(SCROLL, &release));
    out = pointing_device_task_user(report);
    assert(out.x == -1 && out.y == 1);
    process_record_user(SCROLL, &press);
    report.x = 15; report.y = -15;
    out = pointing_device_task_user(report);
    assert(out.h == 0 && out.v == 0);
    process_record_user(SCROLL, &release);
    process_record_user(SCROLL, &press);
    report.x = 1; report.y = -1;
    out = pointing_device_task_user(report);
    assert(out.h == 0 && out.v == 0);
}}
''')
    subprocess.run(["cc", "-std=c11", "-Wall", "-Wextra", "-Werror", "-I", str(directory), str(directory / "test.c"), "-o", str(directory / "test")], check=True)
    subprocess.run([str(directory / "test")], check=True)
print("PASS: cursor mode, held scroll, signed fractional motion, buttons and release reset")
