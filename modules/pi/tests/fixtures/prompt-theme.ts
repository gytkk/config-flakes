import { Theme } from "@earendil-works/pi-coding-agent";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

export function promptTheme(name = "one-half-light"): Theme {
  const { colors } = JSON.parse(readFileSync(resolve(`themes/exports/pi/${name}.json`), "utf8"));
  return new Theme(colors, colors, "truecolor");
}
