import type { CollectionEntry } from "astro:content";

export type Recipe = CollectionEntry<"recipes">;

type Publishable = {
  data: {
    draft: boolean;
    publishedAt: Date;
  };
};

export function publishedRecipes(recipes: Recipe[]): Recipe[];
export function publishedRecipes<T extends Publishable>(recipes: T[]): T[];
export function publishedRecipes(recipes: Publishable[]): Publishable[] {
  return recipes
    .filter((recipe) => !recipe.data.draft)
    .sort(
      (left, right) =>
        right.data.publishedAt.valueOf() - left.data.publishedAt.valueOf(),
    );
}

export function tagSlug(tag: string): string {
  return tag
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "");
}

export function tagsFor(recipes: Recipe[]): Map<string, string> {
  const tags = new Map<string, string>();
  for (const recipe of recipes) {
    for (const tag of recipe.data.tags) {
      const slug = tagSlug(tag);
      if (slug) tags.set(slug, tag);
    }
  }
  return tags;
}
