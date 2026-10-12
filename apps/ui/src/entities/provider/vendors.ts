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
