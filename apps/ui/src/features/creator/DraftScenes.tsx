import { type ReactNode, useLayoutEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { Plus, Trash2 } from "lucide-react";
import { Button, CardButton, Chip, confirm, EmptyState, Field, icon, IconButton, Sheet, Textarea } from "@lettuceai/crisp";
import { useClosingValue } from "@/shared/hooks/useClosingValue";
import type { CreatorScene } from "@/api/generated/bindings";

const placeholders = ["{{char}}", "{{user}}"] as const;

const wordCount = (text: string) => {
  const trimmed = text.trim();
  return trimmed ? trimmed.split(/\s+/).length : 0;
};

function SceneEditor({ scene, disabled, onChange }: {
  scene: CreatorScene;
  disabled: boolean;
  onChange: (patch: Partial<CreatorScene>) => void;
}) {
  const { t, i18n } = useTranslation("characters");
  const editor = useRef<HTMLTextAreaElement>(null);
  const caret = useRef<number | null>(null);
  const words = wordCount(scene.content);

  useLayoutEffect(() => {
    if (caret.current === null) return;
    editor.current?.focus({ preventScroll: true });
    editor.current?.setSelectionRange(caret.current, caret.current);
    caret.current = null;
  }, [scene.content]);

  const insert = (placeholder: string) => {
    const start = editor.current?.selectionStart ?? scene.content.length;
    const end = editor.current?.selectionEnd ?? start;
    caret.current = start + placeholder.length;
    onChange({ content: scene.content.slice(0, start) + placeholder + scene.content.slice(end) });
  };

  return (
    <div className="min-w-0 space-y-5 pb-1">
      <div className="space-y-3">
        <Field label={t("scenes.text")}>
          <Textarea
            ref={editor}
            value={scene.content}
            disabled={disabled}
            rows={10}
            autoGrow
            maxRows={24}
            placeholder={t("scenes.textPlaceholder", { interpolation: { skipOnVariables: true }, char: "{{char}}" })}
            onChange={(event) => onChange({ content: event.target.value })}
          />
        </Field>
        <div className="flex flex-wrap items-center gap-2">
          {placeholders.map((placeholder) => (
            <Chip key={placeholder} disabled={disabled} onMouseDown={(event) => event.preventDefault()} onClick={() => insert(placeholder)}>
              <span className="font-mono text-xs">{placeholder}</span>
            </Chip>
          ))}
          <span className="ml-auto text-sm tabular-nums text-fg-3">
            {t("details.words", { count: words, formattedCount: new Intl.NumberFormat(i18n.language).format(words) })}
          </span>
        </div>
      </div>
      <Field label={t("scenes.directionLabel")} hint={t("scenes.directionHint")}>
        <Textarea
          value={scene.direction ?? ""}
          disabled={disabled}
          rows={3}
          autoGrow
          maxRows={10}
          placeholder={t("scenes.directionPlaceholder")}
          onChange={(event) => onChange({ direction: event.target.value || null })}
        />
      </Field>
    </div>
  );
}

/**
 * The starting scenes of a character draft: scene cards in the draft column, like the character editor's list, and
 * the selected scene's editor in a side drawer.
 */
export function DraftScenes({ scenes, selectedId, disabled, footer, onSelect, onChange }: {
  scenes: CreatorScene[];
  footer?: ReactNode;
  selectedId: string | null;
  disabled: boolean;
  onSelect: (id: string | null) => void;
  onChange: (scenes: CreatorScene[]) => void;
}) {
  const { t } = useTranslation("characters");
  const { t: creator } = useTranslation("creator");
  const shownId = useClosingValue(selectedId).shown;
  const index = scenes.findIndex((scene) => scene.id === shownId);
  const shown = index >= 0 ? { scene: scenes[index]!, number: index + 1 } : null;

  const add = () => {
    const id = crypto.randomUUID();
    onChange([...scenes, { id, content: "", direction: null }]);
    onSelect(id);
  };
  const remove = async () => {
    if (!shown) return;
    const confirmed = await confirm({
      title: t("scenes.deleteTitle"),
      message: t("scenes.deleteBody", { scene: t("scenes.number", { number: shown.number }) }),
      confirmLabel: t("scenes.delete"),
      cancelLabel: t("scenes.cancel"),
      destructive: true,
    });
    if (!confirmed) return;
    onSelect(null);
    onChange(scenes.filter((entry) => entry.id !== shown.scene.id));
  };

  return (
    <section className="space-y-3">
      <p className="text-sm font-medium text-fg">{creator("scenes")}</p>
      {scenes.length === 0 ? (
        <EmptyState
          size="sm"
          title={t("scenes.empty")}
          description={t("scenes.emptyBodyEdit")}
          action={<Button variant="tonal" leading={<Plus size={icon.md} />} disabled={disabled} onClick={add}>{t("scenes.addFirst")}</Button>}
        />
      ) : (
        <div className="space-y-2">
          {scenes.map((entry, position) => {
            const content = entry.content.trim();
            return (
              <CardButton key={entry.id} selected={entry.id === selectedId} aria-label={t("scenes.number", { number: position + 1 })} onClick={() => onSelect(entry.id)}>
                <span className="block truncate text-sm font-medium text-fg">{t("scenes.number", { number: position + 1 })}</span>
                <p className={`mt-1.5 line-clamp-2 whitespace-pre-line text-sm leading-relaxed ${content ? "text-fg-2" : "text-fg-4"}`}>{content || t("scenes.untitled")}</p>
              </CardButton>
            );
          })}
          <Button variant="ghost" block className="justify-start" leading={<Plus size={icon.sm} />} disabled={disabled} onClick={add}>
            {t("scenes.add")}
          </Button>
        </div>
      )}
      <Sheet
        open={selectedId !== null && shown !== null}
        onClose={() => onSelect(null)}
        placement="side"
        size="lg"
        title={shown ? t("scenes.number", { number: shown.number }) : ""}
        {...(footer ? { footer } : {})}
        rightAction={
          <IconButton label={t("scenes.delete")} size="sm" variant="ghost" disabled={disabled} onClick={() => void remove()}>
            <Trash2 size={icon.md} />
          </IconButton>
        }
      >
        {shown && (
          <SceneEditor
            key={shown.scene.id}
            scene={shown.scene}
            disabled={disabled}
            onChange={(patch) => onChange(scenes.map((entry) => (entry.id === shown.scene.id ? { ...entry, ...patch } : entry)))}
          />
        )}
      </Sheet>
    </section>
  );
}
