import { useState, type ReactNode } from "react";
import { ImagePlus, Plus, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Avatar, Button, Field, icon, IconButton, Input, ListGroup, NumberInput, Segmented, Sheet, Switch, Textarea } from "@lettuceai/crisp";
import type { LibraryBoxVariant, LibraryButtonAction, LibraryWidgetDesign, LibraryWidgetImageShape } from "@/api/generated/bindings";
import { useClosingValue } from "@/shared/hooks/useClosingValue";
import { EditorRow } from "@/shared/ui/EditorRow";
import { SelectMenuField } from "@/shared/ui/SelectMenuField";
import { useWidgets } from "./context";
import type { WidgetNode } from "./nodes";

type Node<T extends WidgetNode["type"]> = Extract<WidgetNode, { type: T }>;
type Change<N extends WidgetNode> = (node: N) => void;

export interface WidgetConfigSheetProps {
  node: WidgetNode | null;
  onSave: (node: WidgetNode) => void;
  onClose: () => void;
}

export function WidgetConfigSheet({ node, onSave, onClose }: WidgetConfigSheetProps) {
  const { shown, session } = useClosingValue(node);
  return shown && <ConfigSheet key={session} node={shown} open={node !== null} onSave={onSave} onClose={onClose} />;
}

function ConfigSheet({ node, open, onSave, onClose }: { node: WidgetNode; open: boolean; onSave: (node: WidgetNode) => void; onClose: () => void }) {
  const { t } = useTranslation("conversations");
  const [draft, setDraft] = useState(node);
  return (
    <Sheet
      open={open}
      onClose={onClose}
      size="md"
      title={t(`widgets.types.${node.type}.label`)}
      footer={
        <div className="flex items-center justify-end gap-2">
          <Button variant="ghost" onClick={onClose}>{t("widgets.config.cancel")}</Button>
          <Button onClick={() => onSave(draft)}>{t("widgets.config.save")}</Button>
        </div>
      }
    >
      <div className="space-y-5 pb-2">
        <Body node={draft} change={setDraft} />
        {draft.type !== "divider" && draft.type !== "box" && (
          <ListGroup>
            <EditorRow stack label={t("widgets.config.design.label")}>
              <Segmented
                block
                size="sm"
                aria-label={t("widgets.config.design.label")}
                value={draft.design ?? "default"}
                onChange={(design: LibraryWidgetDesign) => setDraft({ ...draft, design })}
                options={(["default", "minimal", "solid", "outline"] as const).map((value) => ({ value, label: t(`widgets.config.design.${value}`) }))}
              />
            </EditorRow>
          </ListGroup>
        )}
      </div>
    </Sheet>
  );
}

function TextRow({ label, hint, value, onChange }: { label: string; hint?: string; value: string | null; onChange: (value: string | null) => void }) {
  return (
    <EditorRow stack label={label} {...(hint ? { hint } : {})}>
      <Input aria-label={label} value={value ?? ""} onChange={(event) => onChange(event.target.value || null)} />
    </EditorRow>
  );
}

/** A number that may be left empty; an entry that is not a number leaves the value as it was. */
function NumberField({ label, value, nullable, onChange }: { label: string; value: number | null; nullable?: boolean; onChange: (value: number | null) => void }) {
  const [text, setText] = useState(value === null ? "" : String(value));
  return (
    <Field label={label}>
      <Input
        inputMode="numeric"
        value={text}
        onChange={(event) => {
          setText(event.target.value);
          const trimmed = event.target.value.trim();
          if (trimmed === "" && nullable) onChange(null);
          else if (trimmed !== "" && Number.isFinite(Number(trimmed))) onChange(Number(trimmed));
        }}
      />
    </Field>
  );
}

