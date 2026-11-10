import { useState, type ReactNode } from "react";
import {
  AlignLeft,
  Brain,
  CalendarDays,
  Clock,
  Dices,
  Image,
  ImagePlus,
  ListChecks,
  Minus,
  MousePointerClick,
  Palette,
  Plus,
  Shapes,
  SwatchBook,
  Timer,
  Trash2,
  Type,
  User,
} from "lucide-react";
import { useTranslation } from "react-i18next";
import { Avatar, Button, Field, icon, IconButton, Input, ListGroup, ListRow, NumberInput, Sheet, Switch, Textarea, useCompactLayout } from "@lettuceai/crisp";
import type { LibraryBoxVariant, LibraryButtonAction, LibraryWidgetDesign, LibraryWidgetImageShape } from "@/api/generated/bindings";
import { useClosingValue } from "@/shared/hooks/useClosingValue";
import { SettingsControlRow } from "@/shared/ui/SettingsControlRow";
import { SettingsPick, type PickOptions } from "@/shared/ui/SettingsPick";
import { defaultNotation } from "../data/dice";
import { useWidgets } from "./context";
import type { WidgetNode } from "./nodes";

type Node<T extends WidgetNode["type"]> = Extract<WidgetNode, { type: T }>;

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
  const designs = (["default", "minimal", "solid", "outline"] as const).map((value) => ({ value, label: t(`widgets.config.design.${value}`) }));
  const design =
    draft.type === "divider" || draft.type === "box" ? null : (
      <ChoiceRow<LibraryWidgetDesign>
        icon={<Palette size={icon.lg} />}
        title={t("widgets.config.design.label")}
        value={draft.design ?? "default"}
        options={designs}
        onChange={(value) => setDraft({ ...draft, design: value })}
      />
    );
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
        <Body node={draft} change={setDraft} design={design} />
      </div>
    </Sheet>
  );
}

function TextRow({ icon: glyph, title, description, placeholder, value, onChange }: { icon: ReactNode; title: string; description?: string | undefined; placeholder?: string | undefined; value: string | null; onChange: (value: string | null) => void }) {
  const phone = useCompactLayout();
  return (
    <SettingsControlRow
      phone={phone}
      icon={glyph}
      title={title}
      description={description}
      control={<Input className={phone ? "w-full" : "w-56"} aria-label={title} placeholder={placeholder} value={value ?? ""} onChange={(event) => onChange(event.target.value || null)} />}
    />
  );
}

function ChoiceRow<T extends string>({ icon: glyph, title, description, value, options, onChange }: { icon: ReactNode; title: string; description?: string; value: T; options: PickOptions<T>; onChange: (value: T) => void }) {
  const phone = useCompactLayout();
  return (
    <SettingsControlRow
      phone={phone}
      icon={glyph}
      title={title}
      description={description}
      control={<SettingsPick label={title} native={phone} block={phone} value={value} options={options} onChange={onChange} />}
    />
  );
}

function SwitchRow({ icon: glyph, title, checked, onChange }: { icon: ReactNode; title: string; checked: boolean; onChange: (checked: boolean) => void }) {
  return <ListRow icon={glyph} title={title} trailing={<Switch checked={checked} aria-label={title} onChange={onChange} />} />;
}

function TitleRow<N extends WidgetNode & { title: string | null }>({ node, set }: { node: N; set: (node: N) => void }) {
  const { t } = useTranslation("conversations");
  return <TextRow icon={<Type size={icon.lg} />} title={t("widgets.config.fields.title")} placeholder={t(`widgets.types.${node.type}.label`)} value={node.title} onChange={(title) => set({ ...node, title })} />;
}

