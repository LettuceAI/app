import { useEffect, useRef, useState } from "react";
import { Image as ImageIcon, ImagePlus, Loader2, Plus, Trash2, Upload } from "lucide-react";
import { BottomMenu } from "../../../../../components";
import { NumberInput } from "../../../../../components/NumberInput";
import type {
  AuthorNoteNode,
  BoxNode,
  BoxVariant,
  ButtonAction,
  ButtonNode,
  DiceNode,
  DividerNode,
  ImageNode,
  ImageShape,
  ImageSource,
  QuickSnippetsNode,
  ScratchPadNode,
  SelectorKind,
  SelectorNode,
  StatTrackerNode,
  WidgetDesign,
  WidgetNode,
} from "../../../../../../core/storage/chatWidgetSchemas";
import { convertToImageRef } from "../../../../../../core/storage/images";
import { useI18n } from "../../../../../../core/i18n/context";
import { uuidv4 } from "../../../../../../core/storage/repo";
import { useImageData } from "../../../../../hooks/useImageData";
import { WIDGET_TYPE_LABEL } from "./widgetFactories";
import { useWidgetEdit } from "../WidgetEditContext";
import { useWidgetContext } from "../WidgetContext";
import type { CharacterInfoNode } from "../../../../../../core/storage/chatWidgetSchemas";

const makeId = () => uuidv4();

interface FieldProps {
  label: string;
  children: React.ReactNode;
  hint?: string;
}
function Field({ label, hint, children }: FieldProps) {
  return (
    <label className="flex flex-col gap-1">
      <span className="text-xs font-medium text-fg/60">{label}</span>
      {children}
      {hint && <span className="text-[11px] text-fg/40">{hint}</span>}
    </label>
  );
}

interface SegmentedProps<T extends string> {
  value: T;
  options: { value: T; label: string }[];
  onChange: (next: T) => void;
}
function Segmented<T extends string>({ value, options, onChange }: SegmentedProps<T>) {
  return (
    <div
      className={`grid gap-1.5 ${options.length <= 3 ? "grid-cols-3" : "grid-cols-2"}`}
    >
      {options.map((opt) => (
        <button
          key={opt.value}
          type="button"
          onClick={() => onChange(opt.value)}
          className={`rounded-lg border py-2 text-[11px] font-medium transition ${
            value === opt.value
              ? "border-accent/50 bg-accent/10 text-accent"
              : "border-fg/8 bg-fg/5 text-fg/55 hover:bg-fg/10"
          }`}
        >
          {opt.label}
        </button>
      ))}
    </div>
  );
}

const TEXT_INPUT_BASE =
  "rounded-lg border border-fg/10 bg-fg/5 px-3 py-2 text-sm text-fg/80 focus:border-accent/40 focus:outline-none";
const TEXT_INPUT_CLASS = `w-full ${TEXT_INPUT_BASE}`;

interface WidgetConfigSheetProps {
  open: boolean;
  node: WidgetNode | null;
  onClose: () => void;
  onSave: (next: WidgetNode) => void;
}

export function WidgetConfigSheet({
  open,
  node,
  onClose,
  onSave,
}: WidgetConfigSheetProps) {
  const { t } = useI18n();
  const edit = useWidgetEdit();
  const [draft, setDraft] = useState<WidgetNode | null>(node);

  useEffect(() => {
    setDraft(node);
  }, [node]);

  if (!draft) {
    return (
      <BottomMenu isOpen={open} onClose={onClose} title={t("chats.widgets.config.editTitle")}>
        <div className="px-2 py-4 text-sm text-fg/50">{t("chats.widgets.config.noWidgetSelected")}</div>
      </BottomMenu>
    );
  }

  const commit = () => {
    onSave(draft);
    onClose();
  };

  const chooseLibrary = () => {
    if (draft.type !== "image") return;
    const next: WidgetNode = {
      ...draft,
      source: {
        kind: "library",
        path: draft.source.kind === "library" ? draft.source.path : "",
      },
    };
    edit.chooseLibraryImage(next);
  };

  return (
    <BottomMenu
      isOpen={open}
      onClose={onClose}
      title={t(WIDGET_TYPE_LABEL[draft.type])}
    >
      <form
        className="flex flex-col gap-3"
        onSubmit={(e) => {
          e.preventDefault();
          commit();
        }}
      >
        {renderBody(draft, setDraft, chooseLibrary)}
        <div className="flex justify-end gap-2 pt-2">
          <button
            type="button"
            onClick={onClose}
            className="rounded-lg border border-fg/10 bg-fg/5 px-3 py-2 text-sm text-fg/70 hover:bg-fg/10"
          >
            {t("common.buttons.cancel")}
          </button>
          <button
            type="submit"
            className="rounded-lg border border-accent/40 bg-accent/15 px-3 py-2 text-sm font-medium text-accent hover:bg-accent/25"
          >
            {t("common.buttons.save")}
          </button>
        </div>
      </form>
    </BottomMenu>
  );
}

