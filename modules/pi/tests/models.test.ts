import { describe, expect, test } from "bun:test";
import { InMemoryCredentialStore, InMemoryModelsStore } from "@earendil-works/pi-ai";
import { ModelRuntime } from "@earendil-works/pi-coding-agent";
import { fileURLToPath } from "node:url";
import models from "../files/models.json";
import settings from "../files/settings.json";

const overrides = models.providers.openai.modelOverrides;

describe("OpenAI context windows", () => {
  test("covers every configured parent and subagent model", () => {
    const configured = new Set([
      `${settings.defaultProvider}/${settings.defaultModel}`,
      settings.subagents.defaultModel,
      ...settings.subagents.modelScope.allow,
      ...Object.values(settings.subagents.agentOverrides)
        .flatMap((agent) => "model" in agent ? [agent.model] : []),
    ]);
    expect([...configured].sort()).toEqual(Object.keys(overrides).map((id) => `openai/${id}`).sort());
    for (const override of Object.values(overrides)) {
      expect(override).toEqual({ contextWindow: 1_000_000 });
    }
  });

  test("Pi loads the overrides without replacing other model metadata", async () => {
    const create = (modelsPath: string | null) => ModelRuntime.create({
      credentials: new InMemoryCredentialStore(),
      modelsStore: new InMemoryModelsStore(),
      modelsPath,
      refreshOnCreate: false,
      allowModelNetwork: false,
    });
    const baseline = await create(null);
    const configured = await create(fileURLToPath(new URL("../files/models.json", import.meta.url)));
    expect(configured.getError()).toBeUndefined();
    for (const id of Object.keys(overrides)) {
      const original = baseline.getModel("openai", id);
      expect(original).toBeDefined();
      expect(configured.getModel("openai", id)).toEqual({ ...original, contextWindow: 1_000_000 });
    }
    expect(configured.getModels("openai").map((model) => model.id))
      .toEqual(baseline.getModels("openai").map((model) => model.id));
    expect(configured.getModelsOfType("classifier", "openai"))
      .toEqual(baseline.getModelsOfType("classifier", "openai"));
  });
});
