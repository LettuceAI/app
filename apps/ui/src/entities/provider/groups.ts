import type { ProviderDescriptorContract } from "@/api/generated/bindings";

export type ProviderGroup = "cloud" | "local" | "custom";

export const providerGroups: readonly ProviderGroup[] = ["cloud", "local", "custom"];

const customKinds: ReadonlySet<string> = new Set(["custom", "custom-anthropic"]);

/**
 * Providers that need a key and have no fixed address are "custom"; those with no required key are
 * "local". The custom kinds also have an optional key, so they are named here.
 */
export function providerGroup(descriptor: ProviderDescriptorContract): ProviderGroup {
  if (customKinds.has(descriptor.kind)) return "custom";
  if (descriptor.api_key !== "required") return "local";
  return descriptor.default_endpoint === null ? "custom" : "cloud";
}

/** The host a provider's default address points at, or null when it has none. */
export function providerHost(descriptor: ProviderDescriptorContract): string | null {
  const endpoint = descriptor.default_endpoint;
  if (!endpoint) return null;
  try {
    return new URL(endpoint).host;
  } catch {
    return endpoint;
  }
}