function DesignField({
  node,
  setNode,
}: {
  node: WidgetNode;
  setNode: (n: WidgetNode) => void;
}) {
  const { t } = useI18n();
  return (
    <Field label={t("chats.widgets.config.design.label")}>
      <Segmented<WidgetDesign>
        value={node.design ?? "default"}
        options={[
          { value: "default", label: t("chats.widgets.config.design.default") },
          { value: "minimal", label: t("chats.widgets.config.design.minimal") },
          { value: "solid", label: t("chats.widgets.config.design.solid") },
          { value: "outline", label: t("chats.widgets.config.design.outline") },
        ]}
        onChange={(v) => setNode({ ...node, design: v })}
      />
    </Field>
  );
}

function MemoryLimitField({
  draft,
  setDraft,
}: {
  draft: Extract<WidgetNode, { type: "memory" }>;
  setDraft: (next: WidgetNode) => void;
}) {
  const { t } = useI18n();
  return (
    <Field
      label={t("chats.widgets.config.memory.maxEntries")}
      hint={t("chats.widgets.config.memory.maxEntriesHint")}
    >
      <input
        type="number"
        min={1}
        max={100}
        className={TEXT_INPUT_CLASS}
        value={draft.limit ?? 10}
        onChange={(e) => {
          const n = Number(e.target.value);
          setDraft({ ...draft, limit: Number.isFinite(n) ? n : undefined });
        }}
      />
    </Field>
  );
}

function renderBody(
  draft: WidgetNode,
  setDraft: (next: WidgetNode) => void,
  onChooseLibrary: () => void,
): React.ReactNode {
  const design =
    draft.type !== "divider" && draft.type !== "box" ? (
      <DesignField node={draft} setNode={setDraft} />
    ) : null;
  switch (draft.type) {
    case "divider":
      return <DividerForm node={draft} setNode={setDraft} />;
    case "box":
      return <BoxForm node={draft} setNode={setDraft} />;
    case "character_info":
      return (
        <>
          <CharacterInfoForm node={draft} setNode={setDraft} />
          {design}
        </>
      );
    case "persona_info":
      return design;
    case "scratch_pad":
      return (
        <>
          <ScratchPadForm node={draft} setNode={setDraft} />
          {design}
        </>
      );
    case "image":
      return (
        <>
          <ImageForm node={draft} setNode={setDraft} onChooseLibrary={onChooseLibrary} />
          {design}
        </>
      );
    case "selector":
      return (
        <>
          <SelectorForm node={draft} setNode={setDraft} />
          {design}
        </>
      );
    case "button":
      return (
        <>
          <ButtonForm node={draft} setNode={setDraft} />
          {design}
        </>
      );
    case "stat_tracker":
      return (
        <>
          <StatTrackerForm node={draft} setNode={setDraft} />
          {design}
        </>
      );
    case "quick_snippets":
      return (
        <>
          <QuickSnippetsForm node={draft} setNode={setDraft} />
          {design}
        </>
      );
    case "dice":
      return (
        <>
          <DiceForm node={draft} setNode={setDraft} />
          {design}
        </>
      );
    case "memory":
      return (
        <>
          <TitleOnlyForm node={draft} setNode={setDraft} />
          <MemoryLimitField draft={draft} setDraft={setDraft} />
          {design}
        </>
      );
    case "companion_state":
    case "session_info":
      return (
        <>
          <TitleOnlyForm node={draft} setNode={setDraft} />
          {design}
        </>
      );
    case "author_note":
      return (
        <>
          <AuthorNoteForm node={draft} setNode={setDraft} />
          {design}
        </>
      );
  }
}

