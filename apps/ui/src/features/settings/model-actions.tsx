import { confirm } from "@lettuceai/crisp";
import type { TFunction } from "i18next";
import { ApiFailure, isApiFailure, type ApiClient, type ApiFailureCode } from "@/api/client";
import { pickExportTarget } from "@/api/export-target";
import type { FileSource, ModelView } from "@/api/generated/bindings";

export async function deleteModelWithUsage(api: ApiClient, model: ModelView, t: TFunction<"settings">): Promise<boolean> {
  const usage = await api.call("modelUsage", { model_id: model.id });
  const sure = await confirm({
    title: t("models.deleteTitle", { name: model.display_name }),
    message: <div className="space-y-3">
      <p>{t("models.deleteWarning")}</p>
      {usage.characters.length > 0 && <section><h3 className="font-medium">{t("modelEditor.characters")}</h3><ul>{usage.characters.map((entry) => <li key={entry.id}>{entry.name}</li>)}</ul></section>}
      {usage.groups.length > 0 && <section><h3 className="font-medium">{t("modelEditor.usageGroups")}</h3><ul>{usage.groups.map((entry) => <li key={entry.id}>{entry.name}</li>)}</ul></section>}
      {usage.roles.length > 0 && <section><h3 className="font-medium">{t("modelEditor.rolesTitle")}</h3><ul>{usage.roles.map((role) => <li key={role}>{t(`modelEditor.roles.${role}`)}</li>)}</ul></section>}
      <p>{t(usage.characters.length + usage.groups.length + usage.roles.length ? "modelEditor.deleteConsequences" : "modelEditor.usageNone")}</p>
    </div>,
    confirmLabel: t("manage.confirmDelete"), cancelLabel: t("manage.cancel"), destructive: true,
  });
  if (!sure) return false;
  await api.call("modelDelete", { model_id: model.id, expected_revision: model.revision, client_operation_id: crypto.randomUUID() });
  return true;
}

export async function exportModel(api: ApiClient, model: ModelView) {
  const picked = await pickExportTarget(api, { suggested_name: `${model.display_name.replace(/[/\\]/g, "_")}.uec`, kind: "any" });
  if (picked) await api.call("modelExport", { id: model.id, target: picked.target });
}

export interface ModelImportOutcome {
  imported: { id: string; name: string; needsFile: boolean; notes: number }[];
  failed: { name: string; reason: ModelImportFailure }[];
}

export type ModelImportFailure = { kind: "no_provider" } | { kind: "not_model" } | { kind: "error"; code: ApiFailureCode };

const notModelCodes: ReadonlySet<string> = new Set(["unsupported", "malformed", "invalid_input"]);

/** Lets the user pick model files and imports each one in turn, so one bad file never blocks the rest. */
export async function importModels(api: ApiClient): Promise<ModelImportOutcome | null> {
  const picked = await api.call("filesPickOpen", { kinds: ["json"], multiple: true });
  if (picked.sources.length === 0) return null;
  const outcome: ModelImportOutcome = { imported: [], failed: [] };
  for (const source of picked.sources) {
    try {
      const preview = await api.call("importPreview", { source });
      if (preview.content.kind !== "model") throw new ApiFailure("unsupported", "not a model file", null);
      const result = await api.call("importCommit", { source, digest: preview.digest, options: {}, client_operation_id: crypto.randomUUID() });
      if (result.kind !== "imported") throw new ApiFailure("unsupported", "model import did not return an entity", null);
      outcome.imported.push({ id: result.id, name: preview.content.display_name || preview.name, needsFile: preview.content.model_file_required, notes: result.warnings.length });
    } catch (error) {
      outcome.failed.push({ name: sourceName(source), reason: importFailure(error) });
    }
  }
  return outcome;
}

function importFailure(error: unknown): ModelImportFailure {
  if (!isApiFailure(error)) return { kind: "error", code: "internal" };
  if (error.code === "not_found" && error.details?.type === "invalid_field" && error.details.field === "provider_id") return { kind: "no_provider" };
  if (notModelCodes.has(error.code)) return { kind: "not_model" };
  return { kind: "error", code: error.code };
}

function sourceName(source: FileSource): string {
  const uri = decoded(source.uri);
  return uri.split(/[\\/:]/).pop() || uri;
}

function decoded(uri: string): string {
  try {
    return decodeURIComponent(uri);
  } catch {
    return uri;
  }
}
