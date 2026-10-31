// SPDX-License-Identifier: GPL-2.0-or-later
#include QMK_KEYBOARD_H

enum custom_keycodes { SCROLL = SAFE_RANGE };
static bool scrolling;
static int16_t scroll_x, scroll_y;

const uint16_t PROGMEM keymaps[][MATRIX_ROWS][MATRIX_COLS] = {
    [0] = LAYOUT(
        KC_ESC,  KC_1,    KC_2,    KC_3,    KC_4,    KC_5,       KC_6,    KC_7,    KC_8,    KC_9,    KC_0,    KC_BSPC,
        KC_TAB,  KC_Q,    KC_W,    KC_E,    KC_R,    KC_T,       KC_Y,    KC_U,    KC_I,    KC_O,    KC_P,    KC_BSLS,
        KC_CAPS, KC_A,    KC_S,    KC_D,    KC_F,    KC_G,       KC_H,    KC_J,    KC_K,    KC_L,    KC_SCLN, KC_ENT,
        KC_LSFT, KC_Z,    KC_X,    KC_C,    KC_V,    KC_B,       KC_N,    KC_M,    KC_COMM, KC_DOT,  KC_SLSH, KC_RSFT,
        KC_LCTL, KC_LGUI, KC_LALT, MO(1),   KC_SPC,  KC_DEL, KC_BTN1, KC_BTN2, KC_BSPC, KC_SPC, MO(1), KC_LEFT, KC_DOWN, KC_RGHT
    ),
    [1] = LAYOUT(
        KC_GRV,  KC_F1,   KC_F2,   KC_F3,   KC_F4,   KC_F5,      KC_F6,   KC_F7,   KC_F8,   KC_F9,   KC_F10,  QK_BOOT,
        _______, KC_F11,  KC_F12,  _______, _______, _______,    _______, KC_MINS, KC_EQL,  KC_LBRC, KC_RBRC, _______,
        _______, _______, _______, _______, _______, _______,  KC_LEFT, KC_DOWN, KC_UP,   KC_RGHT, KC_QUOT, _______,
        _______, _______, _______, _______, _______, _______,  _______, _______, _______, _______, _______, _______,
        _______, _______, _______, _______, _______, _______, KC_BTN3, SCROLL, KC_DEL, _______, _______, KC_HOME, KC_UP, KC_END
    )
};

bool process_record_user(uint16_t keycode, keyrecord_t *record) {
    if (keycode == SCROLL) {
        scrolling = record->event.pressed;
        scroll_x = scroll_y = 0;
        return false;
    }
    return true;
}

report_mouse_t pointing_device_task_user(report_mouse_t report) {
    if (scrolling) {
        // Keep fractional motion so slow ball movement still scrolls.
        scroll_x += report.x;
        scroll_y += report.y;
        report.h = scroll_x / 16;
        report.v = -scroll_y / 16;
        scroll_x %= 16;
        scroll_y %= 16;
        report.x = report.y = 0;
    }
    return report;
}