function Body({ node, change }: { node: WidgetNode; change: (node: WidgetNode) => void }) {
  const set = <N extends WidgetNode>(next: N) => change(next);
  switch (node.type) {
    case "divider":
      return <DividerForm node={node} set={set} />;
    case "box":
      return <BoxForm node={node} set={set} />;
    case "character_info":
      return <CharacterInfoForm node={node} set={set} />;
    case "persona_info":
      return null;
    case "scratch_pad":
      return <ScratchPadForm node={node} set={set} />;
    case "image":
      return <ImageForm node={node} set={set} />;
    case "selector":
      return <SelectorForm node={node} set={set} />;
    case "button":
      return <ButtonForm node={node} set={set} />;
    case "stat_tracker":
      return <StatTrackerForm node={node} set={set} />;
    case "quick_snippets":
      return <QuickSnippetsForm node={node} set={set} />;
    case "dice":
      return <DiceForm node={node} set={set} />;
    case "memory":
      return <MemoryForm node={node} set={set} />;
    case "companion_state":
    case "session_info":
      return (
        <TitleOnly node={node} set={set} />
      );
    case "author_note":
      return <AuthorNoteForm node={node} set={set} />;
    case "time":
      return <TimeForm node={node} set={set} />;
  }
}

interface FormProps<N extends WidgetNode> {
  node: N;
  set: Change<N>;
}

function TitleOnly({ node, set }: FormProps<Node<"companion_state" | "session_info">>) {
  const { t } = useTranslation("conversations");
  return (
    <ListGroup>
      <TextRow label={t("widgets.config.fields.titleOptional")} value={node.title} onChange={(title) => set({ ...node, title })} />
    </ListGroup>
  );
}

function DividerForm({ node, set }: FormProps<Node<"divider">>) {
  const { t } = useTranslation("conversations");
  return (
    <ListGroup>
      <EditorRow inline label={t("widgets.config.divider.style")}>
        <Segmented
          size="sm"
          aria-label={t("widgets.config.divider.style")}
          value={node.style ?? "line"}
          onChange={(style) => set({ ...node, style })}
          options={[
            { value: "line", label: t("widgets.config.divider.line") },
            { value: "space", label: t("widgets.config.divider.space") },
          ]}
        />
      </EditorRow>
    </ListGroup>
  );
}

function BoxForm({ node, set }: FormProps<Node<"box">>) {
  const { t } = useTranslation("conversations");
  const variants: LibraryBoxVariant[] = ["default", "subtle", "info", "warning", "success", "danger"];
  return (
    <>
      <ListGroup>
        <TextRow label={t("widgets.config.fields.title")} value={node.title} onChange={(title) => set({ ...node, title })} />
        <TextRow label={t("widgets.config.fields.descriptionOptional")} value={node.description} onChange={(description) => set({ ...node, description })} />
        <EditorRow stack label={t("widgets.config.box.variant")}>
          <SelectMenuField
            aria-label={t("widgets.config.box.variant")}
            value={node.variant ?? "default"}
            onChange={(variant) => variant && set({ ...node, variant: variant as LibraryBoxVariant })}
            options={variants.map((value) => ({ value, label: t(`widgets.config.box.${value}`) }))}
          />
        </EditorRow>
      </ListGroup>
    </>
  );
}

function CharacterInfoForm({ node, set }: FormProps<Node<"character_info">>) {
  const { t } = useTranslation("conversations");
  const { members } = useWidgets();
  if (members.length === 0) return null;
  return (
    <ListGroup>
      <EditorRow stack label={t("widgets.config.characterInfo.character")} hint={t("widgets.config.characterInfo.characterHint")}>
        <SelectMenuField
          aria-label={t("widgets.config.characterInfo.character")}
          value={node.character_id ?? ""}
          onChange={(id) => set({ ...node, character_id: id || null })}
          options={[{ value: "", label: t("widgets.config.characterInfo.firstMember") }, ...members.flatMap((member) => (member.id ? [{ value: member.id, label: member.name }] : []))]}
        />
      </EditorRow>
    </ListGroup>
  );
}

function ScratchPadForm({ node, set }: FormProps<Node<"scratch_pad">>) {
  const { t } = useTranslation("conversations");
  return (
    <ListGroup>
      <TextRow label={t("widgets.config.fields.title")} value={node.title} onChange={(title) => set({ ...node, title })} />
      <TextRow label={t("widgets.config.fields.descriptionOptional")} value={node.description} onChange={(description) => set({ ...node, description })} />
      <EditorRow stack label={t("widgets.config.fields.content")} hint={t("widgets.config.scratchPad.markdownHint")}>
        <Textarea rows={6} autoGrow maxRows={14} aria-label={t("widgets.config.fields.content")} value={node.content ?? ""} onChange={(event) => set({ ...node, content: event.target.value })} />
      </EditorRow>
    </ListGroup>
  );
}

