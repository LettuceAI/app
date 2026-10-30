import { Check } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button, icon, ListGroup, ListRow, ListRowButton, Segmented } from "@lettuceai/crisp";
import type { SpeakerSelectionMethod } from "@/api/generated/bindings";
import type { PanelCtx } from "./context";
import { ControlRow } from "./SettingRow";

const choices = ["llm", "heuristic", "round_robin", "director"] as const;

export function SpeakerSelectionGroup({ ctx }: { ctx: PanelCtx }) {
  const { t } = useTranslation("conversations");
  const current = ctx.settings?.speaker_selection;
  if (!current) return null;
  const director = current.method === "director" || current.method === "director_action";
  const choose = (method: SpeakerSelectionMethod) => void ctx.save({ speaker_selection: { type: "set", method } });
  return (
    <ListGroup label={t("settings.sections.speaker")}>
      {choices.map((choice) => {
        const selected = choice === "director" ? director : current.method === choice;
        return (
          <ListRowButton
            key={choice}
            title={t(`settings.speaker.${choice}`)}
            description={t(`settings.speaker.${choice}Hint`)}
            chevron={false}
            disabled={ctx.busy}
            aria-pressed={selected}
            trailing={selected ? <Check size={icon.md} className="text-accent" /> : undefined}
            onClick={() => choose(choice === "director" && director ? current.method : choice)}
          />
        );
      })}
      {director && (
        <ControlRow
          title={t("settings.speaker.style")}
          description={t(current.method === "director_action" ? "settings.speaker.actionHint" : "settings.speaker.cueHint")}
          control={
            <Segmented<SpeakerSelectionMethod>
              size="sm"
              aria-label={t("settings.speaker.style")}
              value={current.method}
              options={[
                { value: "director", label: t("settings.speaker.cue") },
                { value: "director_action", label: t("settings.speaker.action") },
              ]}
              onChange={choose}
            />
          }
        />
      )}
      {current.source === "conversation" && (
        <ListRow
          title={t("settings.speaker.own")}
          trailing={
            <Button variant="ghost" size="sm" disabled={ctx.busy} onClick={() => void ctx.save({ speaker_selection: { type: "reset" } })}>
              {t("settings.reset")}
            </Button>
          }
        />
      )}
    </ListGroup>
  );
}
