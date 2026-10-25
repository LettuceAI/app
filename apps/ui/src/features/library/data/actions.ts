import type { QueryClient } from "@tanstack/react-query";
import { queryKeys } from "@/api/query-keys";
import { ApiFailure, type ApiClient } from "@/api/client";
import type {
  EntityExportResult,
  FileSource,
  ImportCommitResult,
  MediaLibraryItem,
} from "@/api/generated/bindings";
import { newPersonaDraft } from "./defaults";
import type { Entry, EntryKind, EntryView } from "./queries";

export async function saveEntry(
  api: ApiClient,
  loaded: Exclude<EntryView, { kind: "character" }>,
  name: string,
  description: string,
  operationId: string,
) {
  if (loaded.kind === "persona") {
    const { view } = loaded;
    return api.call("personaSave", {
      persona_id: view.id,
      expected_revision: view.revision,
      client_operation_id: operationId,
      draft: { ...view.draft, title: name, description },
    });
  }
  const { lorebook } = loaded.view;
  return api.call("lorebookUpdateMetadata", {
    lorebook_id: lorebook.id,
    expected_revision: lorebook.revision,
    client_operation_id: operationId,
    metadata: {
      name,
      detection: lorebook.detection,
      icon_asset_id: lorebook.icon?.asset_id ?? null,
    },
  });
}
export async function createEntry(
  api: ApiClient,
  kind: Exclude<EntryKind, "character">,
  name: string,
  description: string,
  operationId: string,
): Promise<string> {
  if (kind === "persona")
    return (
      await api.call("personaSave", {
        persona_id: null,
        expected_revision: null,
        client_operation_id: operationId,
        draft: newPersonaDraft(name, description),
      })
    ).id;
  return (
    await api.call("lorebookCreate", {
      client_operation_id: operationId,
      metadata: { name, detection: "recent_messages", icon_asset_id: null },
      entries: [],
    })
  ).lorebook.id;
}
export async function deleteEntry(api: ApiClient, loaded: EntryView, operationId: string) {
  if (loaded.kind === "character")
    return api.call("characterDelete", {
      character_id: loaded.view.id,
      expected_revision: loaded.view.revision,
      client_operation_id: operationId,
    });
  if (loaded.kind === "persona")
    return api.call("personaDelete", {
      persona_id: loaded.view.id,
      expected_revision: loaded.view.revision,
      client_operation_id: operationId,
    });
  return api.call("lorebookDelete", {
    lorebook_id: loaded.view.lorebook.id,
    expected_revision: loaded.view.lorebook.revision,
    client_operation_id: operationId,
  });
}
export async function exportEntry(
  api: ApiClient,
  entry: Entry,
): Promise<EntityExportResult | null> {
  const picked = await api.call("filesPickSave", {
    suggested_name: `${entry.name.replace(/[^\p{L}\p{N}_-]+/gu, "_")}.${entry.kind === "persona" ? "uec" : "json"}`,
    kind: entry.kind === "character" ? "character_card" : entry.kind === "persona" ? "any" : "json",
  });
  if (!picked.target) return null;
  if (entry.kind === "character")
    return api.call("characterExport", { id: entry.id, format: "v3", target: picked.target });
  if (entry.kind === "persona")
    return api.call("personaExport", { id: entry.id, target: picked.target });
  return api.call("lorebookExport", { id: entry.id, format: "world_info", target: picked.target });
}
export async function importEntry(api: ApiClient): Promise<ImportCommitResult | null> {
  const picked = await api.call("filesPickOpen", {
    kinds: ["character_card", "json"],
    multiple: false,
  });
  const source = picked.sources[0];
  if (!source) return null;
  const preview = await api.call("importPreview", { source });
  if (!["character", "persona", "lorebook"].includes(preview.content.kind))
    throw new ApiFailure("unsupported", "unsupported library import", null);
  return commitImport(api, source, preview.digest, crypto.randomUUID());
}
export function commitImport(
  api: ApiClient,
  source: FileSource,
  digest: string,
  operationId: string,
): Promise<ImportCommitResult> {
  return api.call("importCommit", {
    source,
    digest,
    options: {},
    client_operation_id: operationId,
  });
}
export function mediaName(item: MediaLibraryItem): string {
  return item.source_label?.trim() || item.asset.asset_id;
}
export async function saveMedia(api: ApiClient, item: MediaLibraryItem): Promise<boolean> {
  const extension =
    item.mime_type
      .split("/")[1]
      ?.replace("jpeg", "jpg")
      .replace("mpeg", "mp3")
      .replace(/[^a-z0-9]/g, "") || (item.role === "audio" ? "wav" : "png");
  const picked = await api.call("filesPickSave", {
    suggested_name: `${mediaName(item).replace(/[^\p{L}\p{N}_-]+/gu, "_")}.${extension}`,
    kind: item.role,
  });
  if (!picked.target) return false;
  await api.call("mediaSaveTo", { asset_id: item.asset.asset_id, target: picked.target });
  return true;
}
export async function applyAvatar(
  api: ApiClient,
  item: MediaLibraryItem,
  owner: { kind: "character" | "persona"; id: string },
) {
  if (owner.kind === "character") {
    const view = await api.call("characterGet", { character_id: owner.id });
    await api.call("characterMediaSet", {
      character_id: owner.id,
      expected_revision: view.revision,
      client_operation_id: crypto.randomUUID(),
      slot: "avatar_original",
      ordinal: 0,
      input: { kind: "asset", asset_id: item.asset.asset_id },
    });
  } else {
    const view = await api.call("personaGet", { persona_id: owner.id });
    await api.call("personaMediaSet", {
      persona_id: owner.id,
      expected_revision: view.revision,
      client_operation_id: crypto.randomUUID(),
      slot: "avatar",
      ordinal: 0,
      input: { kind: "asset", asset_id: item.asset.asset_id },
    });
  }
}

export async function refreshLibrary(queryClient: QueryClient) {
  await Promise.all(
    [
      queryKeys.library.all,
      queryKeys.characters.all,
      queryKeys.personas.all,
      queryKeys.lorebooks.all,
      queryKeys.media.all,
      queryKeys.home.all,
      queryKeys.conversations.all,
    ].map((queryKey) => queryClient.invalidateQueries({ queryKey })),
  );
}
