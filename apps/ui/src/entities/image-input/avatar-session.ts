import { ApiFailure, type ApiClient } from "@/api/client";
import type { AssetRef, AvatarGenerateRequest, AvatarGenerationAccepted, AvatarGenerationOptions, JobView } from "@/api/generated/bindings";
import { watchJob } from "@/entities/job";
import { pickImage } from "./files";

export interface AvatarContext {
  subjectName: string;
  subjectDescription: string;
  currentAvatar?: AssetRef | null;
  characterId?: string | null;
}

export interface AvatarVariant {
  id: string;
  asset: AssetRef;
  origin: "current" | "upload" | "generation" | "edit";
  prompt: string;
  request: string;
  parentId: string | null;
}

export interface AvatarSessionState {
  options: AvatarGenerationOptions | null;
  loadingOptions: boolean;
  phase: "idle" | "uploading" | "generating" | "editing" | "cancelling";
  variants: readonly AvatarVariant[];
  selectedId: string | null;
  job: JobView | null;
  error: ApiFailure | null;
  errorSource: "options" | "upload" | "generation" | "cancellation" | null;
  pendingRequest: boolean;
}

interface Attempt {
  request: AvatarGenerateRequest;
  parentId: string | null;
  accepted?: AvatarGenerationAccepted | undefined;
  terminal: boolean;
  cancelled?: boolean;
}

interface Run { cancelled: boolean; jobId?: string; abort: AbortController }

export class AvatarSession {
  private state: AvatarSessionState;
  private readonly listeners = new Set<() => void>();
  private readonly context: AvatarContext;
  private readonly api: ApiClient;
  private closed = false;
  private run: Run | null = null;
  private attempt: Attempt | null = null;

  constructor(api: ApiClient, context: AvatarContext) {
    this.api = api;
    this.context = { ...context };
    const current: AvatarVariant | null = context.currentAvatar ? { id: crypto.randomUUID(), asset: context.currentAvatar, origin: "current", prompt: "", request: "", parentId: null } : null;
    this.state = { options: null, loadingOptions: false, phase: "idle", variants: current ? [current] : [], selectedId: current?.id ?? null, job: null, error: null, errorSource: null, pendingRequest: false };
  }