function ImageForm({ node, set }: FormProps<Node<"image">>) {
  const { t } = useTranslation("conversations");
  const { assets, pickImage } = useWidgets();
  const [picking, setPicking] = useState(false);
  const kind = node.source.kind === "unresolved_legacy" ? "logical_asset" : node.source.kind;
  const url = node.source.kind === "logical_asset" ? (assets.get(node.source.asset_id) ?? null) : null;
  const pick = async () => {
    setPicking(true);
    const asset = await pickImage();
    setPicking(false);
    if (asset) set({ ...node, source: { kind: "logical_asset", asset_id: asset.asset_id } });
  };
  const shapes: LibraryWidgetImageShape[] = ["auto", "square", "wide", "circle"];
  return (
    <ListGroup>
      <TextRow label={t("widgets.config.fields.titleOptional")} value={node.title} onChange={(title) => set({ ...node, title })} />
      <TextRow label={t("widgets.config.fields.descriptionOptional")} value={node.description} onChange={(description) => set({ ...node, description })} />
      <EditorRow stack label={t("widgets.config.image.source")}>
        <Segmented
          block
          size="sm"
          aria-label={t("widgets.config.image.source")}
          value={kind}
          onChange={(next) => set({ ...node, source: next === "logical_asset" ? { kind: "unresolved_legacy", token: "" } : { kind: next } })}
          options={[
            { value: "character_avatar", label: t("widgets.config.image.character") },
            { value: "persona_avatar", label: t("widgets.config.image.persona") },
            { value: "logical_asset", label: t("widgets.config.image.upload") },
          ]}
        />
      </EditorRow>
      {kind === "logical_asset" && (
        <EditorRow inline label={node.source.kind === "logical_asset" ? t("widgets.config.image.imageSelected") : t("widgets.config.image.noImageSelected")}>
          <div className="flex items-center gap-3">
            {url ? <Avatar src={url} name={node.title ?? t("widgets.image.alt")} size="lg" /> : <ImagePlus size={icon.xl} className="text-fg-3" aria-hidden />}
            <Button variant="secondary" size="sm" loading={picking} onClick={() => void pick()}>
              {node.source.kind === "logical_asset" ? t("widgets.config.image.replaceImage") : t("widgets.config.image.chooseFile")}
            </Button>
          </div>
        </EditorRow>
      )}
      <EditorRow stack label={t("widgets.config.image.shape")}>
        <Segmented
          block
          size="sm"
          aria-label={t("widgets.config.image.shape")}
          value={node.shape ?? "auto"}
          onChange={(shape) => set({ ...node, shape })}
          options={shapes.map((value) => ({ value, label: t(`widgets.config.image.${value}`) }))}
        />
      </EditorRow>
    </ListGroup>
  );
}

function SelectorForm({ node, set }: FormProps<Node<"selector">>) {
  const { t } = useTranslation("conversations");
  return (
    <ListGroup>
      <TextRow label={t("widgets.config.fields.titleOptional")} value={node.title} onChange={(title) => set({ ...node, title })} />
      <TextRow label={t("widgets.config.fields.descriptionOptional")} value={node.description} onChange={(description) => set({ ...node, description })} />
      <EditorRow stack label={t("widgets.config.selector.selects")}>
        <SelectMenuField
          aria-label={t("widgets.config.selector.selects")}
          value={node.kind}
          onChange={(kind) => kind && set({ ...node, kind: kind as Node<"selector">["kind"] })}
          options={[
            { value: "persona", label: t("widgets.config.selector.persona") },
            { value: "model", label: t("widgets.config.selector.model") },
            { value: "author_note", label: t("widgets.config.selector.authorNote") },
          ]}
        />
      </EditorRow>
    </ListGroup>
  );
}

