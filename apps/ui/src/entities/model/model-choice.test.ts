import { describe, expect, it } from "vitest";
import type { ModelModality, ModelView, ProviderAccountView } from "@/api/generated/bindings";
import { groupByProvider, meetsRequirement, modelChoices, searchModels, type ModelChoice } from "./model-choice";

const account = (id: string, provider_kind = "openai", label = id): ProviderAccountView => ({
  id,
  label,
  provider_kind,
  enabled: true,
  protocol: "open_ai_compatible",
  base_url: null,
  streaming_enabled: true,
  allow_invalid_tls: false,
  api_key_set: true,
  config: {},
  revision: 1,
  created_at: 1,
  updated_at: 1,
});
const model = (
  id: string,
  provider_account_id: string,
  patch: Partial<ModelView> = {},
): ModelView => ({
  id,
  provider_account_id,
  display_name: id,
  external_model_id: id,
  kind: "chat",
  input_scopes: ["text"],
  output_scopes: ["text"],
  config: {},
  revision: 1,
  created_at: 1,
  updated_at: 1,
  ...patch,
});
const choice = (value: ModelView, owner: ProviderAccountView): ModelChoice => ({ model: value, account: owner });
const names = (found: ModelChoice[]) => found.map(({ model: value }) => value.display_name);

const cloud = account("cloud", "openai", "Cloud");
const local = account("local", "llamacpp", "This device");
const router = account("router", "openrouter", "Router");

describe("modelChoices", () => {
  it("joins models to accounts in list order and drops orphans", () => {
    const joined = modelChoices(
      [model("a", "router"), model("orphan", "gone"), model("b", "cloud")],
      [cloud, router],
    );
    expect(joined.map((entry) => [entry.model.id, entry.account.id])).toEqual([
      ["a", "router"],
      ["b", "cloud"],
    ]);
  });
});

describe("meetsRequirement", () => {
  const vision = choice(
    model("vision", "cloud", { input_scopes: ["text", "image"], output_scopes: ["text"] }),
    cloud,
  );
  const painter = choice(
    model("painter", "cloud", { kind: "image", input_scopes: ["text"], output_scopes: ["image"] }),
    cloud,
  );

  it("accepts everything without a requirement", () => {
    expect(meetsRequirement(vision, undefined)).toBe(true);
    expect(meetsRequirement(vision, {})).toBe(true);
  });

  it("checks the kind", () => {
    expect(meetsRequirement(vision, { kind: "chat" })).toBe(true);
    expect(meetsRequirement(painter, { kind: "chat" })).toBe(false);
  });

  it("needs every listed input and output modality", () => {
    const both: readonly ModelModality[] = ["text", "image"];
    expect(meetsRequirement(vision, { input: both })).toBe(true);
    expect(meetsRequirement(painter, { input: both })).toBe(false);
    expect(meetsRequirement(painter, { output: ["image"] })).toBe(true);
    expect(meetsRequirement(vision, { output: ["text", "image"] })).toBe(false);
  });

  it("applies the caller rule last", () => {
    expect(meetsRequirement(vision, { kind: "chat", allow: ({ account: owner }) => owner.id !== "cloud" })).toBe(false);
    expect(meetsRequirement(vision, { kind: "image", allow: () => true })).toBe(false);
    expect(meetsRequirement(vision, { allow: () => true })).toBe(true);
  });
});

describe("groupByProvider", () => {
  it("puts on-device first, then providers in first-seen order", () => {
    const groups = groupByProvider([
      choice(model("r1", "router"), router),
      choice(model("c1", "cloud"), cloud),
      choice(model("l1", "local"), local),
      choice(model("r2", "router"), router),
    ]);
    expect(groups.map((group) => group.account.id)).toEqual(["local", "router", "cloud"]);
    expect(groups[1]?.choices.map((entry) => entry.model.id)).toEqual(["r1", "r2"]);
  });
});

describe("searchModels", () => {
  const all = [
    choice(model("GPT-4o", "cloud", { external_model_id: "gpt-4o" }), cloud),
    choice(model("Llama-3.1-8B", "local", { external_model_id: "llama-3.1-8b-instruct" }), local),
    choice(model("Claude Sonnet", "router", { external_model_id: "anthropic/claude-sonnet-4" }), router),
    choice(model("Crème brûlée", "cloud", { external_model_id: "creme" }), cloud),
  ];

  it("returns every choice in order for an empty query", () => {
    expect(names(searchModels(all, ""))).toEqual(names(all));
    expect(names(searchModels(all, "  --  "))).toEqual(names(all));
  });

  it("matches across separators", () => {
    expect(names(searchModels(all, "gpt4o"))).toEqual(["GPT-4o"]);
    expect(names(searchModels(all, "llama 3.1"))).toEqual(["Llama-3.1-8B"]);
  });

  it("matches words in any order", () => {
    expect(names(searchModels(all, "sonnet claude"))).toEqual(["Claude Sonnet"]);
    expect(names(searchModels(all, "router sonnet"))).toEqual(["Claude Sonnet"]);
  });

  it("ignores diacritics and case", () => {
    expect(names(searchModels(all, "CREME brulee"))).toEqual(["Crème brûlée"]);
  });

  it("matches the provider label and kind", () => {
    expect(names(searchModels(all, "this device"))).toEqual(["Llama-3.1-8B"]);
    expect(names(searchModels(all, "llamacpp"))).toEqual(["Llama-3.1-8B"]);
  });

  it("tolerates a typo through letters in order", () => {
    expect(names(searchModels(all, "sonet"))).toEqual(["Claude Sonnet"]);
  });

  it("does not use in-order matching for one or two characters", () => {
    expect(names(searchModels(all, "gt"))).toEqual([]);
    expect(names(searchModels(all, "cs"))).toEqual([]);
  });

  it("does not bridge gaps longer than two characters", () => {
    expect(names(searchModels(all, "sxxxxnet"))).toEqual([]);
    expect(names(searchModels(all, "sot"))).toEqual([]);
  });

  it("needs every token to match", () => {
    expect(names(searchModels(all, "gpt sonnet"))).toEqual([]);
  });

  it("ranks a name prefix above a provider-only match", () => {
    const ranked = searchModels(
      [
        choice(model("Other", "router", { external_model_id: "other" }), account("router", "openrouter", "Nova host")),
        choice(model("Nova Pro", "cloud", { external_model_id: "pro" }), cloud),
      ],
      "nova",
    );
    expect(names(ranked)).toEqual(["Nova Pro", "Other"]);
  });

  it("keeps list order between equal scores", () => {
    const ranked = searchModels(
      [choice(model("Alpha Mini", "cloud"), cloud), choice(model("Alpha Max", "cloud"), cloud)],
      "alpha",
    );
    expect(names(ranked)).toEqual(["Alpha Mini", "Alpha Max"]);
  });
});
