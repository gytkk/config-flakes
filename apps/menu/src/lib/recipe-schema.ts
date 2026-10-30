import { z } from "zod";

export const recipeSchema = z
  .object({
    title: z.string().min(1),
    description: z.string().min(1),
    publishedAt: z.coerce.date(),
    updatedAt: z.coerce.date().optional(),
    draft: z.boolean(),
    tags: z.array(z.string()),
    cover: z.url().optional(),
    coverAlt: z.string().min(1).optional(),
  })
  .superRefine((value, context) => {
    if (value.cover && !value.coverAlt) {
      context.addIssue({
        code: z.ZodIssueCode.custom,
        message: "coverAlt is required when cover is set",
        path: ["coverAlt"],
      });
    }
  });
