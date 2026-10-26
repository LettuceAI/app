import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { createMemoryHistory } from "@tanstack/react-router";
import { afterEach, describe, expect, it, vi } from "vitest";
import type {
  HfRecommendation,
  HfRecommendationRequest,
  ImageBundleAsset,
  ImageBundleProfile,
  ProviderAccountView,
} from "@/api/generated/bindings";
import { createMockTransport, mockAppStatus, type MockCallHandlers } from "@/api/mock-transport";
import { App } from "@/app/bootstrap/App";
import { createAppRuntime } from "@/app/bootstrap/runtime";
import { createTestI18n } from "@/shared/testing/i18n";

vi.mock("@/api/window-controls", () => ({
  createWindowController: vi.fn().mockResolvedValue(null),
}));
const ok = <T,>(data: T) => ({ status: "ok" as const, data });
const ollama: ProviderAccountView = {
  id: "desk",
  provider_kind: "ollama",
  protocol: "ollama",
  label: "Desk PC",
  base_url: "http://desk:11434",
  enabled: true,
  streaming_enabled: true,
  allow_invalid_tls: false,
  api_key_set: false,
  config: { kind: "ollama", sprout: null },
  revision: 1,
  created_at: 1,
  updated_at: 1,
};
async function mount(path: string, calls: MockCallHandlers = {}, mobile = false) {
  const transport = createMockTransport({
    calls: {
      appStatus: () =>
        ok({
          ...mockAppStatus,
          platform: mobile ? "android" : "other",
          ui_state: { onboarding: { completed: true } },
        }),
      providerAccountsList: () => ok([]),
      providerCatalog: () => ok({ providers: [] }),
      modelsList: () => ok({ models: [], default_model_id: null, revision: 1 }),
      jobsList: () => ok({ items: [], next_cursor: null }),
      hfAuthStatus: () => ok({ type: "missing" }),
      localModelsList: () => ok({ files: [] }),
      hfAvatars: () => ok({ avatars: [] }),
      hfBrowserLinks: () =>
        ok({
          repository_base: "https://huggingface.co",
          ollama_prefix: "hf.co/",
        }),
      ollamaModelsList: () => ok({ models: [] }),
      hfSearch: () => ok({ models: [] }),
      hfReadme: () => ok({ markdown: "# Real README" }),
      hfModelFiles: ({ model_id }) =>
        ok({
          model_id,
          revision: "pin",
          author: "owner",
          downloads: 12,
          likes: 3,
          tags: [],
          architecture: "llama",
          context_length: 32768,
          parameter_count: 12000000000,
          files: [
            {
              filename: "real-Q4_K_M.gguf",
              size: 1024,
              quantization: "Q4_K_M",
              is_mmproj: false,
              is_mtp: false,
              imatrix: false,
            },
          ],
        }),
      ...calls,
    },
  });
  const history = createMemoryHistory({ initialEntries: [path] });
  const runtime = await createAppRuntime({
    transport,
    history,
    i18n: await createTestI18n("en"),
  });
  await act(async () => {
    render(<App runtime={runtime} />);
  });
  return { runtime, transport, history };
}
afterEach(cleanup);

function nativeRecommendation(request: HfRecommendationRequest): HfRecommendation {
  const policy = {
    multi_gpu_enabled: false,
    gpu_device_ids: [],
    single_gpu_device_id: 2,
    distribution: null,
    manual_layers: [],
    priority_vram_limit_bytes: null,
  };
  return {
    preview: {
      devices: [],
      files: request.files.map((file) => ({
        filename: file.filename,
        nextn_predict_layers: 0,
        label: "excellent",
        max_context: 65536,
        recommended_context: 32768,
        gpu_optimal_context: 16384,
        kv_types: [{ kv_type: "q8_0", cache_bytes: 555, recommended_context: 32768 }],
      })),
      recommended_filename: request.files[0]?.filename ?? null,
      gpu_policy: policy,
      selected: request.plan
        ? {
            unified_memory: false,
            filename: request.plan.filename,
            context_length: request.plan.context_length,
            label: "excellent",
            gpu_layers: 17,
            total_layers: 17,
            offload_kqv: true,
            memory_fit: [
              {
                gpu: true,
                devices: ["Actual GPU"],
                capacity_bytes: 12000,
                available_bytes: 10000,
                model_bytes: 1000,
                context_bytes: 555,
                working_bytes: 100,
              },
            ],
            gpu_policy: policy,
            placement: { total_gpu_layers: 17, per_device_layers: [17] },
          }
        : null,
    },
    hardware_available: true,
    metadata_available: true,
    available_ram: 20000,
    available_vram: 10000,
    supports_gpu_offload: true,
    unified_memory: false,
    total_available: 30000,
    kv_base_per_token: null,
    kv_context_cap: null,
    model_max_context: 65536,
    arch: null,
    files: [],
    best: null,
    gpu_layer_count: null,
    kv_types: [],
    plan: null,
  };
}

