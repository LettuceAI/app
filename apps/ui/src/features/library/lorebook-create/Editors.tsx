import { CreationError } from "./CreationError";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Trash2 } from "lucide-react";
import {
  Button,
  Field,
  icon,
  IconButton,
  Input,
  ListGroup,
  ListRow,
  SelectMenu,
  Sheet,
  Switch,
  TagInput,
  Text,
  Textarea,
} from "@lettuceai/crisp";
import type {
  LorebookDraftView,
  LorebookPlanInput,
  LorebookPlanView,
} from "@/api/generated/bindings";
import { categories, category, type PastedSource } from "../data/lorebook-projects";

function Footer({
  busy,
  ready,
  onClose,
  onSave,
  remove,
}: {
  busy: boolean;
  ready: boolean;
  onClose: () => void;
  onSave: () => void;
  remove?: () => void;
}) {
  const { t } = useTranslation("lorebook-create");
  return (
    <div className="flex items-center justify-end gap-2">
      {remove && (
        <IconButton
          label={t("removePlan")}
          variant="ghost"
          disabled={busy}
          className="mr-auto text-danger"
          onClick={remove}
        >
          <Trash2 size={icon.md} />
        </IconButton>
      )}
      <Button variant="ghost" disabled={busy} onClick={onClose}>
        {t("cancel")}
      </Button>
      <Button variant="primary" loading={busy} disabled={!ready || busy} onClick={onSave}>
        {t("save")}
      </Button>
    </div>
  );
}
export function PasteEditor({
  value,
  phone,
  onClose,
  onSave,
}: {
  value: PastedSource | null;
  phone: boolean;
  onClose: () => void;
  onSave: (value: PastedSource) => void;
}) {
  const { t } = useTranslation("lorebook-create");
  const [label, setLabel] = useState(value?.label ?? "");
  const [text, setText] = useState(value?.text ?? "");
  return (
    <Sheet
      open
      onClose={onClose}
      placement={phone ? "bottom" : "side"}
      size="lg"
      title={t(value ? "pastedText" : "pasteText")}
      footer={
        <Footer
          busy={false}
          ready={Boolean(text.trim())}
          onClose={onClose}
          onSave={() =>
            onSave({
              id: value?.id ?? crypto.randomUUID(),
              label: label.trim() || t("pastedText"),
              text: text.trim(),
            })
          }
        />
      }
    >
      <div className="space-y-5">
        <Field label={t("label")} hint={t("labelHint")}>
          <Input value={label} onChange={(event) => setLabel(event.target.value)} data-autofocus />
        </Field>
        <Field label={t("text")} required hint={t("characters", { count: text.length })}>
          <Textarea
            rows={phone ? 7 : 14}
            value={text}
            onChange={(event) => setText(event.target.value)}
          />
        </Field>
      </div>
    </Sheet>
  );
}
export function PlanEditor({
  plan,
  phone,
  busy,
  onClose,
  onSave,
  onRemove,
  error,
  retry,
}: {
  plan: LorebookPlanView | null;
  phone: boolean;
  busy: boolean;
  onClose: () => void;
  onSave: (value: LorebookPlanInput, initial: LorebookPlanView | null) => void;
  onRemove?: () => void;
  error: unknown;
  retry: () => void;
}) {
  const { t } = useTranslation("lorebook-create");
  const [initial] = useState(plan);
  const [title, setTitle] = useState(plan?.title ?? "");
  const [kind, setKind] = useState(plan ? category(plan.category) : "other");
  const [keys, setKeys] = useState(plan?.proposed_keys ?? []);
  const [rationale, setRationale] = useState(plan?.rationale ?? "");
  return (
    <Sheet
      open
      onClose={onClose}
      dismissible={!busy}
      placement={phone ? "bottom" : "side"}
      size="lg"
      title={t(plan ? "plannedEntry" : "addEntry")}
      description={t("planEditorBody")}
      footer={
        <Footer
          busy={busy}
          ready={Boolean(title.trim())}
          onClose={onClose}
          onSave={() =>
            onSave(
              {
                plan_id: plan?.plan_id ?? null,
                title: title.trim(),
                category: kind,
                proposed_keys: keys,
                rationale: rationale.trim(),
                source_refs: initial?.source_refs ?? [],
              },
              initial,
            )
          }
          {...(onRemove ? { remove: onRemove } : {})}
        />
      }
    >
      <div className="space-y-5">
        <CreationError error={error} busy={busy} retry={retry} />
        <Field label={t("entryTitle")} required>
          <Input value={title} onChange={(event) => setTitle(event.target.value)} data-autofocus />
        </Field>
        <Field label={t("kind")}>
          <SelectMenu
            className="w-full"
            value={kind}
            onChange={(value) => value && setKind(value)}
            options={categories.map((value) => ({ value, label: t(`categories.${value}`) }))}
          />
        </Field>
        <Field label={t("keywords")} hint={t("suggestedKeys")}>
          <TagInput value={keys} onChange={setKeys} placeholder={t("keywordPlaceholder")} />
        </Field>
        <Field label={t("coverage")} hint={t("coverageHint")}>
          <Textarea
            rows={3}
            value={rationale}
            onChange={(event) => setRationale(event.target.value)}
          />
        </Field>
      </div>
    </Sheet>
  );
}
export function DraftEditor({
  draft,
  error,
  retry,
  phone,
  busy,
  onClose,
  onSave,
}: {
  draft: LorebookDraftView;
  error: unknown;
  retry: () => void;
  phone: boolean;
  busy: boolean;
  onClose: () => void;
  onSave: (
    value: Pick<LorebookDraftView, "title" | "keywords" | "content" | "always_active">,
    initial: LorebookDraftView,
  ) => void;
}) {
  const { t } = useTranslation("lorebook-create");
  const [initial] = useState(draft);
  const [title, setTitle] = useState(draft.title);
  const [content, setContent] = useState(draft.content);
  const [keywords, setKeywords] = useState(draft.keywords);
  const [always, setAlways] = useState(draft.always_active);
  return (
    <Sheet
      open
      onClose={onClose}
      dismissible={!busy}
      placement={phone ? "bottom" : "side"}
      size="lg"
      title={t("editEntry")}
      footer={
        <Footer
          busy={busy}
          ready={Boolean(title.trim() && content.trim())}
          onClose={onClose}
          onSave={() =>
            onSave(
              {
                title: title.trim(),
                content: content.trim(),
                keywords,
                always_active: always,
              },
              initial,
            )
          }
        />
      }
    >
      <div className="space-y-5">
        <CreationError error={error} busy={busy} retry={retry} />
        <Field label={t("entryTitle")} required>
          <Input value={title} onChange={(event) => setTitle(event.target.value)} data-autofocus />
        </Field>
        <Field
          label={t("content")}
          required
          hint={t("words", { count: content.trim().split(/\s+/).filter(Boolean).length })}
        >
          <Textarea
            rows={phone ? 6 : 10}
            value={content}
            onChange={(event) => setContent(event.target.value)}
          />
        </Field>
        <ListGroup>
          <ListRow
            title={t("alwaysPrompt")}
            description={t("alwaysPromptBody")}
            trailing={
              <Switch checked={always} onChange={setAlways} aria-label={t("alwaysActive")} />
            }
          />
        </ListGroup>
        {!always && (
          <Field label={t("keywords")} hint={t("keysHint")}>
            <TagInput
              value={keywords}
              onChange={setKeywords}
              placeholder={t("keywordPlaceholder")}
            />
          </Field>
        )}
      </div>
    </Sheet>
  );
}
export function RefineEditor({
  draft,
  error,
  retry,
  phone,
  busy,
  onClose,
  onSave,
}: {
  draft: LorebookDraftView;
  error: unknown;
  retry: () => void;
  phone: boolean;
  busy: boolean;
  onClose: () => void;
  onSave: (feedback: string) => void;
}) {
  const { t } = useTranslation("lorebook-create");
  const [note, setNote] = useState("");
  return (
    <Sheet
      open
      onClose={onClose}
      dismissible={!busy}
      placement={phone ? "bottom" : "side"}
      title={t("changeEntry", { name: draft.title })}
      description={t("refineBody")}
      footer={
        <Footer
          busy={busy}
          ready={Boolean(note.trim())}
          onClose={onClose}
          onSave={() => onSave(note.trim())}
        />
      }
    >
      <CreationError error={error} busy={busy} retry={retry} />
      <Field label={t("feedback")}>
        <Textarea
          rows={5}
          value={note}
          onChange={(event) => setNote(event.target.value)}
          data-autofocus
        />
      </Field>
      {draft.revisions.length > 0 && (
        <Text variant="hint">{t("revisionCount", { count: draft.revisions.length })}</Text>
      )}
    </Sheet>
  );
}