function CharacterInfoForm({
  node,
  setNode,
}: {
  node: CharacterInfoNode;
  setNode: (n: CharacterInfoNode) => void;
}) {
  const { t } = useI18n();
  const { characters } = useWidgetContext();
  if (!characters || characters.length === 0) return null;
  return (
    <Field
      label={t("chats.widgets.config.characterInfo.character")}
      hint={t("chats.widgets.config.characterInfo.characterHint")}
    >
      <select
        className={TEXT_INPUT_CLASS}
        value={node.characterId ?? ""}
        onChange={(e) => setNode({ ...node, characterId: e.target.value || undefined })}
      >
        <option value="">{t("chats.widgets.config.characterInfo.firstMember")}</option>
        {characters.map((c) => (
          <option key={c.id} value={c.id}>
            {c.name}
          </option>
        ))}
      </select>
    </Field>
  );
}

function AuthorNoteForm({
  node,
  setNode,
}: {
  node: AuthorNoteNode;
  setNode: (n: AuthorNoteNode) => void;
}) {
  const { t } = useI18n();
  return (
    <>
      <Field label={t("chats.widgets.config.fields.title")}>
        <input
          type="text"
          className={TEXT_INPUT_CLASS}
          value={node.title ?? ""}
          onChange={(e) => setNode({ ...node, title: e.target.value })}
        />
      </Field>
      <Field
        label={t("chats.widgets.config.fields.descriptionOptional")}
        hint={t("chats.widgets.config.authorNote.descriptionHint")}
      >
        <input
          type="text"
          className={TEXT_INPUT_CLASS}
          value={node.description ?? ""}
          onChange={(e) => setNode({ ...node, description: e.target.value })}
        />
      </Field>
    </>
  );
}

function TitleOnlyForm({
  node,
  setNode,
}: {
  node: WidgetNode & { title?: string };
  setNode: (n: WidgetNode) => void;
}) {
  const { t } = useI18n();
  return (
    <Field label={t("chats.widgets.config.fields.titleOptional")}>
      <input
        type="text"
        className={TEXT_INPUT_CLASS}
        value={node.title ?? ""}
        onChange={(e) => setNode({ ...node, title: e.target.value } as WidgetNode)}
      />
    </Field>
  );
}

