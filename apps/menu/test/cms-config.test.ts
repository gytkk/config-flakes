import { expect, test } from "bun:test";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { validateConfig } from "../scripts/validate-cms-config.mjs";

test("CMS config conforms to Sveltia's packaged schema", () => {
  const config = validateConfig();
  const collection = config.collections[0];

  expect(config.backend).toEqual({
    name: "github",
    repo: "gytkk/menu",
    branch: "main",
  });
  expect(config.media_libraries.cloudflare_r2).toEqual({
    access_key_id: "9eb6c3bac8fe6975f47311c54917836b",
    account_id: "910d658b051ca9e5fa5be458140853d9",
    bucket: "pylv-menu-media",
    jurisdiction: "default",
    public_url: "https://media.pylv.dev",
    prefix: "menu/",
  });
  expect(config.media_libraries.all.slugify_filename).toBeTrue();
  expect(config.media_libraries.all.transformations.raster_image).toEqual({
    format: "webp",
    quality: 85,
    width: 2000,
    height: 2000,
  });
  expect(collection.name).toBe("recipes");
  expect(collection.label).toBe("Recipes");
  expect(collection.folder).toBe("src/content/recipes");
  expect(collection.extension).toBe("md");
  expect(collection.slug).toBe("{{slug}}");
  expect(
    collection.fields.map((field: { name: string }) => field.name),
  ).toEqual([
    "title",
    "description",
    "publishedAt",
    "updatedAt",
    "draft",
    "tags",
    "cover",
    "coverAlt",
    "body",
  ]);
  expect(
    collection.fields.find((field: { name: string }) => field.name === "draft"),
  ).toMatchObject({ default: true });
  expect(
    collection.fields.find((field: { name: string }) => field.name === "tags"),
  ).toMatchObject({ default: [] });
  expect(
    collection.fields.find(
      (field: { name: string }) => field.name === "coverAlt",
    ),
  ).not.toMatchObject({ required: false });
});

test("CMS schema validation rejects an invalid config", () => {
  const path = join(
    mkdtempSync(join(tmpdir(), "sveltia-config-")),
    "config.yml",
  );
  writeFileSync(path, "backend: invalid\ncollections: []\n");

  expect(() => validateConfig(path)).toThrow();
});

test("CMS config contains no secret-like values", () => {
  expect(() => validateConfig()).not.toThrow();

  const path = join(
    mkdtempSync(join(tmpdir(), "sveltia-secret-")),
    "config.yml",
  );
  writeFileSync(
    path,
    `backend:
  name: github
  repo: gytkk/menu
collections:
  - name: recipes
    label: github_pat_1234567890abcdefghijklmnop
    files: []
`,
  );

  expect(() => validateConfig(path)).toThrow("Secret-like value");
});
