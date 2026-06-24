import type { ReactElement } from "react";
import {
  Cpu,
  EthernetPort,
  Image as ImageIcon,
  Leaf,
  Mic,
  Settings,
  Sparkles,
  Volume2,
  Wrench,
} from "lucide-react";

import OpenAIIcon from "../../assets/openai_light.svg";
import CerebrasIcon from "../../assets/cerebras.svg";
import AnthropicIcon from "../../assets/anthropic_light.svg";
import OpenRouterIcon from "../../assets/openrouter_light.svg";
import LiteRouterIcon from "../../assets/literouter.png";
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
import LlamaCppIcon from "../../assets/llama-cpp.svg";
import IntenserpIcon from "../../assets/intenserp.png";
import GroqIcon from "../../assets/groq.svg";
import PollinationsIcon from "../../assets/pollinations.svg";

const ICON_MAP: Record<string, ReactElement> = {
  openai: <img src={OpenAIIcon} alt="OpenAI" className="h-6 w-6" />,
  cerebras: <img src={CerebrasIcon} alt="Cerebras" className="h-6 w-6" />,
  anthropic: <img src={AnthropicIcon} alt="Anthropic" className="h-6 w-6" />,
  openrouter: <img src={OpenRouterIcon} alt="OpenRouter" className="h-6 w-6" />,
  literouter: <img src={LiteRouterIcon} alt="LiteRouter" className="h-6 w-6" />,
  pollinations: <img src={PollinationsIcon} alt="Pollinations AI" className="h-6 w-6" />,
  mistral: <img src={MistralAIIcon} alt="MistralAI" className="h-6 w-6" />,
  deepseek: <img src={DeepseekIcon} alt="Deepseek" className="h-6 w-6" />,
  nanogpt: <img src={NanoGPTIcon} alt="NanoGPT" className="h-6 w-6" />,
  xai: <img src={XAIIcon} alt="xAI" className="h-6 w-6" />,
  zai: <img src={ZAIIcon} alt="ZAI" className="h-6 w-6" />,
  moonshot: <img src={MoonShotAIIcon} alt="Moonshot AI" className="h-6 w-6" />,
  gemini: <img src={GeminiIcon} alt="Gemini" className="h-6 w-6" />,
  "gemini-agent-platform-express": (
    <img src={GeminiIcon} alt="Gemini Agent Platform (Express)" className="h-6 w-6" />
  ),
  qwen: <img src={QwenIcon} alt="Qwen" className="h-6 w-6" />,
  groq: <img src={GroqIcon} alt="Groq" className="h-6 w-6" />,
  featherless: <img src={FeatherlessIcon} alt="Featherless" className="h-6 w-6" />,
  nvidia: <img src={NvidiaIcon} alt="NVIDIA" className="h-6 w-6" />,
  chutes: <img src={ChutesAIIcon} alt="Chutes" className="h-6 w-6" />,
  ollama: <img src={OllamaIcon} alt="Ollama" className="h-6 w-6" />,
  lmstudio: <img src={LMStudioIcon} alt="LM Studio" className="h-6 w-6" />,
  intenserp: <img src={IntenserpIcon} alt="IntenseRP Next" className="h-6 w-6" />,
  llamacpp: <img src={LlamaCppIcon} alt="llama.cpp" className="h-6 w-6 object-contain" />,
  "lettuce-host": <EthernetPort className="h-6 w-6 text-emerald-300" />,
  automatic1111: <Cpu className="h-6 w-6 text-orange-400" />,
  localdiffusion: <ImageIcon className="h-6 w-6 text-emerald-300" />,
  stability: <Sparkles className="h-6 w-6 text-sky-400" />,
  "lettuce-engine": <Leaf className="h-6 w-6 text-emerald-400" />,
  custom: <Settings className="h-6 w-6 text-gray-400" />,
  "custom-anthropic": <Settings className="h-6 w-6 text-gray-400" />,
  elevenlabs: <Mic className="h-6 w-6 text-violet-300" />,
  fish_tts: <Volume2 className="h-6 w-6 text-cyan-300" />,
  fish_speech: <Cpu className="h-6 w-6 text-cyan-300" />,
  gemini_tts: <img src={GeminiIcon} alt="Gemini TTS" className="h-6 w-6" />,
  openai_tts: <Volume2 className="h-6 w-6 text-emerald-300" />,
  kokoro: <Cpu className="h-6 w-6 text-pink-300" />,
};

export function getProviderIcon(providerId: string) {
  return ICON_MAP[providerId] ?? <Wrench className="h-6 w-6 text-gray-500" />;
}