describe("Hugging Face browser", () => {
  it("opens the pasted repo and browser Back restores search", async () => {
    const files = vi.fn<NonNullable<MockCallHandlers["hfModelFiles"]>>(({ model_id }) =>
      ok({
        model_id,
        revision: "pin",
        author: "real",
        downloads: 1,
        likes: 1,
        tags: [],
        architecture: null,
        context_length: null,
        parameter_count: null,
        files: [],
      }),
    );
    const { runtime, history } = await mount("/settings/model-browser", {
      hfModelFiles: files,
    });
    fireEvent.change(
      await screen.findByRole("searchbox", {
        name: "Search models or paste a link",
      }),
      {
        target: {
          value: "https://huggingface.co/real/repository/blob/main/file.gguf",
        },
      },
    );
    fireEvent.click(await screen.findByRole("button", { name: "Open real/repository" }));
    await waitFor(() =>
      expect(files).toHaveBeenCalledWith({
        model_id: "real/repository",
        mode: "llm",
      }),
    );
    expect(await screen.findByText("Real README")).toBeTruthy();
    await act(async () => history.back());
    await waitFor(() => expect(runtime.router.state.location.search.repo).toBeUndefined());
    expect(
      await screen.findByRole("searchbox", {
        name: "Search models or paste a link",
      }),
    ).toBeTruthy();
  });
  it("pulls the selected quant on mobile without asking for unavailable Sprout estimates", async () => {
    const pull = vi.fn<NonNullable<MockCallHandlers["ollamaPull"]>>(() =>
      ok({ job_id: "pull-job" }),
    );
    const recommend = vi.fn<NonNullable<MockCallHandlers["hfRecommendation"]>>();
    await mount(
      "/settings/model-browser?repo=owner%2Fmodel&tab=download",
      {
        providerAccountsList: () => ok([ollama]),
        ollamaPull: pull,
        hfRecommendation: recommend,
      },
      true,
    );
    expect(await screen.findByText("No recommendations without Sprout")).toBeTruthy();
    expect(screen.queryByRole("switch", { name: "Image support" })).toBeNull();
    expect(screen.queryByText("KV cache type")).toBeNull();
    expect(screen.queryByRole("tab", { name: "Image models" })).toBeNull();
    fireEvent.click(await screen.findByRole("button", { name: /Q4_K_M/ }));
    fireEvent.click((await screen.findAllByRole("button", { name: "Pull to Desk PC" })).at(-1)!);
    await waitFor(() =>
      expect(pull).toHaveBeenCalledWith(
        expect.objectContaining({
          provider_account_id: "desk",
          model: "hf.co/owner/model:Q4_K_M",
        }),
      ),
    );
    expect(recommend).not.toHaveBeenCalled();
  });
  it("fences planner changes and submits only the current native context, Q8 cache and GPU policy", async () => {
    let release: (() => void) | undefined;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const recommendation = vi.fn<NonNullable<MockCallHandlers["hfRecommendation"]>>(
      async (request) => {
        if (request.plan?.context_length === 32768) await gate;
        return ok(nativeRecommendation(request));
      },
    );
    const download = vi.fn<NonNullable<MockCallHandlers["hfDownload"]>>(() =>
      ok({ job_id: "installed" }),
    );
    await mount("/settings/model-browser?repo=owner%2Fmodel&tab=download", {
      hfRecommendation: recommendation,
      hfDownload: download,
    });
    const button = await screen.findByRole("button", { name: "Download" });
    await waitFor(() => expect(button.hasAttribute("disabled")).toBe(false));
    expect(screen.queryByRole("combobox", { name: "KV cache type" })).toBeNull();
    fireEvent.change(screen.getByRole("slider", { name: "Context" }), {
      target: { value: "32768" },
    });
    await waitFor(() =>
      expect(
        recommendation.mock.calls.some(([request]) => request.plan?.context_length === 32768),
      ).toBe(true),
    );
    expect(button.hasAttribute("disabled")).toBe(true);
    fireEvent.click(button);
    expect(download).not.toHaveBeenCalled();
    await act(async () => release?.());
    await waitFor(() => expect(button.hasAttribute("disabled")).toBe(false));
    fireEvent.click(button);
    await waitFor(() =>
      expect(download).toHaveBeenCalledWith(
        expect.objectContaining({
          revision: "pin",
          setup: expect.objectContaining({
            context_length: 32768,
            kv_type: "q8_0",
            gpu_layers: 17,
            gpu_policy: expect.objectContaining({ single_gpu_device_id: 2 }),
          }),
        }),
      ),
    );
  });

  it("uses the selected file’s bundled MTP capability consistently when an external draft also exists", async () => {
    const recommendation = vi.fn<NonNullable<MockCallHandlers["hfRecommendation"]>>((request) => {
      const data = nativeRecommendation(request);
      data.preview!.files.forEach((file) => {
        file.nextn_predict_layers = file.filename.startsWith("real-") ? 2 : 0;
      });
      return ok(data);
    });
    const download = vi.fn<NonNullable<MockCallHandlers["hfDownload"]>>(() =>
      ok({ job_id: "mtp" }),
    );
    await mount("/settings/model-browser?repo=owner%2Fmodel&tab=download", {
      hfRecommendation: recommendation,
      hfDownload: download,
      hfModelFiles: () =>
        ok({
          model_id: "owner/model",
          revision: "pin",
          author: "owner",
          downloads: 1,
          likes: 1,
          tags: [],
          architecture: null,
          context_length: 65536,
          parameter_count: null,
          files: [
            {
              filename: "real-Q4_K_M.gguf",
              size: 1000,
              quantization: "Q4_K_M",
              is_mmproj: false,
              is_mtp: false,
              imatrix: false,
            },
            {
              filename: "other-Q5_K_M.gguf",
              size: 1500,
              quantization: "Q5_K_M",
              is_mmproj: false,
              is_mtp: false,
              imatrix: false,
            },
            {
              filename: "external-MTP.gguf",
              size: 100,
              quantization: "Q8_0",
              is_mmproj: false,
              is_mtp: true,
              imatrix: false,
            },
          ],
        }),
    });
    const button = await screen.findByRole("button", { name: "Download" });
    await waitFor(() => expect(button.hasAttribute("disabled")).toBe(false));
    fireEvent.click(screen.getByRole("switch", { name: "Multi-token prediction" }));
    await waitFor(() =>
      expect(
        recommendation.mock.calls.some(
          ([request]) => request.sidecars?.mtp_bundled && request.sidecars.mtp === null,
        ),
      ).toBe(true),
    );
    await waitFor(() => expect(button.hasAttribute("disabled")).toBe(false));
    fireEvent.click(button);
    await waitFor(() =>
      expect(download).toHaveBeenLastCalledWith(
        expect.objectContaining({ mtp_bundled: true, mtp_file: null }),
      ),
    );
    fireEvent.click(screen.getByRole("button", { name: /Q5_K_M/ }));
    await waitFor(() =>
      expect(
        recommendation.mock.calls.some(
          ([request]) =>
            request.plan?.filename === "other-Q5_K_M.gguf" &&
            request.sidecars?.mtp?.filename === "external-MTP.gguf" &&
            !request.sidecars.mtp_bundled,
        ),
      ).toBe(true),
    );
    await waitFor(() => expect(button.hasAttribute("disabled")).toBe(false));
    fireEvent.click(button);
    await waitFor(() =>
      expect(download).toHaveBeenLastCalledWith(
        expect.objectContaining({
          file: "other-Q5_K_M.gguf",
          mtp_bundled: false,
          mtp_file: "external-MTP.gguf",
        }),
      ),
    );
  });

  it("adds the projector’s actual reserve and sends only the typed sidecar fields", async () => {
    const recommendation = vi.fn<NonNullable<MockCallHandlers["hfRecommendation"]>>((request) =>
      ok(nativeRecommendation(request)),
    );
    const download = vi.fn<NonNullable<MockCallHandlers["hfDownload"]>>(() =>
      ok({ job_id: "vision" }),
    );
    await mount("/settings/model-browser?repo=owner%2Fmodel&tab=download", {
      hfRecommendation: recommendation,
      hfDownload: download,
      hfModelFiles: () =>
        ok({
          model_id: "owner/model",
          revision: "pin",
          author: "owner",
          downloads: 1,
          likes: 1,
          tags: [],
          architecture: null,
          context_length: 65536,
          parameter_count: null,
          files: [
            {
              filename: "real-Q4_K_M.gguf",
              size: 1000,
              quantization: "Q4_K_M",
              is_mmproj: false,
              is_mtp: false,
              imatrix: false,
            },
            {
              filename: "projection.gguf",
              size: 850000000,
              quantization: "F16",
              is_mmproj: true,
              is_mtp: false,
              imatrix: false,
            },
          ],
        }),
    });
    const button = await screen.findByRole("button", { name: "Download" });
    await waitFor(() => expect(button.hasAttribute("disabled")).toBe(false));
    fireEvent.click(screen.getByRole("switch", { name: "Image support" }));
    await waitFor(() =>
      expect(
        recommendation.mock.calls.some(
          ([request]) =>
            JSON.stringify(request.sidecars?.mmproj) ===
            JSON.stringify({ filename: "projection.gguf", size: 850000000 }),
        ),
      ).toBe(true),
    );
    expect(screen.queryByText("projection.gguf")).toBeNull();
    await waitFor(() => expect(button.hasAttribute("disabled")).toBe(false));
    fireEvent.click(button);
    await waitFor(() =>
      expect(download).toHaveBeenCalledWith(
        expect.objectContaining({ mmproj_file: "projection.gguf" }),
      ),
    );
  });

  it("makes a model from an unused local file without losing it when no local account exists", async () => {
    const accounts: ProviderAccountView[] = [];
    const account: ProviderAccountView = {
      ...ollama,
      id: "local",
      provider_kind: "llamacpp",
      protocol: "llama_cpp",
      label: "llama.cpp (Local)",
      base_url: null,
      config: { kind: "standard" },
    };
    const saveAccount = vi.fn<NonNullable<MockCallHandlers["providerAccountSave"]>>(() => {
      accounts.push(account);
      return ok(account);
    });
    const saved = {
      id: "made",
      provider_account_id: "local",
      external_model_id: "/models/unused.gguf",
      display_name: "unused",
      kind: "chat" as const,
      config: {},
      input_scopes: ["text" as const],
      output_scopes: ["text" as const],
      revision: 1,
      created_at: 1,
      updated_at: 1,
    };
    const saveModel = vi.fn<NonNullable<MockCallHandlers["modelSave"]>>(() => ok(saved));
    const { runtime } = await mount("/settings/model-browser?view=downloaded", {
      providerAccountsList: () => ok(accounts),
      providerAccountSave: saveAccount,
      modelSave: saveModel,
      modelGet: () => ok(saved),
      llamaDevices: () => ok({ devices: [] }),
      localModelsDirGet: () =>
        ok({
          path: "/models",
          default_path: "/models",
          is_custom: false,
          model_count: 1,
        }),
      localModelsList: () =>
        ok({
          files: [
            {
              repo: "owner/model",
              filename: "unused.gguf",
              path: "/models/unused.gguf",
              size: 1000,
              quantization: "Q8_0",
              is_mmproj: false,
              is_mtp: false,
              is_dflash: false,
              architecture: "llama",
              context_length: 65536,
              used_by: [],
            },
          ],
        }),
    });
    fireEvent.click(await screen.findByRole("button", { name: "unused.gguf" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Make a model from it" }));
    await waitFor(() =>
      expect(saveAccount).toHaveBeenCalledWith(
        expect.objectContaining({
          account: expect.objectContaining({
            provider_kind: "llamacpp",
            enabled: true,
          }),
        }),
      ),
    );
    await waitFor(() =>
      expect(saveModel).toHaveBeenCalledWith(
        expect.objectContaining({
          model: expect.objectContaining({
            external_model_id: "/models/unused.gguf",
            provider_account_id: "local",
          }),
        }),
      ),
    );
    await waitFor(() =>
      expect(runtime.router.state.location.pathname).toBe("/settings/models/made"),
    );
  });

  it("explains missing Ollama on mobile and offers provider setup", async () => {
    await mount("/settings/model-browser", {}, true);
    expect(await screen.findByText("Add an Ollama server to get models")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Open providers" })).toBeTruthy();
    expect(screen.queryByText("This computer")).toBeNull();
  });
  it("advances through actual required image roles and requires a fit before installation", async () => {
    const profile: ImageBundleProfile = {
      id: "real-profile",
      display_name: "Real image model",
      family: "FLUX",
      description: "From the backend",
      required_roles: ["diffusion_model", "vae"],
      minimum_runtime_build: null,
      diffusion_markers: [],
      encoder_markers: [],
      encoder_parameter_billions: null,
      recommended_repositories: [{ role: "diffusion_model", repository: "owner/component" }],
      supports_text_to_image: true,
      supports_image_edit: false,
      max_reference_images: null,
      requires_reference_image: false,
      recommended_for_scenes: true,
      default_width: 1024,
      default_height: 1024,
      default_steps: 4,
      default_cfg: 1,
    };
    const file = (role: ImageBundleAsset["role"]): ImageBundleAsset => ({
      selection_id: role,
      profile_id: profile.id,
      role,
      model_id: "owner/component",
      revision: "real-pin",
      relative_path: `${role}.gguf`,
      format: "gguf",
      quantization: "Q8_0",
      size: 500,
      sha256: "verified-hash",
      architecture: "flux",
      gated: false,
    });
    const install = vi.fn<NonNullable<MockCallHandlers["hfImageBundleInstall"]>>();
    const { runtime } = await mount("/settings/model-browser?kind=image", {
      hfImageBundleProfiles: () => ok({ profiles: [profile] }),
      sdRuntimeInventory: () =>
        ok({
          installed: [
            {
              release: "runtime",
              asset: "engine.zip",
              backend: "Vulkan",
              size_bytes: 1000,
              active: true,
            },
          ],
          active: { release: "runtime", asset: "engine.zip" },
        }),
      hfImageBundleSearch: () =>
        ok({
          models: [
            {
              model_id: "owner/component",
              author: "owner",
              likes: 1,
              downloads: 2,
              tags: [],
              pipeline_tag: null,
              last_modified: null,
              trending_score: null,
            },
          ],
        }),
      hfImageBundleFiles: ({ role }) => ok({ assets: [file(role)] }),
      hfImageBundleInstall: install,
    });
    fireEvent.click(await screen.findByRole("button", { name: /^Real image model/ }));
    fireEvent.click(await screen.findByRole("button", { name: /owner\/component/ }));
    fireEvent.click(await screen.findByRole("button", { name: "Use this file" }));
    await waitFor(() => expect(runtime.router.state.location.search.role).toBe("vae"));
    fireEvent.click(await screen.findByRole("button", { name: /owner\/component/ }));
    fireEvent.click(await screen.findByRole("button", { name: "Use this file" }));
    await waitFor(() => expect(runtime.router.state.location.search.imageStep).toBe("review"));
    expect(screen.getByRole("button", { name: "Download and add" }).hasAttribute("disabled")).toBe(
      true,
    );
    expect(screen.getByDisplayValue("Real image model")).toBeTruthy();
    expect(install).not.toHaveBeenCalled();
  });
});
