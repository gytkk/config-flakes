# Menu by pylv

A static Astro recipe site for `https://menu.pylv.dev`, with a self-hosted
Sveltia CMS at `/admin/`.

## Local development

Requires Bun 1.3.13:

```sh
bun ci
bun run format:check
bun run validate:cms
bun run check
bun run test
bun run build
bun run smoke
```

`bun run dev` prepares the pinned CMS browser artifact and starts Astro. Open
`http://localhost:4321/admin/index.html` to use the CMS locally. The deployed
nginx origin serves the same page at `/admin/`.

## Recipes and CMS

Recipes live in `src/content/recipes/**/*.{md,mdx}`. The Sveltia collection
writes YAML-frontmatter `.md` files in that folder using a stable,
title-derived filename slug. Astro remains authoritative for frontmatter
validation. Each recipe has these CMS fields:

- `title`: required non-empty string
- `description`: required non-empty string
- `publishedAt`: required `YYYY-MM-DD` date
- `updatedAt`: optional `YYYY-MM-DD` date
- `draft`: required boolean, defaulting to `true`
- `tags`: required string list, defaulting to `[]`
- `cover`: optional public image URL
- `coverAlt`: required by CMS so selected covers always have accessible alternative text; Astro allows it to be omitted only when `cover` is absent
- `body`: Markdown recipe content

Use the Markdown body for flexible sections such as ingredients, preparation,
and method. Draft recipes remain available to local content tooling but are
excluded from the built home page, recipe routes, tag pages, RSS feed, and
sitemap.

### CMS login and media

Sign in at `/admin/` with a GitHub PAT that can edit `gytkk/menu`. Do not put a
PAT in this repository or the CMS config.

Images are stored in the `pylv-menu-media` Cloudflare R2 bucket under `menu/`
and served from `https://media.pylv.dev`. The public config contains
only the bucket-limited Access Key ID and account metadata. On first R2 use,
enter the R2 Secret Access Key directly in the CMS browser prompt; Sveltia
keeps it in that browser's local storage. An encrypted recovery copy is stored
as `secrets/menu-r2-secret-access-key.age` in the `nix-flakes` repository for
the administrator and deployment Mac only. Never add the plaintext value to
`config.yml`, Git, deployment artifacts, logs, or chat.

The R2 CORS policy permits only `https://menu.pylv.dev` requests for `GET`,
`PUT`, and `HEAD`. Uploads slugify filenames, are limited to 10 MiB, and
convert raster images to WebP with an 85 quality setting and 2000 px maximum
width and height.

### CRUD verification

After signing in, create a draft recipe, edit it, publish it by setting
`draft: false`, and delete the test recipe. Review each Git diff, then run the
local validation and build commands above. Test an image upload separately
with the browser-only Secret Access Key and verify its resulting public URL
uses `https://media.pylv.dev/menu/`.

## Deployment artifact

`bun run build` copies the exact pinned `@sveltia/cms` browser artifact from
`node_modules` into `public/admin/sveltia-cms.js` before Astro builds `dist/`.
The generated source artifact is gitignored; the checked deployment artifact
contains `/admin/index.html`, `/admin/config.yml`, and
`/admin/sveltia-cms.js`. `bun run validate:cms` validates `config.yml` against
the JSON Schema packaged by `@sveltia/cms` 0.181.1.

Deploy only a checked, tested `dist/` directory. The static origin expects at
least `index.html`, `404.html`, `rss.xml`, `sitemap-index.xml`,
`admin/index.html`, `admin/config.yml`, and `admin/sveltia-cms.js`.
