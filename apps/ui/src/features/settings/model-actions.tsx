import { confirm } from "@lettuceai/crisp";
import type { TFunction } from "i18next";
import type { ApiClient } from "@/api/client";
import type { ModelView } from "@/api/generated/bindings";

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
  const picked = await api.call("filesPickSave", { suggested_name: `${model.display_name.replace(/[/\\]/g, "_")}.uec`, kind: "any" });
  if (picked.target) await api.call("modelExport", { id: model.id, target: picked.target });
}
