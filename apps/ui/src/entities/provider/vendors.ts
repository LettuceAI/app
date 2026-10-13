import { logos } from "./logos";

export interface ModelVendor {
  match: RegExp;
  vendor: string;
  logo: string;
}

export const modelVendors: readonly ModelVendor[] = [
  { match: /anthropic|claude/, vendor: "Anthropic", logo: logos.anthropic },
  { match: /openai|gpt-|\bo[134]\b/, vendor: "OpenAI", logo: logos.openai },
  { match: /google|gemini|gemma/, vendor: "Google", logo: logos.gemini },
  { match: /deepseek/, vendor: "DeepSeek", logo: logos.deepseek },
  { match: /z-ai|zai|glm/, vendor: "Z.ai", logo: logos.zai },
  { match: /moonshot|kimi/, vendor: "Moonshot", logo: logos.moonshot },
  { match: /qwen/, vendor: "Qwen", logo: logos.qwen },
  { match: /x-ai|grok/, vendor: "xAI", logo: logos.xai },
  { match: /mistral|mixtral/, vendor: "Mistral", logo: logos.mistral },
  { match: /nvidia|nemotron/, vendor: "NVIDIA", logo: logos.nvidia },
];

/** The vendor a model id belongs to, read from its id. */
export function vendorOf(modelId: string): ModelVendor | null {
  const id = modelId.toLowerCase();
  return modelVendors.find((vendor) => vendor.match.test(id)) ?? null;
}

/**
 * The name a model gets when it is added: the provider's name for it, or the last part of its id, without the
 * "Company: " prefix aggregators put in front when the company is known from the id.
 */
export function defaultModelName(displayName: string | null, modelId: string): string {
  const name = displayName ?? (modelId.split("/").at(-1) || modelId);
  if (!vendorOf(modelId)) return name;
  const colon = name.indexOf(": ");
  return colon > 0 && colon < name.length - 2 ? name.slice(colon + 2) : name;
}
