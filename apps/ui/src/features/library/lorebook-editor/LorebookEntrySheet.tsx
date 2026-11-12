import { useEffect, useRef, useState } from "react";
import { useBlocker } from "@tanstack/react-router";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { AnimatePresence, motion } from "framer-motion";
import { Sparkles, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import {
  Button,
  Chip,
  Field,
  icon,
  IconButton,
  Input,
  ListGroup,
  ListRow,
  motion as m,
  Segmented,
  Sheet,
  Switch,
  TagInput,
  Textarea,
  toast,
} from "@lettuceai/crisp";
import type {
  LorebookEntriesMutateRequest,
  LorebookEntryInput_Serialize,
  LorebookSecondaryLogic,
  LorebookEntryView,
  LorebookView_Serialize,
} from "@/api/generated/bindings";
import { useApi } from "@/api/use-api";
import { isApiFailure } from "@/api/client";
import { queryKeys } from "@/api/query-keys";
import { watchJob } from "@/entities/job";
import { FailureState } from "@/shared/ui/FailureState";
import { lorebookBookKey } from "./data";
import { splitTags } from "@/shared/formatting/splitTags";
import { PatternEditor } from "./PatternEditor";
import { SelectMenuField } from "@/shared/ui/SelectMenuField";

const secondaryLogics: readonly LorebookSecondaryLogic[] = ["and_any", "and_all", "not_any", "not_all"];

const blank: LorebookEntryInput_Serialize = {
  title: "",
  enabled: true,
  always_active: false,
  keywords: [],
  secondary_keywords: [],
  secondary_logic: "and_any",
  case_sensitive: false,
  keyword_mode: "literal",
  content: "",
  priority: 0,
};

export function LorebookEntrySheet({
  book,
  entry,
  open,
  phone,
  onClose,
  onSaved,
  onDelete,
  beforeWrite,
  ownRevision,
}: {
  book: LorebookView_Serialize;
  entry: LorebookEntryView | null;
  open: boolean;
  phone: boolean;
  onClose: () => void;
  onSaved?: (view: LorebookView_Serialize) => void;
  onDelete?: () => void;
  beforeWrite?: () => Promise<unknown>;
  ownRevision?: () => number;
}) {
  const { t } = useTranslation("lorebook-editor");
  const { t: common } = useTranslation();
  const { t: creation } = useTranslation("lorebook-create");
  const api = useApi();
  const client = useQueryClient();
  const bookKey = lorebookBookKey(book.lorebook.id);
  const [draft, setDraft] = useState<LorebookEntryInput_Serialize>(() =>
    entry
      ? {
          title: entry.title,
          enabled: entry.enabled,
          always_active: entry.always_active,
          keywords: entry.keywords,
          secondary_keywords: entry.secondary_keywords,
          secondary_logic: entry.secondary_logic,
          case_sensitive: entry.case_sensitive,
          keyword_mode: entry.keyword_mode,
          content: entry.content,
          priority: entry.priority,
        }
      : blank,
  );
  const pending = useRef<LorebookEntriesMutateRequest | null>(null);
  const [revision, setRevision] = useState(book.lorebook.revision);
  const update = (patch: Partial<LorebookEntryInput_Serialize>) => {
    pending.current = null;
    mutation.reset();
    setDraft((value) => ({ ...value, ...patch }));
  };
  const mutation = useMutation({
    mutationFn: async () => {
      if (!pending.current) {
        await beforeWrite?.();
        pending.current = {
          lorebook_id: book.lorebook.id,
          expected_revision: Math.max(revision, ownRevision?.() ?? 0),
          client_operation_id: crypto.randomUUID(),
          mutations: [
            entry
              ? { type: "update", entry_id: entry.id, entry: structuredClone(draft) }
              : { type: "add", entry: structuredClone(draft), index: null },
          ],
        };
      }
      return api.call("lorebookEntriesMutate", pending.current);
    },
    onSuccess: async (value) => {
      client.setQueryData(bookKey, value);
      await client.invalidateQueries({ queryKey: queryKeys.lorebooks.all });
      onSaved?.(value);
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
      client.setQueryData(bookKey, value);
    },
  });
  const [suggesting, setSuggesting] = useState(false);
  const suggestion = useRef<{ abort: AbortController; jobId: string | null } | null>(null);
  const cancelSuggestion = () => {
    const current = suggestion.current;
    suggestion.current = null;
    setSuggesting(false);
    current?.abort.abort();
    if (current?.jobId)
      void api.call("jobCancel", { job_id: current.jobId }).catch(() => undefined);
  };
  const cancelRef = useRef(cancelSuggestion);
  useEffect(() => {
    cancelRef.current = cancelSuggestion;
  });
  useEffect(() => () => cancelRef.current(), []);
  const suggest = async () => {
    if (suggestion.current) return;
    const run = { abort: new AbortController(), jobId: null as string | null };
    suggestion.current = run;
    setSuggesting(true);
    try {
      const accepted = await api.call("lorebookKeywordsDraft", {
        client_operation_id: crypto.randomUUID(),
        lorebook_id: book.lorebook.id,
        entry_id: entry?.id ?? null,
        title: draft.title.trim() || null,
        content: draft.content.trim(),
        existing_keywords: draft.keywords,
        direction: null,
      });
      run.jobId = accepted.job_id;
      if (run.abort.signal.aborted) {
        void api.call("jobCancel", { job_id: accepted.job_id }).catch(() => undefined);
        return;
      }
      const job = await watchJob(api, accepted.job_id, () => undefined, run.abort.signal);
      if (job.state === "succeeded" && job.result?.type === "lorebook_keywords") {
        const have = new Set(draft.keywords.map((keyword) => keyword.toLowerCase()));
        const fresh = job.result.keywords.filter((keyword) => {
          const key = keyword.trim().toLowerCase();
          if (!key || have.has(key)) return false;
          have.add(key);
          return true;
        });
        if (fresh.length === 0) toast.warning(t("suggestNone"));
        else update({ keywords: [...draft.keywords, ...fresh] });
      } else if (job.state !== "succeeded") {
        const fallback = job.state === "cancelled" ? "cancelled" : "unknown";
        toast.error(creation(`jobErrors.${job.failure?.code ?? fallback}`));
      }
    } catch (error) {
      if (!run.abort.signal.aborted)
        toast.error(common(`errors.code.${isApiFailure(error) ? error.code : "internal"}`));
    } finally {
      if (suggestion.current === run) {
        suggestion.current = null;
        setSuggesting(false);
      }
    }
  };
  const busy = mutation.isPending || refresh.isPending;
  const close = () => {
    cancelSuggestion();
    onClose();
  };
  useBlocker({
    shouldBlockFn: ({ action }) => {
      if (!open) return false;
      if (action === "BACK") {
        if (!busy) close();
        return true;
      }
      return busy;
    },
    enableBeforeUnload: open && busy,
  });
  const words = draft.content.trim().split(/\s+/).filter(Boolean).length;
  return (
    <Sheet
      open={open}
      onClose={close}
      dismissible={!busy}
      placement={phone ? "bottom" : "side"}
      size="lg"
      title={t(entry ? "editTitle" : "newTitle")}
      footer={
        <div className="flex items-center gap-2">
          {entry && onDelete && (
            <IconButton label={t("deleteEntry")} variant="ghost" disabled={busy} onClick={onDelete}>
              <Trash2 size={icon.md} className="text-danger" />
            </IconButton>
          )}
          <span className="flex-1" />
          <Button variant="ghost" disabled={busy} onClick={close}>
            {t("cancel")}
          </Button>
          <Button
            variant="primary"
            loading={mutation.isPending}
            disabled={busy || !draft.content.trim()}
            onClick={() => {
              cancelSuggestion();
              mutation.mutate();
            }}
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
        <Field label={t("contentField")} required hint={t("contentHint", { count: words })}>
          <Textarea
            rows={phone ? 9 : 12}
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
        <AnimatePresence initial={false}>
          {!draft.always_active && (
            <motion.div
              key="keywords"
              initial={{ opacity: 0, height: 0 }}
              animate={{ opacity: 1, height: "auto" }}
              exit={{ opacity: 0, height: 0 }}
              transition={m.settle}
              className="space-y-4 overflow-hidden"
            >
              <Segmented
                block
                aria-label={t("matchBy")}
                value={draft.keyword_mode}
                onChange={(keyword_mode) => update({ keyword_mode })}
                options={[
                  { value: "literal", label: t("modeKeywords") },
                  { value: "regex", label: t("modeRegex") },
                ]}
              />
              <Field
                label={
                  <span className="flex items-center justify-between">
                    <span>{t(draft.keyword_mode === "regex" ? "patterns" : "keywords")}</span>
                    {draft.keyword_mode === "literal" && (
                      <Button
                        size="sm"
                        variant="ghost"
                        loading={suggesting}
                        leading={<Sparkles size={icon.xs} />}
                        disabled={busy || !draft.content.trim()}
                        onClick={() => void suggest()}
                      >
                        {t("suggest")}
                      </Button>
                    )}
                  </span>
                }
                hint={t(draft.keyword_mode === "regex" ? "patternsHint" : "keywordsHint")}
              >
                {draft.keyword_mode === "regex" ? (
                  <PatternEditor
                    key="primary"
                    rowLabel={(number) => t("patternLabel", { number })}
                    addLabel={t("addPattern")}
                    value={draft.keywords}
                    caseSensitive={draft.case_sensitive}
                    disabled={busy}
                    onChange={(keywords) => update({ keywords })}
                  />
                ) : (
                  <div className="space-y-1.5">
                    <TagInput
                      value={draft.keywords}
                      disabled={busy || suggesting}
                      onChange={(keywords) => update({ keywords: splitTags(keywords) })}
                      placeholder={t("keywordPlaceholder")}
                    />
                    <p className="text-xs text-fg-3">{t("keywordSyntax")}</p>
                  </div>
                )}
              </Field>
              <Field
                label={t("secondary")}
                hint={t(`secondaryLogic.${draft.secondary_logic}Hint`)}
              >
                <div className="space-y-2">
                  <SelectMenuField
                    aria-label={t("secondaryLogicLabel")}
                    value={draft.secondary_logic}
                    disabled={busy}
                    onChange={(value) => value && update({ secondary_logic: value as LorebookSecondaryLogic })}
                    options={secondaryLogics.map((logic) => ({
                      value: logic,
                      label: t(`secondaryLogic.${logic}`),
                      description: t(`secondaryLogic.${logic}Hint`),
                    }))}
                  />
                  {draft.keyword_mode === "regex" ? (
                    <PatternEditor
                      key="secondary"
                      rowLabel={(number) => t("secondaryPatternLabel", { number })}
                      addLabel={t("addSecondaryPattern")}
                      tryable={false}
                      value={draft.secondary_keywords}
                      caseSensitive={draft.case_sensitive}
                      disabled={busy}
                      onChange={(secondary_keywords) => update({ secondary_keywords })}
                    />
                  ) : (
                    <TagInput
                      value={draft.secondary_keywords}
                      disabled={busy}
                      onChange={(secondary_keywords) => update({ secondary_keywords: splitTags(secondary_keywords) })}
                      placeholder={t("secondaryPlaceholder")}
                    />
                  )}
                </div>
              </Field>
              <div className="flex flex-wrap gap-2">
                <Chip
                  selected={draft.case_sensitive}
                  disabled={busy || suggesting}
                  onClick={() => update({ case_sensitive: !draft.case_sensitive })}
                >
                  {t("caseSensitive")}
                </Chip>
              </div>
            </motion.div>
          )}
        </AnimatePresence>
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