function ButtonForm({ node, set }: FormProps<Node<"button">>) {
  const { t } = useTranslation("conversations");
  const actions: [Exclude<LibraryButtonAction, "toggle_voice_autoplay">, string][] = [
    ["regenerate", t("widgets.config.button.regenerate")],
    ["continue", t("widgets.config.button.continue")],
    ["swap_places", t("widgets.config.button.swapPlaces")],
    ["abort", t("widgets.config.button.stop")],
    ["new_session", t("widgets.config.button.newSession")],
    ["view_history", t("widgets.config.button.history")],
    ["open_memories", t("widgets.config.button.memories")],
    ["open_search", t("widgets.config.button.search")],
  ];
  return (
    <ListGroup>
      <TextRow label={t("widgets.config.fields.titleOptional")} value={node.title} onChange={(title) => set({ ...node, title })} />
      <TextRow label={t("widgets.config.fields.descriptionOptional")} value={node.description} onChange={(description) => set({ ...node, description })} />
      <EditorRow stack label={t("widgets.config.button.action")}>
        <SelectMenuField
          aria-label={t("widgets.config.button.action")}
          value={node.action}
          onChange={(action) => action && set({ ...node, action: action as LibraryButtonAction })}
          options={actions.map(([value, label]) => ({ value, label }))}
        />
      </EditorRow>
    </ListGroup>
  );
}

function StatTrackerForm({ node, set }: FormProps<Node<"stat_tracker">>) {
  const { t } = useTranslation("conversations");
  const update = (id: string, patch: Partial<Node<"stat_tracker">["stats"][number]>) =>
    set({ ...node, stats: node.stats.map((stat) => (stat.id === id ? { ...stat, ...patch } : stat)) });
  return (
    <>
      <ListGroup>
        <TextRow label={t("widgets.config.fields.titleOptional")} value={node.title} onChange={(title) => set({ ...node, title })} />
        <TextRow label={t("widgets.config.fields.descriptionOptional")} value={node.description} onChange={(description) => set({ ...node, description })} />
      </ListGroup>
      <ListGroup label={t("widgets.config.statTracker.stats")}>
        {node.stats.map((stat) => (
          <div key={stat.id} className="space-y-2.5 px-4 py-3">
            <div className="flex items-center gap-2">
              <Input aria-label={t("widgets.config.statTracker.labelPlaceholder")} placeholder={t("widgets.config.statTracker.labelPlaceholder")} value={stat.label} onChange={(event) => update(stat.id, { label: event.target.value })} />
              <IconButton label={t("widgets.config.statTracker.removeStat")} onClick={() => set({ ...node, stats: node.stats.filter((entry) => entry.id !== stat.id) })}>
                <Trash2 size={icon.md} />
              </IconButton>
            </div>
            <div className="grid grid-cols-3 gap-2">
              <NumberField nullable label={t("widgets.config.statTracker.min")} value={stat.min} onChange={(min) => update(stat.id, { min })} />
              <NumberField label={t("widgets.config.statTracker.start")} value={stat.value ?? 0} onChange={(value) => update(stat.id, { value: value ?? 0 })} />
              <NumberField nullable label={t("widgets.config.statTracker.max")} value={stat.max} onChange={(max) => update(stat.id, { max })} />
            </div>
          </div>
        ))}
        <div className="px-4 py-3">
          <Button variant="secondary" size="sm" leading={<Plus size={icon.sm} />} onClick={() => set({ ...node, stats: [...node.stats, { id: crypto.randomUUID(), label: t("widgets.config.statTracker.defaultStat"), value: 0, min: null, max: null }] })}>
            {t("widgets.config.statTracker.addStat")}
          </Button>
        </div>
      </ListGroup>
    </>
  );
}

