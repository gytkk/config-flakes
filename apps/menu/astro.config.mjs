import sitemap from "@astrojs/sitemap";
import { defineConfig } from "astro/config";

export default defineConfig({
  site: "https://menu.pylv.dev",
  output: "static",
  integrations: [sitemap()],
});
