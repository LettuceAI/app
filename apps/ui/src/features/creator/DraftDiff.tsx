import { useTranslation } from "react-i18next";
import { ArrowRight } from "lucide-react";
import { Avatar, Diff, icon, Text } from "@lettuceai/crisp";
import type { CreatorDraft, CreatorKind } from "@/api/generated/bindings";

function scenesText(draft: CreatorDraft, label: (number: number) => string) {
  return draft.scenes
    .map((scene, index) => [label(index + 1), scene.content, scene.direction ?? ""].filter((line) => line.trim()).join("\n"))
    .join("\n\n");
}

/** What changed between two drafts, field by field; unchanged fields are left out. */
export function DraftDiff({ before, after, kind }: { before: CreatorDraft; after: CreatorDraft; kind: CreatorKind }) {
  const { t } = useTranslation("creator");
  const scene = (number: number) => t("scene", { number });
  const fields = [
    { key: "name", label: t("name"), before: before.name, after: after.name },
    { key: "definition", label: t(kind === "character" ? "definition" : "description"), before: before.definition, after: after.definition },
    ...(kind === "character" ? [{ key: "scenes", label: t("scenes"), before: scenesText(before, scene), after: scenesText(after, scene) }] : []),
  ].filter((field) => field.before !== field.after);
  const avatarChanged = (before.avatar?.asset_id ?? null) !== (after.avatar?.asset_id ?? null);
  if (fields.length === 0 && !avatarChanged) return <Text variant="hint">{t("noChanges")}</Text>;
  return (
    <div className="space-y-6">
      {avatarChanged && (
        <section className="space-y-2">
          <p className="text-sm font-medium text-fg">{t("avatar.label")}</p>
          <div className="flex items-center gap-3">
            <span role="img" aria-label={t("before")}><Avatar src={before.avatar?.url ?? null} name={before.name.trim() || "?"} size="lg" /></span>
            <ArrowRight size={icon.md} className="text-fg-3" aria-hidden />
            <span role="img" aria-label={t("after")}><Avatar src={after.avatar?.url ?? null} name={after.name.trim() || "?"} size="lg" /></span>
          </div>
        </section>
      )}
      {fields.map((field) => (
        <section key={field.key} className="min-w-0 space-y-2">
          <p className="text-sm font-medium text-fg">{field.label}</p>
          <Diff before={field.before} after={field.after} beforeLabel={t("before")} afterLabel={t("after")} context={2} />
        </section>
      ))}
    </div>
  );
}