function QuickSnippetsForm({ node, set }: FormProps<Node<"quick_snippets">>) {
  const { t } = useTranslation("conversations");
  const update = (id: string, patch: Partial<Node<"quick_snippets">["snippets"][number]>) =>
    set({ ...node, snippets: node.snippets.map((snippet) => (snippet.id === id ? { ...snippet, ...patch } : snippet)) });
  return (
    <>
      <ListGroup>
        <TextRow label={t("widgets.config.fields.titleOptional")} value={node.title} onChange={(title) => set({ ...node, title })} />
        <TextRow label={t("widgets.config.fields.descriptionOptional")} value={node.description} onChange={(description) => set({ ...node, description })} />
      </ListGroup>
      <ListGroup label={t("widgets.config.quickSnippets.snippets")}>
        {node.snippets.map((snippet) => (
          <div key={snippet.id} className="space-y-2.5 px-4 py-3">
            <div className="flex items-center gap-2">
              <Input aria-label={t("widgets.config.quickSnippets.labelPlaceholder")} placeholder={t("widgets.config.quickSnippets.labelPlaceholder")} value={snippet.label} onChange={(event) => update(snippet.id, { label: event.target.value })} />
              <IconButton label={t("widgets.config.quickSnippets.removeSnippet")} onClick={() => set({ ...node, snippets: node.snippets.filter((entry) => entry.id !== snippet.id) })}>
                <Trash2 size={icon.md} />
              </IconButton>
            </div>
            <Textarea rows={2} autoGrow maxRows={8} aria-label={t("widgets.config.quickSnippets.textPlaceholder")} placeholder={t("widgets.config.quickSnippets.textPlaceholder")} value={snippet.text} onChange={(event) => update(snippet.id, { text: event.target.value })} />
          </div>
        ))}
        <div className="px-4 py-3">
          <Button variant="secondary" size="sm" leading={<Plus size={icon.sm} />} onClick={() => set({ ...node, snippets: [...node.snippets, { id: crypto.randomUUID(), label: t("widgets.config.quickSnippets.newSnippet"), text: "" }] })}>
            {t("widgets.config.quickSnippets.addSnippet")}
          </Button>
        </div>
      </ListGroup>
    </>
  );
}

function DiceForm({ node, set }: FormProps<Node<"dice">>) {
  const { t } = useTranslation("conversations");
  return (
    <ListGroup>
      <TextRow label={t("widgets.config.fields.titleOptional")} value={node.title} onChange={(title) => set({ ...node, title })} />
      <TextRow label={t("widgets.config.dice.notation")} hint={t("widgets.config.dice.notationHint")} value={node.notation} onChange={(notation) => set({ ...node, notation })} />
    </ListGroup>
  );
}

function MemoryForm({ node, set }: FormProps<Node<"memory">>) {
  const { t } = useTranslation("conversations");
  return (
    <ListGroup>
      <TextRow label={t("widgets.config.fields.titleOptional")} value={node.title} onChange={(title) => set({ ...node, title })} />
      <EditorRow inline label={t("widgets.config.memory.maxEntries")} hint={t("widgets.config.memory.maxEntriesHint")}>
        <NumberInput value={node.limit ?? 10} min={1} max={100} onChange={(limit) => set({ ...node, limit })} />
      </EditorRow>
    </ListGroup>
  );
}

function AuthorNoteForm({ node, set }: FormProps<Node<"author_note">>) {
  const { t } = useTranslation("conversations");
  return (
    <ListGroup>
      <TextRow label={t("widgets.config.fields.title")} value={node.title} onChange={(title) => set({ ...node, title })} />
      <TextRow label={t("widgets.config.fields.descriptionOptional")} hint={t("widgets.config.authorNote.descriptionHint")} value={node.description} onChange={(description) => set({ ...node, description })} />
    </ListGroup>
  );
}

function TimeForm({ node, set }: FormProps<Node<"time">>) {
  const { t } = useTranslation("conversations");
  const row = (label: string, control: ReactNode) => (
    <EditorRow inline label={label}>
      {control}
    </EditorRow>
  );
  return (
    <ListGroup>
      <TextRow label={t("widgets.config.fields.titleOptional")} value={node.title} onChange={(title) => set({ ...node, title })} />
      {row(
        t("widgets.config.time.hourFormat"),
        <Segmented
          size="sm"
          aria-label={t("widgets.config.time.hourFormat")}
          value={node.hour_format ?? "h24"}
          onChange={(hour_format) => set({ ...node, hour_format })}
          options={[
            { value: "h12", label: t("widgets.config.time.h12") },
            { value: "h24", label: t("widgets.config.time.h24") },
          ]}
        />,
      )}
      {row(t("widgets.config.time.showSeconds"), <Switch checked={node.show_seconds ?? false} aria-label={t("widgets.config.time.showSeconds")} onChange={(show_seconds) => set({ ...node, show_seconds })} />)}
      {row(t("widgets.config.time.showDate"), <Switch checked={node.show_date ?? true} aria-label={t("widgets.config.time.showDate")} onChange={(show_date) => set({ ...node, show_date })} />)}
    </ListGroup>
  );
}
