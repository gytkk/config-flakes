# /// script
# requires-python = ">=3.11"
# dependencies = ["playwright==1.56.0"]
# ///
"""Render existing self-contained HTML/SVG with system Chromium."""
from pathlib import Path
import os
import shutil
from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parents[1]
DESIGN = ROOT / 'design/preliminary'
chromium = os.environ.get('KEYCULT_CHROMIUM') or shutil.which('chromium') or shutil.which('chromium-browser')
if not chromium:
    raise SystemExit('Install Chromium, or set KEYCULT_CHROMIUM to its executable path.')
with sync_playwright() as playwright:
    browser = playwright.chromium.launch(executable_path=chromium, headless=True)
    page = browser.new_page(viewport={'width': 1500, 'height': 1100}, device_scale_factor=1)
    page.goto((DESIGN / 'layout.html').as_uri(), wait_until='load')
    page.locator('.panel').first.screenshot(path=str(DESIGN / 'layout.png'))
    page.goto((DESIGN / 'section.svg').as_uri(), wait_until='load')
    page.locator('svg').screenshot(path=str(DESIGN / 'section.png'))
    page.goto((DESIGN / 'print.html').as_uri(), wait_until='load')
    page.pdf(path=str(DESIGN / 'print-1to1.pdf'), prefer_css_page_size=True, print_background=True)
    page.goto((DESIGN / 'rear-clearance.svg').as_uri(), wait_until='load')
    page.locator('svg').screenshot(path=str(DESIGN / 'rear-clearance.png'))
    page.goto((DESIGN / 'rear-clearance-print.html').as_uri(), wait_until='load')
    page.pdf(path=str(DESIGN / 'rear-clearance-print.pdf'), prefer_css_page_size=True, print_background=True)
    browser.close()
print('Rendered layout.png, section.png, rear-clearance.png and both A3 PDFs; run validate.py to verify PDF scale.')
