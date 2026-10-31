// SPDX-License-Identifier: GPL-2.0-or-later
#pragma once

#define SPI_DRIVER SPID0
#define SPI_SCK_PIN GP2
#define SPI_MOSI_PIN GP3
#define SPI_MISO_PIN GP0
#define PMW33XX_CS_PIN GP1
#define PMW33XX_SPI_DIVISOR 64
#define PMW33XX_CPI 800
#define ROTATIONAL_TRANSFORM_ANGLE 0

// Serial 03h reads avoid relying on the flash's quad-enable state.
#define RP2040_FLASH_GENERIC_03H
#define PICO_FLASH_SIZE_BYTES (2 * 1024 * 1024)
#define RP2040_BOOTLOADER_DOUBLE_TAP_RESET
#define RP2040_BOOTLOADER_DOUBLE_TAP_RESET_TIMEOUT 200U