function StatTrackerForm({
  node,
  setNode,
}: {
  node: StatTrackerNode;
  setNode: (n: StatTrackerNode) => void;
}) {
  const { t } = useI18n();
  const updateStat = (
    id: string,
    patch: Partial<{ label: string; value: number; min?: number; max?: number }>,
  ) =>
    setNode({
      ...node,
      stats: node.stats.map((s) => (s.id === id ? { ...s, ...patch } : s)),
    });
  return (
    <>
      <Field label={t("chats.widgets.config.fields.titleOptional")}>
        <input
          type="text"
          className={TEXT_INPUT_CLASS}
          value={node.title ?? ""}
          onChange={(e) => setNode({ ...node, title: e.target.value })}
        />
      </Field>
      <Field label={t("chats.widgets.config.statTracker.stats")}>
        <div className="flex flex-col gap-2">
          {node.stats.map((stat) => (
            <div
              key={stat.id}
              className="flex flex-col gap-2 rounded-lg border border-fg/10 bg-fg/[0.03] p-2"
            >
              <div className="flex items-center gap-2">
                <input
                  type="text"
                  placeholder={t("chats.widgets.config.statTracker.labelPlaceholder")}
                  className={`${TEXT_INPUT_BASE} min-w-0 flex-1`}
                  value={stat.label}
                  onChange={(e) => updateStat(stat.id, { label: e.target.value })}
                />
                <button
                  type="button"
                  onClick={() =>
                    setNode({ ...node, stats: node.stats.filter((s) => s.id !== stat.id) })
                  }
                  className="flex h-8 w-8 shrink-0 items-center justify-center rounded-md border border-fg/15 bg-fg/5 text-fg/50 hover:border-danger/40 hover:text-danger"
                  aria-label={t("chats.widgets.config.statTracker.removeStat")}
                >
                  <Trash2 size={13} />
                </button>
              </div>
              <div className="grid grid-cols-3 gap-2">
                <label className="flex flex-col gap-1">
                  <span className="text-[10px] font-medium uppercase tracking-wide text-fg/45">
                    {t("chats.widgets.config.statTracker.min")}
                  </span>
                  <NumberInput
                    value={stat.min ?? null}
                    max={stat.max}
                    placeholder="-"
                    className={`${TEXT_INPUT_BASE} w-full`}
                    onChange={(next) => updateStat(stat.id, { min: next ?? undefined })}
                  />
                </label>
                <label className="flex flex-col gap-1">
                  <span className="text-[10px] font-medium uppercase tracking-wide text-fg/45">
                    {t("chats.widgets.config.statTracker.start")}
                  </span>
                  <NumberInput
                    value={stat.value}
                    min={stat.min}
                    max={stat.max}
                    className={`${TEXT_INPUT_BASE} w-full`}
                    onChange={(next) => updateStat(stat.id, { value: next ?? 0 })}
                  />
                </label>
                <label className="flex flex-col gap-1">
                  <span className="text-[10px] font-medium uppercase tracking-wide text-fg/45">
                    {t("chats.widgets.config.statTracker.max")}
                  </span>
                  <NumberInput
                    value={stat.max ?? null}
                    min={stat.min}
                    placeholder="-"
                    className={`${TEXT_INPUT_BASE} w-full`}
                    onChange={(next) => updateStat(stat.id, { max: next ?? undefined })}
                  />
                </label>
              </div>
            </div>
          ))}
          <button
            type="button"
            onClick={() =>
              setNode({
                ...node,
                stats: [...node.stats, { id: makeId(), label: t("chats.widgets.config.statTracker.defaultStat"), value: 0 }],
              })
            }
            className="flex items-center justify-center gap-1 rounded-lg border border-dashed border-fg/20 py-1.5 text-[11px] font-medium text-fg/60 hover:border-accent/40 hover:text-accent"
          >
            <Plus size={12} strokeWidth={2.4} /> {t("chats.widgets.config.statTracker.addStat")}
          </button>
        </div>
      </Field>
    </>
  );
}

function QuickSnippetsForm({
  node,
  setNode,
}: {
  node: QuickSnippetsNode;
  setNode: (n: QuickSnippetsNode) => void;
}) {
  const { t } = useI18n();
  const updateSnippet = (id: string, patch: Partial<{ label: string; text: string }>) =>
    setNode({
      ...node,
      snippets: node.snippets.map((s) => (s.id === id ? { ...s, ...patch } : s)),
    });
  return (
    <>
      <Field label={t("chats.widgets.config.fields.titleOptional")}>
        <input
          type="text"
          className={TEXT_INPUT_CLASS}
          value={node.title ?? ""}
          onChange={(e) => setNode({ ...node, title: e.target.value })}
        />
      </Field>
      <Field label={t("chats.widgets.config.quickSnippets.snippets")}>
        <div className="flex flex-col gap-2">
          {node.snippets.map((snippet) => (
            <div key={snippet.id} className="flex items-center gap-2">
              <input
                type="text"
                placeholder={t("chats.widgets.config.quickSnippets.labelPlaceholder")}
                className={`${TEXT_INPUT_BASE} w-24 shrink-0`}
                value={snippet.label}
                onChange={(e) => updateSnippet(snippet.id, { label: e.target.value })}
              />
              <input
                type="text"
                placeholder={t("chats.widgets.config.quickSnippets.textPlaceholder")}
                className={`${TEXT_INPUT_BASE} min-w-0 flex-1`}
                value={snippet.text}
                onChange={(e) => updateSnippet(snippet.id, { text: e.target.value })}
              />
              <button
                type="button"
                onClick={() =>
                  setNode({
                    ...node,
                    snippets: node.snippets.filter((s) => s.id !== snippet.id),
                  })
                }
                className="flex h-8 w-8 shrink-0 items-center justify-center rounded-md border border-fg/15 bg-fg/5 text-fg/50 hover:border-danger/40 hover:text-danger"
                aria-label={t("chats.widgets.config.quickSnippets.removeSnippet")}
              >
                <Trash2 size={13} />
              </button>
            </div>
          ))}
          <button
            type="button"
            onClick={() =>
              setNode({
                ...node,
                snippets: [...node.snippets, { id: makeId(), label: t("chats.widgets.config.quickSnippets.newSnippet"), text: "" }],
              })
            }
            className="flex items-center justify-center gap-1 rounded-lg border border-dashed border-fg/20 py-1.5 text-[11px] font-medium text-fg/60 hover:border-accent/40 hover:text-accent"
          >
            <Plus size={12} strokeWidth={2.4} /> {t("chats.widgets.config.quickSnippets.addSnippet")}
          </button>
        </div>
      </Field>
    </>
  );
}