function DescriptionRow<N extends WidgetNode & { description: string | null }>({ node, set, description }: { node: N; set: (node: N) => void; description?: string }) {
  const { t } = useTranslation("conversations");
  return <TextRow icon={<AlignLeft size={icon.lg} />} title={t("widgets.config.fields.description")} description={description} value={node.description} onChange={(value) => set({ ...node, description: value })} />;
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

interface FormProps<N extends WidgetNode> {
  node: N;
  set: (node: N) => void;
  /** The design row, the last row of the form's first group. */
  design: ReactNode;
}

function Body({ node, change, design }: { node: WidgetNode; change: (node: WidgetNode) => void; design: ReactNode }) {
  const set = <N extends WidgetNode>(next: N) => change(next);
  switch (node.type) {
    case "divider":
      return <DividerForm node={node} set={set} design={design} />;
    case "box":
      return <BoxForm node={node} set={set} design={design} />;
    case "character_info":
      return <CharacterInfoForm node={node} set={set} design={design} />;
    case "persona_info":
      return <ListGroup>{design}</ListGroup>;
    case "scratch_pad":
      return <ScratchPadForm node={node} set={set} design={design} />;
    case "image":
      return <ImageForm node={node} set={set} design={design} />;
    case "selector":
      return <SelectorForm node={node} set={set} design={design} />;
    case "button":
      return <ButtonForm node={node} set={set} design={design} />;
    case "stat_tracker":
      return <StatTrackerForm node={node} set={set} design={design} />;
    case "quick_snippets":
      return <QuickSnippetsForm node={node} set={set} design={design} />;
    case "dice":
      return <DiceForm node={node} set={set} design={design} />;
    case "memory":
      return <MemoryForm node={node} set={set} design={design} />;
    case "companion_state":
    case "session_info":
      return (
        <ListGroup>
          <TitleRow node={node} set={set} />
          {design}
        </ListGroup>
      );
    case "author_note":
      return <AuthorNoteForm node={node} set={set} design={design} />;
    case "time":
      return <TimeForm node={node} set={set} design={design} />;
  }
}

function DividerForm({ node, set }: FormProps<Node<"divider">>) {
  const { t } = useTranslation("conversations");
  return (
    <ListGroup>
      <ChoiceRow
        icon={<Minus size={icon.lg} />}
        title={t("widgets.config.divider.style")}
        value={node.style ?? "line"}
        options={[
          { value: "line", label: t("widgets.config.divider.line") },
          { value: "space", label: t("widgets.config.divider.space") },
        ]}
        onChange={(style) => set({ ...node, style })}
      />
    </ListGroup>
  );
}

function BoxForm({ node, set }: FormProps<Node<"box">>) {
  const { t } = useTranslation("conversations");
  const variants: LibraryBoxVariant[] = ["default", "subtle", "info", "warning", "success", "danger"];
  return (
    <ListGroup>
      <TitleRow node={node} set={set} />
      <DescriptionRow node={node} set={set} />
      <ChoiceRow
        icon={<SwatchBook size={icon.lg} />}
        title={t("widgets.config.box.variant")}
        value={node.variant ?? "default"}
        options={variants.map((value) => ({ value, label: t(`widgets.config.box.${value}`) }))}
        onChange={(variant) => set({ ...node, variant })}
      />
    </ListGroup>
  );
}

function CharacterInfoForm({ node, set, design }: FormProps<Node<"character_info">>) {
  const { t } = useTranslation("conversations");
  const { members } = useWidgets();
  return (
    <ListGroup>
      {members.length > 0 && (
        <ChoiceRow
          icon={<User size={icon.lg} />}
          title={t("widgets.config.characterInfo.character")}
          description={t("widgets.config.characterInfo.characterHint")}
          value={node.character_id ?? ""}
          options={[{ value: "", label: t("widgets.config.characterInfo.firstMember") }, ...members.flatMap((member) => (member.id ? [{ value: member.id, label: member.name }] : []))]}
          onChange={(id) => set({ ...node, character_id: id || null })}
        />
      )}
      {design}
    </ListGroup>
  );
}

function ScratchPadForm({ node, set, design }: FormProps<Node<"scratch_pad">>) {
  const { t } = useTranslation("conversations");
  return (
    <>
      <ListGroup>
        <TitleRow node={node} set={set} />
        <DescriptionRow node={node} set={set} />
        {design}
      </ListGroup>
      <ListGroup label={t("widgets.config.fields.content")}>
        <div className="px-4 py-3">
          <Textarea rows={6} autoGrow maxRows={14} aria-label={t("widgets.config.fields.content")} value={node.content ?? ""} onChange={(event) => set({ ...node, content: event.target.value })} />
        </div>
      </ListGroup>
    </>
  );
}

function ImageForm({ node, set, design }: FormProps<Node<"image">>) {
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
      <TitleRow node={node} set={set} />
      <DescriptionRow node={node} set={set} />
      <ChoiceRow
        icon={<Image size={icon.lg} />}
        title={t("widgets.config.image.source")}
        value={kind}
        options={[
          { value: "character_avatar", label: t("widgets.config.image.character") },
          { value: "persona_avatar", label: t("widgets.config.image.persona") },
          { value: "logical_asset", label: t("widgets.config.image.upload") },
        ]}
        onChange={(next) => set({ ...node, source: next === "logical_asset" ? { kind: "unresolved_legacy", token: "" } : { kind: next } })}
      />
      {kind === "logical_asset" && (
        <ListRow
          icon={url ? <Avatar src={url} name={node.title ?? t("widgets.image.alt")} size="sm" /> : <ImagePlus size={icon.lg} />}
          title={node.source.kind === "logical_asset" ? t("widgets.config.image.imageSelected") : t("widgets.config.image.noImageSelected")}
          trailing={
            <Button variant="secondary" size="sm" loading={picking} onClick={() => void pick()}>
              {node.source.kind === "logical_asset" ? t("widgets.config.image.replaceImage") : t("widgets.config.image.chooseFile")}
            </Button>
          }
        />
      )}
      <ChoiceRow
        icon={<Shapes size={icon.lg} />}
        title={t("widgets.config.image.shape")}
        value={node.shape ?? "auto"}
        options={shapes.map((value) => ({ value, label: t(`widgets.config.image.${value}`) }))}
        onChange={(shape) => set({ ...node, shape })}
      />
      {design}
    </ListGroup>
  );
}

