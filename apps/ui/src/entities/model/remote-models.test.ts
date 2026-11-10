import { describe, expect, it } from "vitest";
import type { RemoteModelContract } from "@/api/generated/bindings";
import { filterModels, noFilters } from "./remote-models";

const remoteModel = (overrides: Partial<RemoteModelContract> & { id: string }): RemoteModelContract => ({
  cache_read_price: null,
  display_name: null,
  description: null,
  context_length: 128000,
  input_modalities: ["text"],
  output_modalities: ["text"],
  supported_endpoints: null,
  input_price: null,
  output_price: null,
  ...overrides,
});

const models = [
  remoteModel({ id: "anthropic/claude-sonnet-5", display_name: "Anthropic: Claude Sonnet 5", input_price: 0.000003 }),
  remoteModel({ id: "openai/gpt-4o-mini", display_name: "OpenAI: GPT-4o Mini", input_price: 0.00000015 }),
  remoteModel({ id: "deepseek/deepseek-v3.2", display_name: "DeepSeek: DeepSeek V3.2", input_price: 0.00000027 }),
];
const ids = (text: string, sort = noFilters.sort) =>
  filterModels(models, { text, onlyFree: false, filters: { ...noFilters, sort } }).map((model) => model.id);

describe("filterModels", () => {
  it("matches words in any order, across separators and with small gaps", () => {
    expect(ids("sonnet claude")).toEqual(["anthropic/claude-sonnet-5"]);
    expect(ids("gpt4o")).toEqual(["openai/gpt-4o-mini"]);
    expect(ids("dpsk")).toEqual(["deepseek/deepseek-v3.2"]);
  });

  it("lists everything sorted by name without a query and keeps the chosen sort while searching", () => {
    expect(ids("")).toEqual(["anthropic/claude-sonnet-5", "deepseek/deepseek-v3.2", "openai/gpt-4o-mini"]);
    expect(ids("e", "price")).toEqual(["openai/gpt-4o-mini", "deepseek/deepseek-v3.2", "anthropic/claude-sonnet-5"]);
  });
});
