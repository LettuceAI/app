import { Check } from "lucide-react";
import type { ProviderCapabilitiesCamel } from "../../../../core/providers/capabilities";
import { getProviderIcon } from "../../../../core/utils/providerIcons";

interface ProviderCardProps {
    provider: ProviderCapabilitiesCamel;
    isActive: boolean;
    onClick: () => void;
    variant?: "standard" | "compact";
}

export function ProviderCard({ provider, isActive, onClick, variant = "standard" }: ProviderCardProps) {
    if (variant === "compact") {
        return (
            <button
                className={`relative group rounded-xl border px-4 py-3 text-left transition-all duration-200 ${isActive
                    ? "border-emerald-400/40 bg-emerald-400/10 ring-1 ring-emerald-400/30"
                    : "border-white/10 bg-white/5 hover:border-white/20 hover:bg-white/8 active:scale-[0.98]"
                    }`}
                onClick={onClick}
            >
                <div className="flex items-center gap-3">
                    <div className={`flex h-10 w-10 shrink-0 items-center justify-center rounded-lg border ${isActive ? "border-emerald-400/30 bg-emerald-400/10" : "border-white/15 bg-white/8"
                        }`}>
                        {getProviderIcon(provider.id)}
                    </div>
                    <div className="flex-1 min-w-0">
                        <h3 className={`text-sm font-medium leading-tight truncate ${isActive ? "text-emerald-100" : "text-white"}`}>
                            {provider.name}
                        </h3>
                        <p className="text-xs text-gray-500 leading-snug truncate">{getProviderDescriptionShort(provider.id)}</p>
                    </div>
                    <div className={`flex h-5 w-5 shrink-0 items-center justify-center rounded-full border transition-colors ${isActive
                        ? "border-emerald-400/60 bg-emerald-400/30 text-emerald-200"
                        : "border-white/15 text-transparent"
                        }`}>
                        <Check size={10} />
                    </div>
                </div>
            </button>
        );
    }

    // variant mobile
    return (
        <button
            className={`relative group min-h-[88px] rounded-2xl border px-3 py-3 text-left transition-all duration-200 ${isActive
                ? "border-white/25 bg-white/15 shadow-lg"
                : "border-white/10 bg-white/5 hover:border-white/20 hover:bg-white/10 active:scale-[0.98]"
                }`}
            onClick={onClick}
        >
            <div className="flex flex-col gap-2">
                <div className="flex items-start justify-between gap-2">
                    <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg border border-white/15 bg-white/8">
                        {getProviderIcon(provider.id)}
                    </div>
                    <div className={`flex h-4 w-4 shrink-0 items-center justify-center rounded-full border transition-colors ${isActive
                        ? "border-emerald-400/60 bg-emerald-400/20 text-emerald-300"
                        : "border-white/20 text-transparent"
                        }`}>
                        <Check size={10} />
                    </div>
                </div>
                <div className="space-y-0.5">
                    <h3 className="text-sm font-semibold text-white leading-tight">{provider.name}</h3>
                    <p className="text-[11px] text-gray-400 leading-snug line-clamp-2">{getProviderDescription(provider.id)}</p>
                </div>
            </div>
        </button>
    );
}

function getProviderDescription(providerId: string): string {
    switch (providerId) {
        case "chutes":
            return "OpenAI-compatible inference for top open-source models";
        case "openai":
            return "GPT-5, GPT-4.1, and GPT-4o models for expressive RP";
        case "anthropic":
            return "Claude 4.5 Sonnet & Haiku for deep, emotional dialogue";
        case "nanogpt":
        case "featherless":
        case "openrouter":
            return "Access models like GPT-5, Claude 4.5, Grok-3, Mixtral, and more";
        case "openai-compatible":
            return "Use any OpenAI-style API endpoint";
        case "mistral":
            return "Mistral Small 3.2, Mixtral 8x22B, and other Mistral models";
        case "deepseek":
            return "DeepSeek-V3.2-Exp, DeepSeek-R1, and other high-efficiency models";
        case "xai":
            return "Grok-1.5, Grok-3, and newer xAI models";
        case "zai":
            return "GLM-4.5, GLM-4.6, and Air variants";
        case "moonshot":
            return "Kimi-K2 Thinking and Kimi-K1 models";
        case "gemini":
            return "Gemini 2.5 Flash, 2.5 Pro, and more";
        case "qwen":
            return "Qwen3-VL and newer Qwen models";
        case "nvidia":
            return "Nemotron, Llama, DeepSeek, and more via NVIDIA NIM";
        case "custom":
            return "Point LettuceAI to any custom model endpoint";
        default:
            return "AI model provider";
    }
}

function getProviderDescriptionShort(providerId: string): string {
    switch (providerId) {
        case "chutes":
            return "Open-source model inference";
        case "openai":
            return "GPT-5, GPT-4o, GPT-4.1";
        case "anthropic":
            return "Claude 4.5 Sonnet & Haiku";
        case "nanogpt":
        case "featherless":
        case "openrouter":
            return "Multi-model aggregator";
        case "openai-compatible":
            return "Custom OpenAI endpoint";
        case "mistral":
            return "Mistral & Mixtral models";
        case "deepseek":
            return "DeepSeek-V3, R1";
        case "xai":
            return "Grok-1.5, Grok-3";
        case "zai":
            return "GLM-4.5, GLM-4.6";
        case "moonshot":
            return "Kimi-K2 Thinking";
        case "gemini":
            return "Gemini 2.5 Flash & Pro";
        case "qwen":
            return "Qwen3-VL models";
        case "nvidia":
            return "NVIDIA NIM inference";
        case "custom":
            return "Custom endpoint";
        default:
            return "AI provider";
    }
}
