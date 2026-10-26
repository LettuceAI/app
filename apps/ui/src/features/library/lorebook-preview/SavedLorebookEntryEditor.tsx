import { useRef, useState } from "react";
import { useBlocker } from "@tanstack/react-router";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import {
  Button,
  Chip,
  Field,
  Input,
  ListGroup,
  ListRow,
  Sheet,
  Switch,
  TagInput,
  Textarea,
} from "@lettuceai/crisp";
import type {
  LorebookEntriesMutateRequest,
  LorebookEntryInput,
  LorebookEntryView,
  LorebookView_Serialize,
} from "@/api/generated/bindings";
import { useApi } from "@/api/use-api";
import { isApiFailure } from "@/api/client";
import { queryKeys } from "@/api/query-keys";
import { FailureState } from "@/shared/ui/FailureState";

export function SavedLorebookEntryEditor({
  book,
  entry,
  open,
  phone,
  onClose,
  onSaved,
}: {
  book: LorebookView_Serialize;
  entry: LorebookEntryView;
  open: boolean;
  phone: boolean;
  onClose: () => void;
  onSaved: () => void;
}) {
  const { t } = useTranslation("lorebook-preview");
  const api = useApi();
  const client = useQueryClient();
  const [draft, setDraft] = useState<LorebookEntryInput>(() => ({
    title: entry.title,
    enabled: entry.enabled,
    always_active: entry.always_active,
    keywords: entry.keywords,
    case_sensitive: entry.case_sensitive,
    keyword_mode: entry.keyword_mode,
    content: entry.content,
    priority: entry.priority,
  }));
  const pending = useRef<LorebookEntriesMutateRequest | null>(null);
  const [revision, setRevision] = useState(book.lorebook.revision);
  const update = (patch: Partial<LorebookEntryInput>) => {
    pending.current = null;
    mutation.reset();
    setDraft((value) => ({ ...value, ...patch }));
  };
  const mutation = useMutation({
    mutationFn: async () => {
      pending.current ??= {
        lorebook_id: book.lorebook.id,
        expected_revision: revision,
        client_operation_id: crypto.randomUUID(),
        mutations: [{ type: "update", entry_id: entry.id, entry: structuredClone(draft) }],
      };
      return api.call("lorebookEntriesMutate", pending.current);
    },
    onSuccess: async (value) => {
      client.setQueryData(
        [...queryKeys.lorebooks.all, "detail", book.lorebook.id, "preview"],
        value,
      );
      await client.invalidateQueries({ queryKey: queryKeys.lorebooks.all });
      onSaved();
      onClose();
    },
  });
  const conflict = isApiFailure(mutation.error) && mutation.error.code === "conflict";
  const refresh = useMutation({
    mutationFn: () => api.call("lorebookGet", { lorebook_id: book.lorebook.id }),
    onSuccess: (value) => {
      setRevision(value.lorebook.revision);
      pending.current = null;
      mutation.reset();
      client.setQueryData(
        [...queryKeys.lorebooks.all, "detail", book.lorebook.id, "preview"],
        value,
      );
    },
  });
  const busy = mutation.isPending || refresh.isPending;
  useBlocker({
    shouldBlockFn: ({ action }) => {
      if (!open) return false;
      if (action === "BACK") {
        if (!busy) onClose();
        return true;
      }
      return busy;
    },
    enableBeforeUnload: open && busy,
  });
  return (
    <Sheet
      open={open}
      onClose={onClose}
      dismissible={!busy}
      placement={phone ? "bottom" : "side"}
      size="lg"
      title={t("editorTitle")}
      footer={
        <div className="flex items-center justify-end gap-2">
          <Button variant="ghost" disabled={busy} onClick={onClose}>
            {t("cancel")}
          </Button>
          <Button
            variant="primary"
            loading={mutation.isPending}
            disabled={busy || !draft.content.trim()}
            onClick={() => mutation.mutate()}
          >
            {t("save")}
          </Button>
        </div>
      }
    >
      <div className="space-y-5">
        <Field label={t("titleField")} hint={t("titleHint")}>
          <Input
            data-autofocus
            value={draft.title}
            disabled={busy}
            onChange={(event) => update({ title: event.target.value })}
          />
        </Field>
        <Field label={t("contentField")} required hint={t("contentHint")}>
          <Textarea
            rows={phone ? 5 : 8}
            value={draft.content}
            disabled={busy}
            onChange={(event) => update({ content: event.target.value })}
          />
        </Field>
        <ListGroup>
          <ListRow
            title={t("enabled")}
            trailing={
              <Switch
                aria-label={t("enabled")}
                checked={draft.enabled}
                disabled={busy}
                onChange={(enabled) => update({ enabled })}
              />
            }
          />
          <ListRow
            title={t("always")}
            description={t("alwaysHint")}
            trailing={
              <Switch
                aria-label={t("always")}
                checked={draft.always_active}
                disabled={busy}
                onChange={(always_active) => update({ always_active })}
              />
            }
          />
        </ListGroup>
        {!draft.always_active && (
          <div className="space-y-4">
            <Field label={t("keywords")} hint={t("keywordsHint")}>
              <TagInput
                separators={draft.keyword_mode === "regex" ? [] : [","]}
                value={draft.keywords}
                disabled={busy}
                onChange={(keywords) => update({ keywords })}
                placeholder={t("keywordPlaceholder")}
              />
            </Field>
            <div className="flex flex-wrap gap-2">
              <Chip
                selected={draft.keyword_mode === "regex"}
                disabled={busy}
                onClick={() =>
                  update({ keyword_mode: draft.keyword_mode === "regex" ? "literal" : "regex" })
                }
              >
                {t("regex")}
              </Chip>
              <Chip
                selected={draft.case_sensitive}
                disabled={busy}
                onClick={() => update({ case_sensitive: !draft.case_sensitive })}
              >
                {t("caseSensitive")}
              </Chip>
            </div>
          </div>
        )}
        {conflict ? (
          <div role="alert" className="space-y-2">
            <p className="text-sm text-danger">{t("conflict")}</p>
            <Button
              variant="secondary"
              loading={refresh.isPending}
              onClick={() => refresh.mutate()}
            >
              {t("refreshRevision")}
            </Button>
          </div>
        ) : (
          mutation.isError && (
            <FailureState error={mutation.error} onRetry={() => mutation.mutate()} size="sm" />
          )
        )}
        {refresh.isError && (
          <FailureState error={refresh.error} onRetry={() => refresh.mutate()} size="sm" />
        )}
      </div>
    </Sheet>
  );
}
