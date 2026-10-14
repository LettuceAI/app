import type { SettingsModelSettingsLayer } from "@/api/generated/bindings";
import { describe, expect, it } from "vitest";
import { runtimeFieldDefault } from "./model-editor-defaults";
import full from "./fixtures/model-profile-config.json";
import { effectiveField, featureKeys, parseModelConfig, readField, resetFeature, serializeModelConfig, writeField } from "@/entities/model/model-config";

it("round-trips every model setting from the Rust entity-file example in its stored encoding", () => {
  expect(serializeModelConfig(parseModelConfig(full))).toEqual(full);
  expect(Object.keys(full.feature_parameters).sort()).toEqual([...featureKeys].sort());
});

describe("feature override encoding", () => {
  it("writes tagged overrides and leaves chat, other features and local settings untouched", () => {
    const config = parseModelConfig(full);
    const next = writeField(config, "help_me_reply", "chat", "temperature", 0.35);
    expect(next.feature_parameters.help_me_reply?.parameters?.temperature).toEqual({ kind: "set", value: 0.35 });
    expect(next.chat_parameters).toEqual(full.chat_parameters);
    expect(next.feature_parameters.dynamic_memory).toEqual(full.feature_parameters.dynamic_memory);
    expect(next.llama_cpp).toEqual(full.llama_cpp);
    expect(config).toEqual(full);
  });
  it("stores nested Ollama overrides and sparse llama sampler values", () => {
    let config = writeField(parseModelConfig(full), "creation_helper", "ollama", "min_p", 0.2);
    config = writeField(config, "creation_helper", "sampler", "dry_multiplier", 0.3);
    expect(config.feature_parameters.creation_helper?.parameters?.ollama?.min_p).toEqual({ kind: "set", value: 0.2 });
    expect(config.feature_parameters.creation_helper?.llama_sampler?.dry_multiplier).toBe(0.3);
  });
  it("preserves explicit clear and inherit encodings and null values", () => {
    const config = parseModelConfig(full);
    config.feature_parameters.help_me_reply = { parameters: { temperature: { kind: "clear" }, top_p: { kind: "inherit" } } };
    config.chat_parameters.top_k = null;
    expect(serializeModelConfig(parseModelConfig(serializeModelConfig(config)))).toEqual(config);
    expect(effectiveField(config, undefined, "help_me_reply", { store: "chat", key: "temperature" })).toBeUndefined();
    expect(effectiveField(config, undefined, "help_me_reply", { store: "chat", key: "top_p" })).toBe(full.chat_parameters.top_p);
  });
  it("resets one feature to chat inheritance without changing other features", () => {
    const config = resetFeature(parseModelConfig(full), "dynamic_memory");
    expect(config.feature_parameters.dynamic_memory).toBeUndefined();
    expect(readField(config, "dynamic_memory", "chat", "temperature").overridden).toBe(false);
    expect(effectiveField(config, undefined, "dynamic_memory", { store: "chat", key: "temperature" })).toBe(full.chat_parameters.temperature);
    expect(config.feature_parameters.creation_helper).toEqual(full.feature_parameters.creation_helper);
  });
});

it("keeps generic and separate KV cache types mutually exclusive and paired", () => {
  let config = writeField(parseModelConfig(full), "chat", "llama", "kv_type", "f16");
  expect(config.llama_cpp.kv_type_k).toBeUndefined();
  expect(config.llama_cpp.kv_type_v).toBeUndefined();
  config = writeField(config, "chat", "llama", "kv_type_k", "q8_0");
  expect(config.llama_cpp.kv_type).toBeUndefined();
  expect(config.llama_cpp.kv_type_v).toBe("f16");
  config = writeField(config, "chat", "llama", "kv_type_v", "q4_0");
  expect(config.llama_cpp.kv_type_k).toBe("q8_0");
  config = writeField(config, "chat", "llama", "kv_type_k", undefined);
  expect(config.llama_cpp.kv_type_k).toBeUndefined();
  expect(config.llama_cpp.kv_type_v).toBeUndefined();
});

it("uses llama sampler profile defaults and the provider token fallback without persisting them", () => {
  const config = parseModelConfig(full);
  config.llama_cpp.sampler = { profile: "stable" };
  expect(runtimeFieldDefault("local", "temperature", config, undefined, "chat")).toBe(0.55);
  expect(runtimeFieldDefault("local", "llamaAdaptiveDecay", config, undefined, "chat")).toBe(0.95);
  expect(runtimeFieldDefault("local", "maxOutputTokens", config, undefined, "chat")).toBe(512);
  expect(runtimeFieldDefault("cloud", "maxOutputTokens", config, undefined, "chat")).toBe(4096);
  expect(runtimeFieldDefault("cloud", "reasoningBudgetTokens", config, undefined, "chat")).toBeUndefined();
  expect(config.llama_cpp.sampler).toEqual({ profile: "stable" });
});

it("resolves the entire KV group from one layer and displays shared types in K and V", () => {
  const config = parseModelConfig(full);
  config.llama_cpp = { kv_type: "q8_0" };
  expect(effectiveField(config, undefined, "chat", { store: "llama", key: "kv_type_k" })).toBe("q8_0");
  expect(effectiveField(config, undefined, "chat", { store: "llama", key: "kv_type_v" })).toBe("q8_0");
  const split = writeField(config, "chat", "llama", "kv_type_k", "q4_0");
  expect(split.llama_cpp).toEqual({ kv_type_k: "q4_0", kv_type_v: "q8_0" });
  expect(effectiveField(split, undefined, "chat", { store: "llama", key: "kv_type" })).toBe("split");
  const defaults = { llama_cpp: { kv_type: "f32" } } as unknown as SettingsModelSettingsLayer;
  expect(effectiveField(split, defaults, "chat", { store: "llama", key: "kv_type_k" })).toBe("q4_0");
  expect(effectiveField(split, defaults, "chat", { store: "llama", key: "kv_type_v" })).toBe("q8_0");
  const inherited = parseModelConfig(full);
  inherited.llama_cpp = {};
  expect(effectiveField(inherited, defaults, "chat", { store: "llama", key: "kv_type_v" })).toBe("f32");
  const overridden = writeField(inherited, "chat", "llama", "kv_type_k", "q8_0", defaults);
  expect(overridden.llama_cpp).toEqual({ kv_type_k: "q8_0", kv_type_v: "f32" });
  const custom = writeField(config, "chat", "llama", "kv_type", "split");
  expect(custom.llama_cpp).toEqual({ kv_type_k: "q8_0", kv_type_v: "q8_0" });
  expect(effectiveField(custom, undefined, "chat", { store: "llama", key: "kv_type" })).toBe("split");
  expect(writeField(custom, "chat", "llama", "kv_type", "f16").llama_cpp).toEqual({ kv_type: "f16" });
});
