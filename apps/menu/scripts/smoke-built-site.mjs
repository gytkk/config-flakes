import { existsSync, readFileSync, readdirSync } from "node:fs";
import { resolve } from "node:path";
import { CMS_SECRET_PATTERN } from "./validate-cms-config.mjs";

const dist = resolve("dist");
const requiredFiles = [
  "index.html",
  "404.html",
  "recipes/welcome/index.html",
  "tags/index.html",
  "tags/recipes/index.html",
  "rss.xml",
  "sitemap-index.xml",
  "robots.txt",
  "admin/index.html",
  "admin/config.yml",
  "admin/sveltia-cms.js",
];

for (const file of requiredFiles) {
  if (!existsSync(resolve(dist, file))) {
    throw new Error(`Missing built file: ${file}`);
  }
}

const output = Object.fromEntries(
  requiredFiles.map((file) => [
    file,
    readFileSync(resolve(dist, file), "utf8"),
  ]),
);
const files = readdirSync(dist, { recursive: true, withFileTypes: true })
  .filter((entry) => entry.isFile())
  .map((entry) => readFileSync(resolve(entry.parentPath, entry.name), "utf8"))
  .join("\n");
const expectedOutput = [
  ["index.html", "https://menu.pylv.dev/"],
  ["index.html", "/recipes/welcome/"],
  ["recipes/welcome/index.html", "Welcome to Menu by pylv | Menu by pylv"],
  ["recipes/welcome/index.html", 'rel="canonical"'],
  ["recipes/welcome/index.html", 'property="og:title"'],
  ["404.html", "Page not found"],
  ["rss.xml", "Welcome to Menu by pylv"],
  ["sitemap-index.xml", "https://menu.pylv.dev/sitemap-0.xml"],
  ["admin/index.html", 'src="/admin/sveltia-cms.js"'],
  ["admin/index.html", 'name="robots" content="noindex, nofollow"'],
  [
    "admin/config.yml",
    "https://unpkg.com/@sveltia/cms@0.181.1/schema/sveltia-cms.json",
  ],
  ["admin/config.yml", "access_key_id: 9eb6c3bac8fe6975f47311c54917836b"],
  ["admin/config.yml", "bucket: pylv-menu-media"],
  ["admin/config.yml", "default: true"],
  ["admin/config.yml", "default: []"],
  ["admin/config.yml", "repo: gytkk/menu"],
  ["admin/config.yml", "prefix: menu/"],
  ["admin/config.yml", "folder: src/content/recipes"],
  ["admin/config.yml", "extension: md"],
  ["admin/config.yml", 'slug: "{{slug}}"'],
  ["admin/config.yml", "name: title"],
  ["admin/config.yml", "name: description"],
  ["admin/config.yml", "name: publishedAt"],
  ["admin/config.yml", "name: updatedAt"],
  ["admin/config.yml", "name: draft"],
  ["admin/config.yml", "name: tags"],
  ["admin/config.yml", "name: cover"],
  ["admin/config.yml", "name: coverAlt"],
  ["admin/config.yml", "name: body"],
];

for (const [file, text] of expectedOutput) {
  if (!output[file].includes(text)) {
    throw new Error(`Missing expected output in ${file}: ${text}`);
  }
}

if (
  files.includes("Private recipe draft") ||
  files.includes("/recipes/draft/")
) {
  throw new Error("Draft recipe leaked into production output");
}

if (CMS_SECRET_PATTERN.test(output["admin/config.yml"])) {
  throw new Error("Secret-like configuration leaked into the built admin site");
}