function DiceForm({
  node,
  setNode,
}: {
  node: DiceNode;
  setNode: (n: DiceNode) => void;
}) {
  const { t } = useI18n();
  return (
    <>
      <Field label={t("chats.widgets.config.fields.titleOptional")}>
        <input
          type="text"
          className={TEXT_INPUT_CLASS}
          value={node.title ?? ""}
          onChange={(e) => setNode({ ...node, title: e.target.value })}
        />
      </Field>
      <Field label={t("chats.widgets.config.dice.notation")} hint={t("chats.widgets.config.dice.notationHint")}>
        <input
          type="text"
          className={TEXT_INPUT_CLASS}
          value={node.notation ?? ""}
          placeholder="1d20"
          onChange={(e) => setNode({ ...node, notation: e.target.value })}
        />
      </Field>
    </>
  );
}

function DividerForm({
  node,
  setNode,
}: {
  node: DividerNode;
  setNode: (n: DividerNode) => void;
}) {
  const { t } = useI18n();
  return (
    <Field label={t("chats.widgets.config.divider.style")}>
      <Segmented
        value={node.style ?? "line"}
        options={[
          { value: "line", label: t("chats.widgets.config.divider.line") },
          { value: "space", label: t("chats.widgets.config.divider.space") },
        ]}
        onChange={(v) => setNode({ ...node, style: v })}
      />
    </Field>
  );
}

function BoxForm({
  node,
  setNode,
}: {
  node: BoxNode;
  setNode: (n: BoxNode) => void;
}) {
  const { t } = useI18n();
  return (
    <>
      <Field label={t("chats.widgets.config.fields.title")}>
        <input
          type="text"
          className={TEXT_INPUT_CLASS}
          value={node.title ?? ""}
          onChange={(e) => setNode({ ...node, title: e.target.value })}
        />
      </Field>
      <Field label={t("chats.widgets.config.fields.descriptionOptional")}>
        <input
          type="text"
          className={TEXT_INPUT_CLASS}
          value={node.description ?? ""}
          onChange={(e) => setNode({ ...node, description: e.target.value })}
        />
      </Field>
      <Field label={t("chats.widgets.config.box.variant")}>
        <Segmented<BoxVariant>
          value={node.variant ?? "default"}
          options={[
            { value: "default", label: t("chats.widgets.config.box.default") },
            { value: "subtle", label: t("chats.widgets.config.box.subtle") },
            { value: "info", label: t("chats.widgets.config.box.info") },
            { value: "warning", label: t("chats.widgets.config.box.warning") },
            { value: "success", label: t("chats.widgets.config.box.success") },
            { value: "danger", label: t("chats.widgets.config.box.danger") },
          ]}
          onChange={(v) => setNode({ ...node, variant: v })}
        />
      </Field>
      <p className="text-[11px] text-fg/40">
        {t("chats.widgets.config.box.childrenHint")}
      </p>
    </>
  );
}

function ScratchPadForm({
  node,
  setNode,
}: {
  node: ScratchPadNode;
  setNode: (n: ScratchPadNode) => void;
}) {
  const { t } = useI18n();
  return (
    <>
      <Field label={t("chats.widgets.config.fields.title")}>
        <input
          type="text"
          className={TEXT_INPUT_CLASS}
          value={node.title ?? ""}
          onChange={(e) => setNode({ ...node, title: e.target.value })}
        />
      </Field>
      <Field label={t("chats.widgets.config.fields.descriptionOptional")}>
        <input
          type="text"
          className={TEXT_INPUT_CLASS}
          value={node.description ?? ""}
          onChange={(e) => setNode({ ...node, description: e.target.value })}
        />
      </Field>
      <Field label={t("common.labels.content")} hint={t("chats.widgets.config.scratchPad.markdownHint")}>
        <textarea
          rows={8}
          className={`${TEXT_INPUT_CLASS} resize-y font-mono text-[12px]`}
          value={node.content ?? ""}
          onChange={(e) => setNode({ ...node, content: e.target.value })}
        />
      </Field>
    </>
  );
}