  getSnapshot = (): AvatarSessionState => this.state;
  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => { this.listeners.delete(listener); };
  };

  private update(patch: Partial<AvatarSessionState>): void {
    if (this.closed) return;
    this.state = { ...this.state, ...patch };
    this.listeners.forEach((listener) => listener());
  }

  async loadOptions(): Promise<void> {
    if (this.closed || this.state.loadingOptions) return;
    this.update({ loadingOptions: true, error: null, errorSource: null });
    try {
      const options = await this.api.call("avatarGenerationOptions");
      this.update({ options });
    } catch (error) {
      this.update({ options: null, error: ApiFailure.fromRejection(error), errorSource: "options" });
    } finally {
      this.update({ loadingOptions: false });
    }
  }

  select(id: string): void {
    if (this.state.phase === "idle" && this.state.variants.some((variant) => variant.id === id)) this.update({ selectedId: id });
  }

  selectedAsset(): AssetRef | null {
    if (this.closed || this.state.phase !== "idle" || this.state.pendingRequest) return null;
    return this.state.variants.find((variant) => variant.id === this.state.selectedId)?.asset ?? null;
  }

  async upload(): Promise<void> {
    if (this.closed || this.state.phase !== "idle" || this.state.pendingRequest) return;
    this.update({ phase: "uploading", error: null, errorSource: null });
    try {
      const asset = await pickImage(this.api, "avatar");
      if (asset) this.add({ id: crypto.randomUUID(), asset, origin: "upload", prompt: "", request: "", parentId: null });
    } catch (error) {
      this.update({ error: ApiFailure.fromRejection(error), errorSource: "upload" });
    } finally {
      this.update({ phase: "idle" });
    }
  }

  private add(variant: AvatarVariant): void {
    this.update({ variants: [...this.state.variants, variant], selectedId: variant.id });
  }

  async generate(prompt: string): Promise<void> {
    await this.start(prompt, false);
  }

  async edit(instruction: string): Promise<void> {
    await this.start(instruction, true);
  }

  private async start(text: string, editing: boolean): Promise<void> {
    if (this.closed || this.state.phase !== "idle" || this.state.pendingRequest) return;
    this.attempt = null;
    const options = this.state.options;
    const parent = this.state.variants.find((variant) => variant.id === this.state.selectedId);
    if (!text.trim() || (editing && !parent)) {
      this.update({ error: new ApiFailure("invalid_input", "an avatar request and edit source are required", null), errorSource: "generation" });
      return;
    }
    if (!options?.enabled || !options.model || !options.model.supports_editing) {
      this.update({ error: new ApiFailure(options?.enabled === false ? "unsupported" : "model_required", "an image input and output model is required", null), errorSource: "options" });
      return;
    }
    const subject = { subject_name: this.context.subjectName, subject_description: this.context.subjectDescription };
    const request: AvatarGenerateRequest = {
      request_id: crypto.randomUUID(), model_id: options.model.id,
      character_id: this.context.characterId ?? null,
      source_asset_id: editing ? parent!.asset.asset_id : null,
      kind: editing ? { type: "edit", ...subject, current_avatar_prompt: parent!.prompt, edit_request: text.trim() }
        : { type: "generation", ...subject, avatar_request: text.trim() },
    };
    await this.execute({ request, parentId: editing ? parent!.id : null, terminal: false });
  }

  async retry(): Promise<void> {
    if (this.closed || this.state.phase !== "idle" || this.state.errorSource !== "generation" || !this.attempt) return;
    const attempt = this.attempt;
    await this.execute(attempt.terminal ? { ...attempt, accepted: undefined, terminal: false, cancelled: false, request: { ...attempt.request, request_id: crypto.randomUUID() } } : attempt);
  }

  private async execute(attempt: Attempt): Promise<void> {
    const run: Run = { cancelled: attempt.cancelled ?? false, abort: new AbortController() };
    this.run = run;
    this.attempt = attempt;
    const editing = attempt.request.kind.type === "edit";
    this.update({ phase: run.cancelled ? "cancelling" : editing ? "editing" : "generating", error: null, errorSource: null, job: null, pendingRequest: true });
    try {
      const accepted = attempt.accepted ?? await this.api.call("avatarGenerate", attempt.request);
      attempt.accepted = accepted;
      run.jobId = accepted.job_id;
      if (run.cancelled || this.closed) {
        try {
          await this.cancelJob(run);
        } catch (error) {
          this.update({ error: ApiFailure.fromRejection(error), errorSource: "cancellation" });
        }
        if (this.closed) return;
      }
      const job = await watchJob(this.api, accepted.job_id, (job) => this.update({ job }), run.abort.signal);
      attempt.terminal = true;
      this.update({ pendingRequest: false });
      if (run.cancelled || this.closed || job.state === "cancelled") {
        this.update({ error: null, errorSource: null });
        return;
      }
      if (job.state !== "succeeded" || job.result?.type !== "image_generation" || job.result.images.length === 0) {
        throw new ApiFailure("unavailable", "avatar generation did not produce an image", job.failure?.image ? { type: "image", failure: job.failure.image.kind } : null);
      }
      for (const image of job.result.images) {
        this.add({ id: crypto.randomUUID(), asset: image.asset, origin: editing ? "edit" : "generation", prompt: accepted.prompt,
          request: attempt.request.kind.type === "edit" ? attempt.request.kind.edit_request : attempt.request.kind.avatar_request, parentId: attempt.parentId });
      }
    } catch (error) {
      const failure = ApiFailure.fromRejection(error);
      if (!attempt.accepted && failure.code !== "transport" && failure.code !== "internal" && failure.code !== "unavailable") {
        this.update({ pendingRequest: false });
      }
      if (!this.closed) this.update({ error: failure, errorSource: "generation" });
    } finally {
      if (this.run === run) this.run = null;
      this.update({ phase: "idle" });
    }
  }

  private async cancelJob(run: Run): Promise<void> {
    if (!run.jobId) return;
    await this.api.call("jobCancel", { job_id: run.jobId });
  }

  async cancel(): Promise<void> {
    const run = this.run;
    if (this.closed) return;
    if (!run) {
      if (this.state.pendingRequest && this.attempt) await this.execute({ ...this.attempt, cancelled: true });
      return;
    }
    run.cancelled = true;
    if (this.attempt) this.attempt.cancelled = true;
    this.update({ phase: "cancelling", error: null, errorSource: null });
    try {
      await this.cancelJob(run);
    } catch (error) {
      this.update({ error: ApiFailure.fromRejection(error), errorSource: "cancellation" });
    }
  }

  dispose(): void {
    if (this.closed) return;
    this.closed = true;
    this.listeners.clear();
    const run = this.run;
    if (run) {
      run.cancelled = true;
      run.abort.abort();
      void this.cancelJob(run).catch(() => undefined);
    } else if (this.attempt?.accepted && !this.attempt.terminal) {
      void this.api.call("jobCancel", { job_id: this.attempt.accepted.job_id }).catch(() => undefined);
    }
  }
}
