import anthropic from "./assets/anthropic_light.svg";
import cerebras from "./assets/cerebras.svg";
import chutes from "./assets/chutes.webp";
import deepseek from "./assets/deepseek.svg";
import featherless from "./assets/featherless.svg";
import gemini from "./assets/gemini.svg";
import groq from "./assets/groq.svg";
import intenserp from "./assets/intenserp.png";
import literouter from "./assets/literouter.png";
import lmstudio from "./assets/lmstudio_light.png";
import mistral from "./assets/mistralai_light.svg";
import moonshot from "./assets/moonshot_light.svg";
import nanogpt from "./assets/nanogpt.png";
import nvidia from "./assets/nvidia.svg";
import ollama from "./assets/ollama_light.png";
import openai from "./assets/openai_light.svg";
import openrouter from "./assets/openrouter_light.svg";
import pollinations from "./assets/pollinations.svg";
import qwen from "./assets/qwen.svg";
import xai from "./assets/xai_light.svg";
import zai from "./assets/zai_light.svg";

const providerLogos: Readonly<Record<string, string>> = {
  openai,
  anthropic,
  gemini,
  "gemini-agent-platform-express": gemini,
  openrouter,
  deepseek,
  mistral,
  xai,
  moonshot,
  zai,
  qwen,
  groq,
  cerebras,
  nvidia,
  chutes,
  featherless,
  nanogpt,
  literouter,
  pollinations,
  ollama,
  lmstudio,
  intenserp,
};

export const logos = {
  anthropic,
  deepseek,
  gemini,
  mistral,
  moonshot,
  nvidia,
  openai,
  qwen,
  xai,
  zai,
} as const;

/** The logo for a catalog kind, or null for kinds that have none. */
export function providerLogo(kind: string): string | null {
  return providerLogos[kind] ?? null;
}
