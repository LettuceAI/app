import { Leaf, Settings, Wrench } from "lucide-react";

import OpenAIIcon from "../../assets/openai_light.svg";
import AnthropicIcon from "../../assets/anthropic_light.svg";
import OpenRouterIcon from "../../assets/openrouter_light.svg";
import MistralAIIcon from "../../assets/mistralai_light.svg";
import DeepseekIcon from "../../assets/deepseek.svg";
import NanoGPTIcon from "../../assets/nanogpt.png";
import XAIIcon from "../../assets/xai_light.svg";
import ZAIIcon from "../../assets/zai_light.svg";
import MoonShotAIIcon from "../../assets/moonshot_light.svg";
import GeminiIcon from "../../assets/gemini.svg";
import QwenIcon from "../../assets/qwen.svg";
import FeatherlessIcon from "../../assets/featherless.svg";
import NvidiaIcon from "../../assets/nvidia.svg";
import ChutesAIIcon from "../../assets/chutes_ai.jpeg";
import OllamaIcon from "../../assets/ollama_light.png";
import LMStudioIcon from "../../assets/lmstudio_light.png";

const ICON_MAP: Record<string, JSX.Element> = {
  openai: <img src={OpenAIIcon} alt="OpenAI" className="h-6 w-6" />,
  anthropic: <img src={AnthropicIcon} alt="Anthropic" className="h-6 w-6" />,
  openrouter: <img src={OpenRouterIcon} alt="OpenRouter" className="h-6 w-6" />,
  mistral: <img src={MistralAIIcon} alt="MistralAI" className="h-6 w-6" />,
  deepseek: <img src={DeepseekIcon} alt="Deepseek" className="h-6 w-6" />,
  nanogpt: <img src={NanoGPTIcon} alt="NanoGPT" className="h-6 w-6" />,
  xai: <img src={XAIIcon} alt="xAI" className="h-6 w-6" />,
  zai: <img src={ZAIIcon} alt="ZAI" className="h-6 w-6" />,
  moonshot: <img src={MoonShotAIIcon} alt="Moonshot AI" className="h-6 w-6" />,
  gemini: <img src={GeminiIcon} alt="Gemini" className="h-6 w-6" />,
  qwen: <img src={QwenIcon} alt="Qwen" className="h-6 w-6" />,
  featherless: <img src={FeatherlessIcon} alt="Featherless" className="h-6 w-6" />,
  nvidia: <img src={NvidiaIcon} alt="NVIDIA" className="h-6 w-6" />,
  chutes: <img src={ChutesAIIcon} alt="Chutes" className="h-6 w-6" />,
  ollama: <img src={OllamaIcon} alt="Ollama" className="h-6 w-6" />,
  lmstudio: <img src={LMStudioIcon} alt="LM Studio" className="h-6 w-6" />,
  llamacpp: <Wrench className="h-6 w-6 text-gray-500" />,
  "lettuce-engine": <Leaf className="h-6 w-6 text-emerald-400" />,
  custom: <Settings className="h-6 w-6 text-gray-400" />,
  "custom-anthropic": <Settings className="h-6 w-6 text-gray-400" />,
};

export function getProviderIcon(providerId: string) {
  return ICON_MAP[providerId] ?? <Wrench className="h-6 w-6 text-gray-500" />;
}
