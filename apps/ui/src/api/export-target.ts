import type { ApiClient } from "./client";
import type { FileSavePicked, FilesPickSaveRequest } from "./generated/bindings";

// Android publishes to Downloads with a null target; a null result means cancellation.
export async function pickExportTarget(api: ApiClient, request: FilesPickSaveRequest): Promise<FileSavePicked | null> {
  const status = await api.call("appStatus");
  if (status.platform === "android") return { target: null };
  const picked = await api.call("filesPickSave", request);
  return picked.target ? picked : null;
}
