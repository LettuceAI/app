export type SdcppLoraFile = {
  filename: string;
  path: string;
  bytesOnDisk: number;
  keywords: string[];
  keywordSource: "metadata" | "civitai" | "manual" | "none";
  architecture: string | null;
  architectureSource: "metadata" | "civitai" | "none";
  compatibility: "compatible" | "incompatible" | "unknown";
};

export type SdcppLoraKeywordDiscovery = {
  keywords: string[];
  source: "metadata" | "civitai" | "manual" | "none";
  sha256: string | null;
  architecture: string | null;
  architectureSource: "metadata" | "civitai" | "none";
  compatibility: "compatible" | "incompatible" | "unknown";
};

export function parseLoraKeywordDraft(value: string): string[] {
  const seen = new Set<string>();
  const keywords: string[] = [];
  for (const part of value.split(/[\n,;]+/)) {
    const keyword = part.trim();
    const key = keyword.toLowerCase();
    if (!keyword || seen.has(key)) continue;
    seen.add(key);
    keywords.push(keyword);
    if (keywords.length === 32) break;
  }
  return keywords;
}

export function loraArchitectureLabel(
  architecture: string | null | undefined,
  unknownLabel: string,
): string {
  switch (architecture) {
    case "z-image": return "Z-Image";
    case "flux2-klein-4b": return "FLUX.2 Klein 4B";
    case "flux2-klein-9b": return "FLUX.2 Klein 9B";
    case "flux2": return "FLUX.2";
    case "flux1": return "FLUX.1";
    case "flux": return "FLUX";
    case "krea-2": return "Krea 2";
    case "qwen-image-edit-2511": return "Qwen Image Edit 2511";
    case "qwen-image-edit": return "Qwen Image Edit";
    case "qwen-image": return "Qwen Image";
    case "sdxl": return "Stable Diffusion XL";
    case "sd3": return "Stable Diffusion 3";
    case "sd2": return "Stable Diffusion 2";
    case "sd1": return "Stable Diffusion 1.x";
    case "pony": return "Pony";
    case "illustrious": return "Illustrious";
    case "noobai": return "NoobAI";
    default: return unknownLabel;
  }
}
