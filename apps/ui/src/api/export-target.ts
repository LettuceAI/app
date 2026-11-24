import type { ApiClient } from "./client";
import type { FileSavePicked, FilesPickSaveRequest } from "./generated/bindings";

export async function pickExportTarget(api: ApiClient, request: FilesPickSaveRequest): Promise<FileSavePicked | null> {
  const picked = await api.call("filesPickSave", request);
  return picked.target ? picked : null;
}