function SelectorForm({ node, set, design }: FormProps<Node<"selector">>) {
  const { t } = useTranslation("conversations");
  return (
    <ListGroup>
      <ChoiceRow
        icon={<ListChecks size={icon.lg} />}
        title={t("widgets.config.selector.selects")}
        value={node.kind}
        options={[
          { value: "persona", label: t("widgets.config.selector.persona") },
          { value: "model", label: t("widgets.config.selector.model") },
          { value: "author_note", label: t("widgets.config.selector.authorNote") },
        ]}
        onChange={(kind) => set({ ...node, kind })}
      />
      <TitleRow node={node} set={set} />
      <DescriptionRow node={node} set={set} />
      {design}
    </ListGroup>
  );
}

function ButtonForm({ node, set, design }: FormProps<Node<"button">>) {
  const { t } = useTranslation("conversations");
  const actions: { value: Exclude<LibraryButtonAction, "toggle_voice_autoplay">; label: string }[] = [
    { value: "regenerate", label: t("widgets.config.button.regenerate") },
    { value: "continue", label: t("widgets.config.button.continue") },
    { value: "swap_places", label: t("widgets.config.button.swapPlaces") },
    { value: "abort", label: t("widgets.config.button.stop") },
    { value: "new_session", label: t("widgets.config.button.newSession") },
    { value: "view_history", label: t("widgets.config.button.history") },
    { value: "open_memories", label: t("widgets.config.button.memories") },
    { value: "open_search", label: t("widgets.config.button.search") },
  ];
  return (
    <ListGroup>
      <ChoiceRow
        icon={<MousePointerClick size={icon.lg} />}
        title={t("widgets.config.button.action")}
        value={node.action === "toggle_voice_autoplay" ? "regenerate" : node.action}
        options={actions}
        onChange={(action) => set({ ...node, action })}
      />
      <TitleRow node={node} set={set} />
      <DescriptionRow node={node} set={set} />
      {design}
    </ListGroup>
  );
}

