import { useLayoutEffect, useRef, type ReactNode } from "react";
import { Plus, Star, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Alert, Button, CardButton, Chip, confirm, EmptyState, Field, icon, IconButton, Textarea } from "@lettuceai/crisp";
import { hasSceneContent, sceneText, withAddedScene, withDefaultScene, withEditedScene, withoutScene, wordCount, type SceneDraft } from "../data/form";
import type { StepProps } from "./step-props";

interface ScenesStepProps extends StepProps {
  phone: boolean;
  editorOpen: boolean;
  selectedId: string | null;
  onSelect: (id: string | null) => void;
}

function SceneEditor({ form, update, scene, phone, onRemove, warning }: StepProps & { scene: SceneDraft; phone: boolean; onRemove: () => void; warning?: ReactNode }) {
  const { t, i18n } = useTranslation("characters");
  const editor = useRef<HTMLTextAreaElement>(null);
  const caret = useRef<number | null>(null);
  const text = sceneText(scene);
  const direction = scene.direction ?? "";
  const isDefault = scene.id === form.draft.defaults.default_scene_id;
  const number = new Intl.NumberFormat(i18n.language);
  const change = (nextText: string, nextDirection = direction) => update((current) => withEditedScene(current, scene.id, { text: nextText, direction: nextDirection }));

  useLayoutEffect(() => {
    editor.current?.focus({ preventScroll: true });
  }, [scene.id]);

  useLayoutEffect(() => {
    if (caret.current === null) return;
    editor.current?.focus({ preventScroll: true });
    editor.current?.setSelectionRange(caret.current, caret.current);
    caret.current = null;
  }, [text]);

  const insert = (placeholder: string) => {
    const start = editor.current?.selectionStart ?? text.length;
    const end = editor.current?.selectionEnd ?? start;
    caret.current = start + placeholder.length;
    change(text.slice(0, start) + placeholder + text.slice(end));
  };

  return <div className="min-w-0 space-y-5">
    <div className="flex min-h-11 items-center gap-2">
      {!phone && <h2 className="min-w-0 flex-1 truncate text-lg font-semibold text-fg">{t("scenes.number", { number: scene.ordinal + 1 })}</h2>}
      <Button size="sm" variant="ghost" className={phone ? "ml-auto" : undefined} aria-pressed={isDefault} leading={<Star size={icon.sm} fill={isDefault ? "currentColor" : "none"} />} onClick={() => update((current) => withDefaultScene(current, isDefault ? null : scene.id))}>{t(isDefault ? "scenes.defaultSelected" : "scenes.makeDefault")}</Button>
      <IconButton label={t("scenes.delete")} size="sm" onClick={onRemove}><Trash2 size={icon.md} /></IconButton>
    </div>
    <div className="space-y-3">
      <Field label={t("scenes.text")}>
        <Textarea ref={editor} value={text} rows={10} autoGrow maxRows={24} placeholder={t("scenes.textPlaceholder", { interpolation: { skipOnVariables: true }, char: "{{char}}" })} onChange={(event) => change(event.target.value)} />
      </Field>
      <div className="flex flex-wrap items-center gap-2">
        {["{{char}}", "{{user}}"].map((placeholder) => <Chip key={placeholder} onMouseDown={(event) => event.preventDefault()} onClick={() => insert(placeholder)}><span className="font-mono text-xs">{placeholder}</span></Chip>)}
        <span className="ml-auto text-sm tabular-nums text-fg-3">{t("details.words", { count: wordCount(text), formattedCount: number.format(wordCount(text)) })}</span>
      </div>
    </div>
    {warning}
    <Field label={t("scenes.directionLabel")} hint={t("scenes.directionHint")}>
      <Textarea value={direction} rows={3} autoGrow maxRows={10} placeholder={t("scenes.directionPlaceholder")} onChange={(event) => change(text, event.target.value)} />
    </Field>
  </div>;
}

export function ScenesStep({ form, update, phone, editorOpen, selectedId, onSelect, api }: ScenesStepProps) {
  const { t } = useTranslation("characters");
  const { scenes, defaults } = form.draft;
  const scene = scenes.find((entry) => entry.id === selectedId);
  const emptyCount = scenes.filter((entry) => !hasSceneContent(entry) && (phone || entry.id !== selectedId)).length;
  const emptyWarning = emptyCount > 0 ? <Alert tone="warning" title={t("scenes.emptyWarning", { count: emptyCount })} description={t("scenes.emptyWarningBody")} /> : null;
  const add = () => {
    const id = crypto.randomUUID();
    update((current) => withAddedScene(current, { text: "", direction: "" }, id));
    onSelect(id);
  };
  const remove = async () => {
    if (!scene || !await confirm({
      title: t("scenes.deleteTitle"),
      message: t("scenes.deleteBody", { scene: t("scenes.number", { number: scene.ordinal + 1 }) }),
      confirmLabel: t("scenes.delete"),
      cancelLabel: t("scenes.cancel"),
      destructive: true,
    })) return;
    const index = scenes.indexOf(scene);
    const next = scenes[index + 1] ?? scenes[index - 1];
    update((current) => withoutScene(current, scene.id));
    onSelect(phone ? null : next?.id ?? null);
  };
  const showList = !phone || !editorOpen;
  const showEditor = !phone || editorOpen;

  if (scenes.length === 0) return <div className="space-y-6">
    <p className="max-w-3xl text-base text-fg-2">{t("scenes.body")}</p>
    <EmptyState title={t("scenes.empty")} description={t("scenes.emptyBody")} action={<Button variant="tonal" leading={<Plus size={icon.md} />} onClick={add}>{t("scenes.addFirst")}</Button>} />
  </div>;

  return <div className="space-y-6">
    {showList && <p className="max-w-3xl text-base text-fg-2">{t("scenes.body")}</p>}
    <div className={phone ? "min-w-0" : "grid items-start gap-6 @3xl:grid-cols-3 @3xl:gap-8"}>
      {showList && <div className="min-w-0 space-y-3">
        <div className="space-y-2">
          {scenes.map((entry, index) => {
            const content = sceneText(entry).trim();
            const selected = entry.id === selectedId;
            return <CardButton key={entry.id} selected={selected} aria-label={t("scenes.number", { number: index + 1 })} onClick={() => onSelect(entry.id)}>
              <div className="flex items-center justify-between gap-2">
                <span className="min-w-0 truncate text-sm font-medium text-fg">{t("scenes.number", { number: index + 1 })}</span>
                {entry.id === defaults.default_scene_id && <span className="flex shrink-0 items-center gap-1 text-xs text-fg-3"><Star size={icon.xs} fill="currentColor" />{t("scenes.default")}</span>}
              </div>
              <p className={`mt-1.5 line-clamp-2 whitespace-pre-line text-sm leading-relaxed ${content ? "text-fg-2" : "text-fg-4"}`}>{content || t("scenes.untitled")}</p>
            </CardButton>;
          })}
          <Button variant="ghost" block className="justify-start" leading={<Plus size={icon.sm} />} onClick={add}>{t("scenes.add")}</Button>
        </div>
        {emptyWarning}
      </div>}
      {showEditor && <div className="min-w-0 @3xl:col-span-2">
        {scene ? <SceneEditor key={scene.id} api={api} form={form} update={update} scene={scene} phone={phone} onRemove={() => void remove()} warning={phone ? emptyWarning : undefined} /> : scenes.length > 0 && <EmptyState size="sm" title={t("scenes.select")} description={t("scenes.selectBody")} />}
      </div>}
    </div>
  </div>;
}
