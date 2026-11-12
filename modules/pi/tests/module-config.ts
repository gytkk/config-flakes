import { readFileSync } from "node:fs";

const path = process.env.PI_TEST_MODULE_CONFIG;
if (!path) throw new Error("Run through modules/pi/tests/run.sh to evaluate the module configuration.");
export const moduleConfig = JSON.parse(readFileSync(path, "utf8"));
export const settings = moduleConfig.enabled.settings;
