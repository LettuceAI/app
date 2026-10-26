import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { createMemoryHistory } from "@tanstack/react-router";
import { afterEach, describe, expect, it, vi } from "vitest";
import type {
  HfRecommendation_Serialize as HfRecommendation,
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
          last_modified: null,
          files: [
            {
              filename: "real-Q4_K_M.gguf",
              display_name: null,
              parts: [],
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
      files: (request.plan
        ? request.files.filter((file) => file.filename === request.plan?.filename)
        : request.files.slice(0, 1)
      ).map((file) => ({
        filename: file.filename,
        nextn_predict_layers: 0,
        label: "excellent",
        max_context: 65536,
        recommended_context: 32768,
        gpu_optimal_context: 16384,
        kv_types: [{ kv_type: "q8_0", cache_bytes: 555, recommended_context: 32768 }],
      })),
      recommended_filename: request.files[0]?.filename ?? null,
      recommended_kv_type: "q8_0",
      upgrade: null,
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
            gpu_mode: "full",
            prefill_speed: null,
            generation_speed: null,
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

function logicalSplitInfo() {
  return {
    model_id: "owner/model",
    revision: "pin",
    author: "owner",
    downloads: 1,
    likes: 1,
    tags: [],
    architecture: null,
    context_length: 65536,
    parameter_count: null,
    last_modified: null,
    files: [
      {
        filename: "BF16/model-BF16-00001-of-00002.gguf",
        display_name: "BF16/model-BF16.gguf",
        size: 3000,
        quantization: "BF16",
        is_mmproj: false,
        is_mtp: false,
        imatrix: false,
        parts: [
          { filename: "BF16/model-BF16-00001-of-00002.gguf", size: 2000 },
          { filename: "BF16/model-BF16-00002-of-00002.gguf", size: 1000 },
        ],
      },
    ],
  };
}

describe("Hugging Face browser", () => {
  it("shows structured pending file and memory layouts without invented fit values", async () => {
    const never = new Promise<Awaited<ReturnType<NonNullable<MockCallHandlers["hfModelFiles"]>>>>(
      () => {},
    );
    await mount("/settings/model-browser?repo=owner%2Fmodel&tab=download", {
      hfModelFiles: () => never,
    });
    const pending = await screen.findByRole("status", {
      name: "Checking model headers and memory fit…",
    });
    expect(pending.getAttribute("aria-busy")).toBe("true");
    expect(screen.getByText("Files")).toBeTruthy();
    expect(screen.getAllByText("Memory").length).toBeGreaterThan(0);
    expect(screen.queryByText("Full speed")).toBeNull();
    expect(screen.queryByText("Cache size unavailable")).toBeNull();
  });

  it("keeps one logical split choice and submits actual first-part identity with all metadata parts", async () => {
    const recommend = vi.fn<NonNullable<MockCallHandlers["hfRecommendation"]>>((request) =>
      ok(nativeRecommendation(request)),
    );
    const download = vi.fn<NonNullable<MockCallHandlers["hfDownload"]>>(() =>
      ok({ job_id: "split" }),
    );
    await mount("/settings/model-browser?repo=owner%2Fmodel&tab=download", {
      hfModelFiles: () => ok(logicalSplitInfo()),
      hfRecommendation: recommend,
      hfDownload: download,
    });
    await screen.findByText("BF16/model-BF16.gguf");
    expect(screen.queryByText("BF16/model-BF16-00001-of-00002.gguf")).toBeNull();
    expect(screen.queryByText("BF16/model-BF16-00002-of-00002.gguf")).toBeNull();
    expect(screen.getByText("2 parts")).toBeTruthy();
    const button = screen.getByRole("button", { name: "Download BF16" });
    await waitFor(() => expect(button.hasAttribute("disabled")).toBe(false));
    const selected = recommend.mock.calls.find(([request]) => request.plan)?.[0];
    expect(selected?.files).toEqual(
      logicalSplitInfo().files.map(({ filename, size, parts }) => ({ filename, size, parts })),
    );
    fireEvent.click(button);
    await waitFor(() =>
      expect(download).toHaveBeenCalledWith(
        expect.objectContaining({ file: "BF16/model-BF16-00001-of-00002.gguf" }),
      ),
    );
  });

  it("uses compact choice rows, segmented placement controls and actual GPU/RAM weight pools", async () => {
    const recommend = vi.fn<NonNullable<MockCallHandlers["hfRecommendation"]>>((request) => {
      const result = nativeRecommendation(request);
      if (result.preview?.selected) {
        result.preview.selected.gpu_mode = "few_layers";
        result.preview.selected.memory_fit.unshift({
          gpu: false,
          devices: [],
          capacity_bytes: 25000,
          available_bytes: 20000,
          model_bytes: 8500,
          context_bytes: 250,
          working_bytes: 777,
        });
      }
      return ok(result);
    });
    await mount("/settings/model-browser?repo=owner%2Fmodel&tab=download", {
      hfRecommendation: recommend,
    });
    await screen.findByText("Mostly in system memory");
    const gpu = screen.getByText("Actual GPU VRAM");
    const ram = screen.getByText("System RAM");
    expect(gpu.compareDocumentPosition(ram) & Node.DOCUMENT_POSITION_FOLLOWING).not.toBe(0);
    expect(screen.getAllByText(/777/).length).toBeGreaterThan(0);
    const row = screen.getByRole("button", { name: /real-Q4_K_M.gguf/ });
    expect(row.getAttribute("aria-pressed")).toBe("true");
    const offload = screen.getByRole("radiogroup", { name: "Model offload" });
    expect(within(offload).getAllByRole("radio").length).toBe(4);
    expect(screen.getByRole("radiogroup", { name: "KV cache location" })).toBeTruthy();
    fireEvent.click(within(offload).getByRole("radio", { name: "CPU" }));
    expect(screen.getByRole("button", { name: "Download Q4_K_M" }).hasAttribute("disabled")).toBe(
      true,
    );
    await waitFor(() =>
      expect(recommend.mock.calls.some(([request]) => request.plan?.model_offload === "cpu")).toBe(
        true,
      ),
    );
  });

  it("edits the context as typed tokens, clamps it and marks the full-speed limit", async () => {
    await mount("/settings/model-browser?repo=owner%2Fmodel&tab=download", {
      hfRecommendation: (request) => ok(nativeRecommendation(request)),
    });
    expect(await screen.findByRole("button", { name: "16,384" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Max 65,536" })).toBeTruthy();
    fireEvent.click(await screen.findByRole("button", { name: "8,192" }));
    const input = screen.getByRole("textbox", { name: "Context length in tokens" });
    fireEvent.change(input, { target: { value: "48k" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(await screen.findByRole("button", { name: "49,152" })).toBeTruthy();
    expect(await screen.findByText("Spills to RAM, slower start")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "49,152" }));
    const again = screen.getByRole("textbox", { name: "Context length in tokens" });
    fireEvent.change(again, { target: { value: "9999999" } });
    fireEvent.keyDown(again, { key: "Enter" });
    expect(await screen.findByRole("button", { name: "65,536" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "65,536" }));
    const cancelled = screen.getByRole("textbox", { name: "Context length in tokens" });
    fireEvent.change(cancelled, { target: { value: "4096" } });
    fireEvent.keyDown(cancelled, { key: "Escape" });
    expect(await screen.findByRole("button", { name: "65,536" })).toBeTruthy();
  });

  it("removes stale recommendations and blocks downloading a context that does not fit", async () => {
    const download = vi.fn<NonNullable<MockCallHandlers["hfDownload"]>>(() =>
      ok({ job_id: "download" }),
    );
    await mount("/settings/model-browser?repo=owner%2Fmodel&tab=download", {
      hfDownload: download,
      hfRecommendation: (request) => {
        const result = nativeRecommendation(request);
        if (request.plan && request.plan.context_length > 16384 && result.preview?.selected) {
          result.preview.selected.label = "unrunnable";
          result.preview.files[0]!.label = "unrunnable";
          result.preview.recommended_filename = null;
        }
        return ok(result);
      },
    });
    const button = await screen.findByRole("button", { name: "Download Q4_K_M" });
    await waitFor(() => expect(button.hasAttribute("disabled")).toBe(false));
    expect(screen.getByText("Recommended")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "8,192" }));
    const input = screen.getByRole("textbox", { name: "Context length in tokens" });
    fireEvent.change(input, { target: { value: "32k" } });
    fireEvent.keyDown(input, { key: "Enter" });
    await screen.findAllByText("Will not run");
    expect(button.hasAttribute("disabled")).toBe(true);
    expect(screen.queryByText("Recommended")).toBeNull();
    expect(screen.getByText("Does not fit")).toBeTruthy();
    fireEvent.click(button);
    expect(download).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "32,768" }));
    const restored = screen.getByRole("textbox", { name: "Context length in tokens" });
    fireEvent.change(restored, { target: { value: "8192" } });
    fireEvent.keyDown(restored, { key: "Enter" });
    await waitFor(() => expect(button.hasAttribute("disabled")).toBe(false));
    expect(screen.getByText("Recommended")).toBeTruthy();
  });

  it("starts with the recommended Q6 cache and lets manual Q4 reach the calculator and download", async () => {
    const recommend = vi.fn<NonNullable<MockCallHandlers["hfRecommendation"]>>((request) => {
      const result = nativeRecommendation(request);
      if (result.preview) {
        result.preview.recommended_kv_type = "q6_k";
        for (const file of result.preview.files)
          file.kv_types = ["f16", "q8_0", "q6_k", "q4_0"].map((kv_type) => ({
            kv_type,
            cache_bytes: 123,
            recommended_context: 32768,
          }));
      }
      return ok(result);
    });
    const download = vi.fn<NonNullable<MockCallHandlers["hfDownload"]>>(() =>
      ok({ job_id: "download" }),
    );
    await mount("/settings/model-browser?repo=owner%2Fmodel&tab=download", {
      hfRecommendation: recommend,
      hfDownload: download,
    });
    const picker = await screen.findByRole("combobox", { name: "KV cache type" });
    await waitFor(() =>
      expect(recommend.mock.calls.some(([request]) => request.plan?.kv_type === "q6_k")).toBe(true),
    );
    fireEvent.click(picker);
    fireEvent.click(await screen.findByRole("option", { name: /Q4_0/ }));
    await waitFor(() =>
      expect(recommend.mock.calls.some(([request]) => request.plan?.kv_type === "q4_0")).toBe(true),
    );
    const button = screen.getByRole("button", { name: "Download Q4_K_M" });
    await waitFor(() => expect(button.hasAttribute("disabled")).toBe(false));
    fireEvent.click(button);
    await waitFor(() =>
      expect(download).toHaveBeenCalledWith(
        expect.objectContaining({ setup: expect.objectContaining({ kv_type: "q4_0" }) }),
      ),
    );
  });

  it("filters a long file list inside a fixed-height scroller", async () => {
    const info = logicalSplitInfo();
    const files = Array.from({ length: 12 }, (_, index) => ({
      ...info.files[0]!,
      filename: `model-Q${index + 1}_K.gguf`,
      display_name: null,
      parts: [],
      quantization: `Q${index + 1}_K`,
      size: 1000 + index,
    }));
    await mount("/settings/model-browser?repo=owner%2Fmodel&tab=download", {
      hfModelFiles: () => ok({ ...info, files }),
      hfRecommendation: (request) => ok(nativeRecommendation(request)),
    });
    const filter = await screen.findByRole("searchbox", { name: "Files" });
    expect(screen.getByText("12 of 12")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Fits" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: /show all/i })).toBeNull();
    fireEvent.change(filter, { target: { value: "Q11" } });
    expect(await screen.findByText("1 of 12")).toBeTruthy();
  });

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
        last_modified: null,
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
    const button = await screen.findByRole("button", { name: "Download Q4_K_M" });
    await waitFor(() => expect(button.hasAttribute("disabled")).toBe(false));
    expect(screen.getByText("Half of F16, barely different")).toBeTruthy();
    fireEvent.change(screen.getByRole("slider", { name: "Context length" }), {
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
          last_modified: null,
          files: [
            {
              filename: "real-Q4_K_M.gguf",
              display_name: null,
              parts: [],
              size: 1000,
              quantization: "Q4_K_M",
              is_mmproj: false,
              is_mtp: false,
              imatrix: false,
            },
            {
              filename: "other-Q5_K_M.gguf",
              display_name: null,
              parts: [],
              size: 1500,
              quantization: "Q5_K_M",
              is_mmproj: false,
              is_mtp: false,
              imatrix: false,
            },
            {
              filename: "external-MTP.gguf",
              display_name: null,
              parts: [],
              size: 100,
              quantization: "Q8_0",
              is_mmproj: false,
              is_mtp: true,
              imatrix: false,
            },
          ],
        }),
    });
    const button = await screen.findByRole("button", { name: "Download Q4_K_M" });
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
          last_modified: null,
          files: [
            {
              filename: "real-Q4_K_M.gguf",
              display_name: null,
              parts: [],
              size: 1000,
              quantization: "Q4_K_M",
              is_mmproj: false,
              is_mtp: false,
              imatrix: false,
            },
            {
              filename: "projection.gguf",
              display_name: null,
              parts: [],
              size: 850000000,
              quantization: "F16",
              is_mmproj: true,
              is_mtp: false,
              imatrix: false,
            },
          ],
        }),
    });
    const button = await screen.findByRole("button", { name: "Download Q4_K_M" });
    await waitFor(() => expect(button.hasAttribute("disabled")).toBe(false));
    fireEvent.click(screen.getByRole("switch", { name: "Image support" }));
    await waitFor(() =>
      expect(
        recommendation.mock.calls.some(
          ([request]) =>
            JSON.stringify(request.sidecars?.mmproj) ===
            JSON.stringify({ filename: "projection.gguf", size: 850000000, parts: [] }),
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
    expect(await screen.findByText("Connect an Ollama server")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Add Ollama" })).toBeTruthy();
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
  it("shows the planner speeds for the placement and hides them when there is no rating", async () => {
    await mount("/settings/model-browser?repo=owner%2Fmodel&tab=download", {
      hfRecommendation: (request) => {
        const result = nativeRecommendation(request);
        const selected = result.preview?.selected;
        if (selected) {
          selected.prefill_speed = "fast";
          selected.generation_speed = "medium";
        }
        return ok(result);
      },
    });
    await screen.findByText("All on the GPU");
    const prefill = screen.getByText("Prefill");
    const generation = screen.getByText("Generation");
    expect(prefill.nextElementSibling?.textContent).toBe("Fast");
    expect(generation.nextElementSibling?.textContent).toBe("Medium");
    cleanup();
    await mount("/settings/model-browser?repo=owner%2Fmodel&tab=download", {
      hfRecommendation: (request) => {
        const result = nativeRecommendation(request);
        if (result.preview?.selected) result.preview.selected.unified_memory = true;
        return ok(result);
      },
    });
    await screen.findByText("GPU with shared memory");
    expect(screen.queryByText("Prefill")).toBeNull();
    expect(screen.queryByText("Generation")).toBeNull();
    expect(screen.queryByText("Unknown")).toBeNull();
    expect(screen.getByText("Will run")).toBeTruthy();
  });

  it("offers a better quant that runs as well and selects it on request", async () => {
    const recommend = vi.fn<NonNullable<MockCallHandlers["hfRecommendation"]>>((request) => {
      const result = nativeRecommendation(request);
      if (result.preview && request.plan?.filename === "real-Q4_K_M.gguf")
        result.preview.upgrade = "real-Q6_K.gguf";
      return ok(result);
    });
    const file = (quantization: string, size: number) => ({
      filename: `real-${quantization}.gguf`,
      display_name: null,
      parts: [],
      size,
      quantization,
      is_mmproj: false,
      is_mtp: false,
      imatrix: false,
    });
    await mount("/settings/model-browser?repo=owner%2Fmodel&tab=download", {
      hfRecommendation: recommend,
      hfModelFiles: ({ model_id }) =>
        ok({
          ...logicalSplitInfo(),
          model_id,
          files: [file("Q4_K_M", 1024), file("Q6_K", 2 * 1024 ** 3)],
        }),
    });
    expect(await screen.findByText("Q6_K runs well too")).toBeTruthy();
    expect(screen.getByText(/^2(\.0)? GB, closer to the original model\.$/)).toBeTruthy();
    const plans = () => recommend.mock.calls.map(([request]) => request).filter((request) => request.plan);
    expect(plans()[0]?.files.map((item) => item.filename)).toEqual([
      "real-Q4_K_M.gguf",
      "real-Q6_K.gguf",
    ]);
    fireEvent.click(screen.getByRole("button", { name: "Use Q6_K" }));
    await waitFor(() =>
      expect(plans().some((request) => request.plan?.filename === "real-Q6_K.gguf")).toBe(true),
    );
    await waitFor(() => expect(screen.queryByText("Q6_K runs well too")).toBeNull());
    expect(
      screen.getByRole("button", { name: /real-Q6_K.gguf/ }).getAttribute("aria-pressed"),
    ).toBe("true");
  });

  it("hides the upgrade suggestion when there is none", async () => {
    await mount("/settings/model-browser?repo=owner%2Fmodel&tab=download", {
      hfRecommendation: (request) => ok(nativeRecommendation(request)),
    });
    await screen.findByText("All on the GPU");
    expect(screen.queryByText(/runs well too/)).toBeNull();
  });

  it("shows when the repository was last updated", async () => {
    const modified = new Date(Date.now() - 3 * 24 * 3600 * 1000).toISOString();
    await mount("/settings/model-browser?repo=owner%2Fmodel&tab=info", {
      hfModelFiles: ({ model_id }) => ok({ ...logicalSplitInfo(), model_id, last_modified: modified }),
    });
    expect(await screen.findByText(/^Updated /)).toBeTruthy();
  });

  it("names the computer's GPU in the destination menu, with a count for several", async () => {
    const device = (index: number, name: string) => ({
      index,
      name,
      description: "",
      backend: "CUDA",
      device_type: "Gpu",
      memory_free: 8,
      memory_total: 8,
    });
    await mount("/settings/model-browser", {
      llamaDevices: () => ok({ devices: [device(0, "RTX 4070")] }),
    });
    fireEvent.click(await screen.findByRole("button", { name: /This computer/ }));
    expect(await screen.findByRole("menuitem", { name: "This computer · RTX 4070" })).toBeTruthy();
    cleanup();
    await mount("/settings/model-browser", {
      llamaDevices: () => ok({ devices: [device(0, "RTX 4070"), device(1, "RTX 3060")] }),
    });
    fireEvent.click(await screen.findByRole("button", { name: /This computer/ }));
    expect(
      await screen.findByRole("menuitem", { name: "This computer · RTX 4070 +1" }),
    ).toBeTruthy();
  });

  it("shows downloads, likes and the update date on image repository cards without a file count", async () => {
    const profile: ImageBundleProfile = {
      id: "real-profile",
      display_name: "Real image model",
      family: "FLUX",
      description: "From the backend",
      required_roles: ["diffusion_model"],
      minimum_runtime_build: null,
      diffusion_markers: [],
      encoder_markers: [],
      encoder_parameter_billions: null,
      recommended_repositories: [],
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
    await mount("/settings/model-browser?kind=image", {
      hfImageBundleProfiles: () => ok({ profiles: [profile] }),
      hfImageBundleSearch: () =>
        ok({
          models: [
            {
              model_id: "owner/component",
              author: "owner",
              likes: 1250,
              downloads: 48200,
              tags: [],
              pipeline_tag: null,
              last_modified: new Date(Date.now() - 14 * 24 * 3600 * 1000).toISOString(),
              trending_score: null,
            },
          ],
        }),
    });
    fireEvent.click(await screen.findByRole("button", { name: /^Real image model/ }));
    const card = await screen.findByRole("button", { name: /owner\/component/ });
    expect(within(card).getByText("48K")).toBeTruthy();
    expect(within(card).getByText("1.3K")).toBeTruthy();
    expect(card.textContent).toMatch(/ago|week|day/);
    expect(card.textContent).not.toMatch(/files? fit/);
  });
});