function StatTrackerForm({ node, set, design }: FormProps<Node<"stat_tracker">>) {
  const { t } = useTranslation("conversations");
  const update = (id: string, patch: Partial<Node<"stat_tracker">["stats"][number]>) =>
    set({ ...node, stats: node.stats.map((stat) => (stat.id === id ? { ...stat, ...patch } : stat)) });
  return (
    <>
      <ListGroup>
        <TitleRow node={node} set={set} />
        <DescriptionRow node={node} set={set} />
        {design}
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

function QuickSnippetsForm({ node, set, design }: FormProps<Node<"quick_snippets">>) {
  const { t } = useTranslation("conversations");
  const update = (id: string, patch: Partial<Node<"quick_snippets">["snippets"][number]>) =>
    set({ ...node, snippets: node.snippets.map((snippet) => (snippet.id === id ? { ...snippet, ...patch } : snippet)) });
  return (
    <>
      <ListGroup>
        <TitleRow node={node} set={set} />
        <DescriptionRow node={node} set={set} />
        {design}
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

function DiceForm({ node, set, design }: FormProps<Node<"dice">>) {
  const { t } = useTranslation("conversations");
  return (
    <ListGroup>
      <TitleRow node={node} set={set} />
      <TextRow icon={<Dices size={icon.lg} />} title={t("widgets.config.dice.notation")} description={t("widgets.config.dice.notationHint")} placeholder={defaultNotation} value={node.notation} onChange={(notation) => set({ ...node, notation })} />
      {design}
    </ListGroup>
  );
}

function MemoryForm({ node, set, design }: FormProps<Node<"memory">>) {
  const { t } = useTranslation("conversations");
  const phone = useCompactLayout();
  return (
    <ListGroup>
      <TitleRow node={node} set={set} />
      <SettingsControlRow
        phone={phone}
        icon={<Brain size={icon.lg} />}
        title={t("widgets.config.memory.maxEntries")}
        description={t("widgets.config.memory.maxEntriesHint")}
        control={<NumberInput className={phone ? "w-full" : "w-32"} value={node.limit ?? 10} min={1} max={100} onChange={(limit) => set({ ...node, limit })} />}
      />
      {design}
    </ListGroup>
  );
}

function AuthorNoteForm({ node, set, design }: FormProps<Node<"author_note">>) {
  const { t } = useTranslation("conversations");
  return (
    <ListGroup>
      <TitleRow node={node} set={set} />
      <DescriptionRow node={node} set={set} description={t("widgets.config.authorNote.descriptionHint")} />
      {design}
    </ListGroup>
  );
}

function TimeForm({ node, set, design }: FormProps<Node<"time">>) {
  const { t } = useTranslation("conversations");
  return (
    <ListGroup>
      <TitleRow node={node} set={set} />
      <ChoiceRow
        icon={<Clock size={icon.lg} />}
        title={t("widgets.config.time.hourFormat")}
        value={node.hour_format ?? "h24"}
        options={[
          { value: "h12", label: t("widgets.config.time.h12") },
          { value: "h24", label: t("widgets.config.time.h24") },
        ]}
        onChange={(hour_format) => set({ ...node, hour_format })}
      />
      <SwitchRow icon={<Timer size={icon.lg} />} title={t("widgets.config.time.showSeconds")} checked={node.show_seconds ?? false} onChange={(show_seconds) => set({ ...node, show_seconds })} />
      <SwitchRow icon={<CalendarDays size={icon.lg} />} title={t("widgets.config.time.showDate")} checked={node.show_date ?? true} onChange={(show_date) => set({ ...node, show_date })} />
      {design}
    </ListGroup>
  );
}

