import { describe, expect, test } from "bun:test";
import { recipeSchema } from "../src/lib/recipe-schema";
import { publishedRecipes, tagSlug } from "../src/lib/recipes";

const validRecipe = {
  title: "Valid recipe",
  description: "A valid description.",
  publishedAt: "2026-08-08",
  draft: false,
  tags: [],
};

describe("recipe frontmatter", () => {
  test("accepts required fields and empty tags", () => {
    expect(recipeSchema.parse(validRecipe).draft).toBeFalse();
  });

  test("rejects missing required fields", () => {
    expect(
      recipeSchema.safeParse({ ...validRecipe, draft: undefined }).success,
    ).toBeFalse();
  });

  test("requires coverAlt when cover is provided", () => {
    expect(
      recipeSchema.safeParse({
        ...validRecipe,
        cover: "https://media.pylv.dev/menu/cover.png",
      }).success,
    ).toBeFalse();
    expect(
      recipeSchema.safeParse({
        ...validRecipe,
        cover: "https://media.pylv.dev/menu/cover.png",
        coverAlt: "Cover image",
      }).success,
    ).toBeTrue();
  });
});

test("normalizes tags into stable route slugs", () => {
  expect(tagSlug(" Korean & Quick ")).toBe("korean-quick");
});

test("filters drafts and sorts published recipes by date", () => {
  const recipes = [
    {
      data: { draft: false, publishedAt: new Date("2026-08-08") },
      id: "older",
    },
    { data: { draft: true, publishedAt: new Date("2026-08-10") }, id: "draft" },
    {
      data: { draft: false, publishedAt: new Date("2026-08-09") },
      id: "newer",
    },
  ];

  expect(publishedRecipes(recipes).map((recipe) => recipe.id)).toEqual([
    "newer",
    "older",
  ]);
});
