import type { ApiClient } from "@/api/client";
import type { JobKindDto, JobPage, JobView } from "@/api/generated/bindings";

export const downloadKinds: readonly JobKindDto[] = ["artifact_install", "model_pull"];

export async function listActiveDownloads(api: ApiClient): Promise<JobView[]> {
  const jobs: JobView[] = [];
  let cursor: string | null = null;
  do {
    const page: JobPage = await api.call("jobsList", { kinds: ["artifact_install", "model_pull"], states: ["queued", "claimed", "running", "cancellation_requested", "cleaning_up"], subject: null, cursor, limit: 100 });
    jobs.push(...page.items);
    cursor = page.next_cursor;
  } while (cursor);
  return jobs;
}

export function downloadName(job: JobView | null): string | null {
  const detail = job?.subject_detail;
  switch (detail?.type) {
    case "sd_runtime_install":
      return `${detail.release} · ${detail.backend}`;
    case "whisper_model_install":
    case "sd_upscaler_install":
    case "sd_lora_install":
      return detail.filename;
    case "sd_model_install":
    case "image_bundle":
    case "model_download":
      return detail.display_name;
    case "model_pull":
      return detail.model;
    default:
      return null;
  }
}