function ImageForm({
  node,
  setNode,
  onChooseLibrary,
}: {
  node: ImageNode;
  setNode: (n: ImageNode) => void;
  onChooseLibrary: () => void;
}) {
  const { t } = useI18n();
  const sourceKind = node.source.kind;
  const fileRef = useRef<HTMLInputElement>(null);
  const [uploadBusy, setUploadBusy] = useState(false);
  const updateSource = (next: ImageSource) => setNode({ ...node, source: next });

  const currentPath =
    node.source.kind === "upload" || node.source.kind === "library"
      ? node.source.path
      : "";
  const previewUrl = useImageData(currentPath || null);

  const handleFile = async (file: File | undefined) => {
    if (!file) return;
    setUploadBusy(true);
    try {
      const dataUrl = await new Promise<string>((resolve, reject) => {
        const reader = new FileReader();
        reader.onload = () => resolve(reader.result as string);
        reader.onerror = () => reject(new Error("read failed"));
        reader.readAsDataURL(file);
      });
      const id = await convertToImageRef(dataUrl);
      if (id) updateSource({ kind: "upload", path: id });
    } catch (err) {
      console.error("Widget image upload failed:", err);
    } finally {
      setUploadBusy(false);
    }
  };

  const handleSourceClick = (kind: ImageSource["kind"]) => {
    if (kind === "character_avatar") {
      updateSource({ kind: "character_avatar" });
    } else if (kind === "persona_avatar") {
      updateSource({ kind: "persona_avatar" });
    } else if (kind === "library") {
      onChooseLibrary();
    } else {
      if (node.source.kind !== "upload") updateSource({ kind: "upload", path: "" });
      fileRef.current?.click();
    }
  };

  return (
    <>
      <input
        ref={fileRef}
        type="file"
        accept="image/*"
        className="hidden"
        onChange={(e) => void handleFile(e.target.files?.[0])}
      />
      <Field label={t("chats.widgets.config.fields.titleOptional")}>
        <input
          type="text"
          className={TEXT_INPUT_CLASS}
          value={node.title ?? ""}
          onChange={(e) => setNode({ ...node, title: e.target.value })}
        />
      </Field>
      <Field label={t("chats.widgets.config.fields.descriptionOptional")}>
        <input
          type="text"
          className={TEXT_INPUT_CLASS}
          value={node.description ?? ""}
          onChange={(e) => setNode({ ...node, description: e.target.value })}
        />
      </Field>
      <Field label={t("chats.widgets.config.image.source")}>
        <Segmented<ImageSource["kind"]>
          value={sourceKind}
          options={[
            { value: "character_avatar", label: t("chats.widgets.config.image.character") },
            { value: "persona_avatar", label: t("chats.widgets.config.image.persona") },
            { value: "library", label: t("chats.widgets.config.image.library") },
            { value: "upload", label: t("chats.widgets.config.image.upload") },
          ]}
          onChange={handleSourceClick}
        />
      </Field>
      {(sourceKind === "library" || sourceKind === "upload") && (
        <div className="flex items-center gap-3 rounded-lg border border-fg/10 bg-fg/5 p-2.5">
          <div className="h-16 w-16 shrink-0 overflow-hidden rounded-lg border border-fg/10 bg-fg/5">
            {uploadBusy ? (
              <div className="flex h-full w-full items-center justify-center text-fg/40">
                <Loader2 size={18} className="animate-spin" />
              </div>
            ) : previewUrl ? (
              <img src={previewUrl} alt="" className="h-full w-full object-cover" />
            ) : (
              <div className="flex h-full w-full items-center justify-center text-fg/30">
                <ImagePlus size={18} />
              </div>
            )}
          </div>
          <div className="min-w-0 flex-1">
            <p className="text-[12px] text-fg/60">
              {currentPath
                ? t("chats.widgets.config.image.imageSelected")
                : t("chats.widgets.config.image.noImageSelected")}
            </p>
            <button
              type="button"
              onClick={() =>
                sourceKind === "library" ? onChooseLibrary() : fileRef.current?.click()
              }
              className="mt-1.5 flex items-center gap-1.5 rounded-md border border-fg/15 bg-fg/5 px-2.5 py-1.5 text-[12px] text-fg/80 transition hover:bg-fg/10"
            >
              {sourceKind === "library" ? (
                <ImageIcon size={13} />
              ) : (
                <Upload size={13} />
              )}
              {currentPath
                ? sourceKind === "library"
                  ? t("chats.widgets.config.image.chooseAnother")
                  : t("chats.widgets.config.image.replaceImage")
                : sourceKind === "library"
                  ? t("chats.widgets.config.image.chooseFromLibrary")
                  : t("chats.widgets.config.image.chooseFile")}
            </button>
          </div>
        </div>
      )}
      <Field label={t("chats.widgets.config.image.shape")}>
        <Segmented<ImageShape>
          value={node.shape ?? "auto"}
          options={[
            { value: "auto", label: t("chats.widgets.config.image.auto") },
            { value: "square", label: t("chats.widgets.config.image.square") },
            { value: "wide", label: t("chats.widgets.config.image.wide") },
            { value: "circle", label: t("chats.widgets.config.image.circle") },
          ]}
          onChange={(v) => setNode({ ...node, shape: v })}
        />
      </Field>
    </>
  );
}

