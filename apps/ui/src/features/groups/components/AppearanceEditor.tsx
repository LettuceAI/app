import { useTranslation } from "react-i18next";
import { LoadingState, useTheme } from "@lettuceai/crisp";
import { usePreferences, type ChatLook } from "@/entities/preferences";
import { ChatPreview, ChatSettings } from "@/features/settings";
import { FailureState } from "@/shared/ui/FailureState";
import { appearanceFieldKeys, appearanceFields, appearanceLook, changeAppearance, type AppearanceDraft } from "../data/appearance";

export function AppearanceEditor<D extends AppearanceDraft>({ draft, onChange, phone, kind = "group", preview = true }: { draft: D; onChange: (draft: D) => void; phone: boolean; kind?: "group" | "character"; preview?: boolean }) {
  const { t } = useTranslation("groups");
  const { theme } = useTheme();
  const preferences = usePreferences();
  if (!preferences.ready) return preferences.error ? <FailureState error={preferences.error} onRetry={preferences.retry} /> : <LoadingState />;
  const group = kind === "group";
  const fields = appearanceFields(draft, preferences.value);
  const look = appearanceLook(draft, preferences.value, group);
  const own = (key: keyof ChatLook) => appearanceFieldKeys(key).some((field) => fields.has(field));
  const reset = (keys: string[]) => onChange({ ...draft, presentation_overrides: [...fields].filter((field) => !keys.includes(field)) });
  return <div className="space-y-6">
    <p className="text-sm text-fg-3">{t(group ? "appearance.body" : "appearance.characterBody")}</p>
    {preview && <ChatPreview look={look} group={group} />}
    <ChatSettings group groupView={group} look={look} phone={phone} set={(key, value) => onChange(changeAppearance(draft, preferences.value, key, value, theme.colors.accent))} overridden={own} ownLabel={t(group ? "appearance.own" : "appearance.characterOwn")} resetLabel={(title) => t("appearance.resetField", { title })} resetField={(key) => reset(appearanceFieldKeys(key))} onReset={() => onChange({ ...draft, presentation_overrides: [] })} />
  </div>;
}
