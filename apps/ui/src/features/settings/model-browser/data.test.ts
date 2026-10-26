import { describe, expect, it } from "vitest";
import { fitSettingsKey, contextValue, hasSprout, parameterBillions, repoFromLink } from "./data";

describe("Hugging Face browser input", () => {
  it("reuses fit badges across file switches only for the same GPU, context and sidecar settings", () => {
    const plan = {
      filename: "one.gguf",
      context_length: 8192,
      kv_type: "q8_0",
      model_offload: "auto" as const,
      kv_placement: "auto" as const,
    };
    const initial = fitSettingsKey({ gpu_policy: null, sidecars: null, plan: null });
    const current = {
      gpu_policy: null,
      sidecars: { mmproj: null, mtp: null, mtp_bundled: false },
      plan,
    };
    expect(fitSettingsKey(current)).toBe(initial);
    expect(fitSettingsKey({ ...current, plan: { ...plan, filename: "two.gguf" } })).toBe(initial);
    expect(fitSettingsKey({ ...current, plan: { ...plan, context_length: 32768 } })).not.toBe(
      initial,
    );
    expect(
      fitSettingsKey({ ...current, sidecars: { ...current.sidecars, mtp_bundled: true } }),
    ).not.toBe(initial);
    expect(
      fitSettingsKey({
        ...current,
        gpu_policy: {
          multi_gpu_enabled: true,
          gpu_device_ids: [2, 7],
          single_gpu_device_id: null,
          distribution: null,
          manual_layers: [],
          priority_vram_limit_bytes: null,
        },
      }),
    ).not.toBe(initial);
  });

  it("opens the repository from actual Hugging Face URLs, including file links", () => {
    expect(
      repoFromLink(
        "https://huggingface.co/real-owner/real-repo/blob/main/model.gguf?download=true",
        "https://huggingface.co",
      ),
    ).toBe("real-owner/real-repo");
    expect(
      repoFromLink("https://huggingface.co.evil.test/owner/repo", "https://huggingface.co"),
    ).toBeNull();
    expect(repoFromLink("https://example.org/owner/repo", "https://huggingface.co")).toBeNull();
    expect(repoFromLink("javascript:alert(1)", "https://huggingface.co")).toBeNull();
  });
  it("parses exact tokens and k inputs, rounds and clamps without accepting malformed drafts", () => {
    expect(contextValue("32,768", 131072)).toBe(32768);
    expect(contextValue("48k", 131072)).toBe(49152);
    expect(contextValue("1", 131072)).toBe(2048);
    expect(contextValue("9999999", 131072)).toBe(131072);
    expect(contextValue("33000", 131072)).toBe(33024);
    expect(contextValue("32junk", 131072)).toBeNull();
    expect(contextValue("", 131072)).toBeNull();
  });
  it("uses repo-name parameter metadata only when a size is actually present", () => {
    expect(parameterBillions("org/Model-12B-GGUF")).toBe(12);
    expect(parameterBillions("org/Model-1.5b")).toBe(1.5);
    expect(parameterBillions("org/Model-without-size")).toBeNull();
  });
  it("requires the actual enabled Sprout configuration and a nonempty endpoint", () => {
    const account = (sprout: unknown) => ({
      config: { kind: "ollama", sprout },
    });
    expect(hasSprout(undefined)).toBe(true);
    expect(hasSprout(account({ enabled: true, url: "http://host:3000" }))).toBe(true);
    expect(hasSprout(account({ enabled: false, url: "http://host:3000" }))).toBe(false);
    expect(hasSprout(account({ enabled: true, url: "  " }))).toBe(false);
  });
});