function SelectorForm({
  node,
  setNode,
}: {
  node: SelectorNode;
  setNode: (n: SelectorNode) => void;
}) {
  const { t } = useI18n();
  return (
    <>
      <Field label={t("chats.widgets.config.fields.titleOptional")}>
        <input
          type="text"
          className={TEXT_INPUT_CLASS}
          value={node.title ?? ""}
          onChange={(e) => setNode({ ...node, title: e.target.value })}
        />
      </Field>
      <Field label={t("chats.widgets.config.fields.descriptionOptional")}>
        <input
          type="text"
          className={TEXT_INPUT_CLASS}
          value={node.description ?? ""}
          onChange={(e) => setNode({ ...node, description: e.target.value })}
        />
      </Field>
      <Field label={t("chats.widgets.config.selector.selects")}>
        <Segmented<SelectorKind>
          value={node.kind}
          options={[
            { value: "persona", label: t("chats.widgets.config.selector.persona") },
            { value: "model", label: t("chats.widgets.config.selector.model") },
            { value: "author_note", label: t("chats.widgets.config.selector.authorNote") },
          ]}
          onChange={(v) => setNode({ ...node, kind: v })}
        />
      </Field>
    </>
  );
}

function ButtonForm({
  node,
  setNode,
}: {
  node: ButtonNode;
  setNode: (n: ButtonNode) => void;
}) {
  const { t } = useI18n();
  return (
    <>
      <Field label={t("chats.widgets.config.fields.titleOptional")}>
        <input
          type="text"
          className={TEXT_INPUT_CLASS}
          value={node.title ?? ""}
          onChange={(e) => setNode({ ...node, title: e.target.value })}
        />
      </Field>
      <Field label={t("chats.widgets.config.fields.descriptionOptional")}>
        <input
          type="text"
          className={TEXT_INPUT_CLASS}
          value={node.description ?? ""}
          onChange={(e) => setNode({ ...node, description: e.target.value })}
        />
      </Field>
      <Field label={t("chats.widgets.config.button.action")}>
        <Segmented<ButtonAction>
          value={node.action}
          options={[
            { value: "regenerate", label: t("chats.widgets.config.button.regenerate") },
            { value: "continue", label: t("chats.widgets.config.button.continue") },
            { value: "swap_places", label: t("chats.widgets.config.button.swapPlaces") },
            { value: "abort", label: t("chats.widgets.config.button.stop") },
            { value: "new_session", label: t("chats.widgets.config.button.newSession") },
            { value: "view_history", label: t("chats.widgets.config.button.history") },
            { value: "open_memories", label: t("chats.widgets.config.button.memories") },
            { value: "open_search", label: t("chats.widgets.config.button.search") },
            { value: "toggle_voice_autoplay", label: t("chats.widgets.config.button.voice") },
          ]}
          onChange={(v) => setNode({ ...node, action: v })}
        />
      </Field>
    </>
  );
}
