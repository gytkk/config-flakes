import { beforeAll, expect, test } from "bun:test";
import { stripVTControlCharacters } from "node:util";

// Nix selects the upstream workspace at runtime; this checkout has none of its dependencies.
const source = process.env.OMP_SOURCE_DIR;
const check = source ? test : test.skip;
let renderSegment: (id: string, context: unknown) => { content: string; visible: boolean };
let describeSegment: (id: string, context: unknown) => { spans: { t: string }[] } | null;

beforeAll(async () => {
  if (!source) return;
  ({ renderSegment, describeSegment } = await import(`${source}/packages/tui/src/status-line/segments.ts`));
  const { initTheme } = await import(`${source}/packages/tui/src/theme/index.ts`);
  await initTheme();
});

check("subscription and reset-credit metadata do not alter quota text in either renderer", () => {
  const usage = {
    fiveHour: { percent: 12, resetMinutes: 90 },
    sevenDay: { percent: 24, resetHours: 48 },
  };
  const context = { usage, sessionAccent: false };
  const detailed = {
    ...context,
    usage: {
      ...usage,
      tier: "prolite",
      resetCredits: {
        bankedCount: 2, redeemableCount: 0, expiryHours: 5,
        unavailableReason: "weekly cooldown", expiring: { tier: "imminent", count: 1, expiresAtMs: 1 },
      },
    },
  };
  const plain = renderSegment("usage", context);
  expect(renderSegment("usage", detailed)).toEqual(plain);
  expect(describeSegment("usage", detailed)).toEqual(describeSegment("usage", context));
  const text = stripVTControlCharacters(plain.content);
  expect(text).toContain("5h 12%");
  expect(text).toContain("7d 24%");
});

check("accounts with only hidden metadata do not leave an empty usage icon", () => {
  const context = { usage: { tier: "prolite", resetCredits: { bankedCount: 2, redeemableCount: 2 } } };
  expect(renderSegment("usage", context)).toEqual({ content: "", visible: false });
  expect(describeSegment("usage", context)).toBeNull();
  const zeroQuota = { usage: { fiveHour: { percent: 0 } } };
  expect(renderSegment("usage", zeroQuota).visible).toBe(true);
  expect(describeSegment("usage", zeroQuota)?.spans.some(span => span.t === "0%")).toBe(true);
});
