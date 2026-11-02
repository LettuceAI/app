import type { ImageFailureKind, JobFailureCode } from "@/api/generated/bindings";
import { isApiFailure } from "@/api/client";
import type { AvatarSessionState } from "./avatar-session";

export type AvatarFailureCopy =
  | "cancelled" | "setup" | "invalidRequest" | "engine" | "timedOut" | "memory" | "storage" | "provider"
  | "empty" | "rejected" | "refused" | "authentication" | "unavailable" | "unknown";

const imageCopy: Record<ImageFailureKind, AvatarFailureCopy> = {
  cancelled: "cancelled",
  local_unsupported: "setup",
  outdated_registration: "setup",
  model_file_missing: "setup",
  model_not_configured: "setup",
  runtime_not_installed: "setup",
  runtime_incompatible: "setup",
  upscaler_missing: "setup",
  model_missing: "setup",
  lora_conflict: "setup",
  lora_in_use: "setup",
  lora_invalid: "setup",
  invalid_request: "invalidRequest",
  server_start_failed: "engine",
  server_not_ready: "engine",
  engine_rejected: "engine",
  engine_failed: "engine",
  interrupted: "engine",
  engine_timed_out: "timedOut",
  out_of_memory: "memory",
  storage_failed: "storage",
  provider_failed: "provider",
  no_image_returned: "empty",
  output_rejected: "rejected",
  other: "unknown",
};

const jobCopy: Partial<Record<JobFailureCode, AvatarFailureCopy>> = {
  cancelled: "cancelled",
  safety_refusal: "refused",
  authentication: "authentication",
  capability_unavailable: "setup",
  resource_unavailable: "unavailable",
  timed_out: "timedOut",
  storage_failure: "storage",
  invalid_input: "invalidRequest",
};

const setupCopy: ReadonlySet<AvatarFailureCopy> = new Set(["setup", "authentication", "unavailable"]);

export type AvatarNoticeKind =
  | "options" | "disabled" | "noModel" | "incompatible" | "upload" | "generation" | "cancellation" | "uncertain" | "cancelled";

export interface AvatarNotice {
  kind: AvatarNoticeKind;
  tone: "danger" | "warning" | "info";
  copy: AvatarFailureCopy | null;
  code: string | null;
  retry: "options" | "upload" | "generation" | "cancellation" | null;
  cancel: boolean;
  configure: boolean;
}

function generationCopy(state: AvatarSessionState): AvatarFailureCopy {
  const failure = state.job?.failure;
  if (failure?.image) return imageCopy[failure.image.kind];
  if (failure) return jobCopy[failure.code] ?? "unknown";
  if (state.job?.state === "succeeded") return "empty";
  const details = state.error?.details;
  if (details?.type === "image") return imageCopy[details.failure];
  const code = state.error?.code;
  if (details?.type === "invalid_field" && details.field === "model_id") return "setup";
  if (code === "not_found" && !details) return "setup";
  if (code === "model_required" || code === "model_unavailable") return "setup";
  if (code === "cancelled") return "cancelled";
  return "unknown";
}

/** Whether a terminal job failure is worth another attempt, by its typed retryability; failures with no job always are. */
function retryable(state: AvatarSessionState): boolean {
  const failure = state.job?.failure;
  if (state.job?.state === "failed" || state.job?.state === "interrupted") return failure ? failure.retryable : true;
  return true;
}

/** The one thing the panel has to say about the session right now, chosen from typed state and never from message text. */
export function avatarNotice(state: AvatarSessionState): AvatarNotice | null {
  const { error, errorSource, options, phase, pendingRequest, job } = state;
  const uncertain = pendingRequest && phase === "idle";
  if (uncertain && errorSource !== "cancellation") {
    return { kind: "uncertain", tone: "warning", copy: null, code: error?.code ?? null, retry: "generation", cancel: true, configure: false };
  }
  if (error && errorSource === "cancellation") {
    return { kind: "cancellation", tone: "danger", copy: null, code: error.code, retry: "cancellation", cancel: false, configure: false };
  }
  if (error && errorSource === "upload") {
    return { kind: "upload", tone: "danger", copy: null, code: isApiFailure(error) ? error.code : "internal", retry: "upload", cancel: false, configure: false };
  }
  if (error && errorSource === "generation") {
    const copy = generationCopy(state);
    return { kind: "generation", tone: "danger", copy, code: error.code, retry: retryable(state) ? "generation" : null, cancel: false, configure: setupCopy.has(copy) };
  }
  if (error && errorSource === "options" && !options) {
    return { kind: "options", tone: "warning", copy: null, code: error.code, retry: "options", cancel: false, configure: true };
  }
  if (options) {
    if (!options.enabled) return { kind: "disabled", tone: "info", copy: null, code: null, retry: null, cancel: false, configure: true };
    if (!options.model) return { kind: "noModel", tone: "info", copy: null, code: null, retry: null, cancel: false, configure: true };
    if (!options.model.supports_editing) return { kind: "incompatible", tone: "info", copy: null, code: null, retry: null, cancel: false, configure: true };
  }
  if (job?.state === "cancelled" && phase === "idle") {
    return { kind: "cancelled", tone: "info", copy: null, code: null, retry: null, cancel: false, configure: false };
  }
  return null;
}

export function avatarCanGenerate(state: AvatarSessionState): boolean {
  const { options } = state;
  return Boolean(options?.enabled && options.model?.supports_editing);
}

export function avatarBusy(state: AvatarSessionState): boolean {
  return state.phase !== "idle" || state.pendingRequest;
}
