import type { ConversationSettingsView, ModelsView } from "@/api/generated/bindings";

export function attachmentScopes(settings: ConversationSettingsView | undefined, catalog: ModelsView | undefined): string[] {
  if (!settings || !catalog) return [];
  const scopes = (id: string | null) => catalog.models.find((model) => model.id === id)?.input_scopes ?? [];
  if (settings.kind !== "group") return scopes(settings.model.id);
  const members = settings.members?.participants.filter((member) => member.enabled) ?? [];
  if (members.length === 0) return [];
  const models = members.map((member) => scopes(settings.model.id ?? member.model.id ?? catalog.default_model_id));
  return models[0]!.filter((scope) => models.every((model) => model.includes(scope)));
}
