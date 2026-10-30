import rss from "@astrojs/rss";
import type { APIRoute } from "astro";
import { getCollection } from "astro:content";
import { publishedRecipes } from "../lib/recipes";

export const GET: APIRoute = async (context) =>
  rss({
    title: "Menu by pylv",
    description: "Approachable recipes for deciding what to cook.",
    site: context.site ?? "https://menu.pylv.dev",
    items: publishedRecipes(await getCollection("recipes")).map((recipe) => ({
      title: recipe.data.title,
      description: recipe.data.description,
      pubDate: recipe.data.publishedAt,
      link: `/recipes/${recipe.id}/`,
    })),
  });
