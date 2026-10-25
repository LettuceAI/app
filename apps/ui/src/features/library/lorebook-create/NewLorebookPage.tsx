import { useCallback, useEffect, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useLocation, useNavigate, useRouter, useSearch } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import {
  ArrowRight,
  BookOpen,
  ClipboardPaste,
  FilePlus2,
  FileText,
  Trash2,
  Upload,
  User,
  UserCircle,
  Wand2,
} from "lucide-react";
import {
  ActionBar,
  Avatar,
  Button,
  ChoiceCard,
  CrispStringsProvider,
  Field,
  icon,
  IconButton,
  Input,
  ListGroup,
  ListRow,
  ListRowButton,
  Page,
  Skeleton,
  Slider,
  Stepper,
  Text,
  Textarea,
} from "@lettuceai/crisp";
import { ApiFailure } from "@/api/client";
import type {
  FileSource,
  ImportPreview_Serialize,
  LorebookProjectSourceInput,
} from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { ImportSheet } from "@/features/opened-files";
import { useApi } from "@/api/use-api";
import { usePageLayout } from "@/shared/hooks/usePageLayout";
import { FailureState } from "@/shared/ui/FailureState";
import { SliderValue } from "@/shared/ui/SliderValue";
import { SettingsControlRow } from "@/shared/ui/SettingsControlRow";
import { refreshLibrary } from "../data/actions";
import {
  briefDraftKey,
  briefSources,
  emptyBrief,
  projectKey,
  sourceBytes,
  totalSourceLimit,
  type BriefDraft,
  type DocumentSource,
} from "../data/lorebook-projects";
import { PasteEditor } from "./Editors";
import { SourceDocuments } from "./SourceDocuments";
import { SourcePicker } from "./SourcePicker";
import { ModelRow } from "./ModelRow";
import { generatorDefaultsQuery } from "@/entities/lorebook";
import { CreationError } from "./CreationError";

export function NewLorebookPage() {
  const { t, i18n } = useTranslation("lorebook-create");
  const api = useApi();
  const client = useQueryClient();
  const navigate = useNavigate();
  const router = useRouter();
  const { phone } = usePageLayout();
  const search = useSearch({ from: "/library/lorebooks/new" });
  const flowPushed = useLocation({ select: (location) => location.state.lorebookFlowPushed });
  const pushed = useLocation({ select: (location) => location.state.lorebookPanelPushed });
  const [draft, setDraft] = useState<BriefDraft>(
    () => client.getQueryData<BriefDraft>(briefDraftKey) ?? emptyBrief(),
  );
  const draftValue = useRef(draft);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<unknown>(null);
  const [importing, setImporting] = useState<{
    source: FileSource;
    preview: ImportPreview_Serialize;
  } | null>(null);
  const running = useRef(false);
  const alive = useRef(true);
  const retry = useRef<() => void>(() => {});
  const [filesBusy, setFilesBusy] = useState(false);
  const defaults = useQuery(generatorDefaultsQuery(api));
  const update = useCallback(
    (patch: Partial<BriefDraft> | ((current: BriefDraft) => BriefDraft)) => {
      if (!alive.current) return;
      const current = draftValue.current;
      const next = typeof patch === "function" ? patch(current) : { ...current, ...patch };
      draftValue.current = next;
      setDraft(next);
      client.setQueryData(briefDraftKey, next);
    },
    [client],
  );
  useEffect(() => {
    alive.current = true;
    let stop: (() => void) | undefined;
    let disposed = false;
    void api
      .subscribe((event) => {
        if (event.type === "database_reloaded") {
          alive.current = false;
          client.removeQueries({ queryKey: briefDraftKey });
          client.removeQueries({ queryKey: ["lorebook-create-operation"] });
          void navigate({ to: "/library", replace: true });
        }
      })
      .then((unsubscribe) => {
        if (disposed) unsubscribe();
        else stop = unsubscribe;
      })
      .catch((failure: unknown) => {
        if (alive.current) setError(failure);
      });
    return () => {
      disposed = true;
      alive.current = false;
      stop?.();
    };
  }, [api, client, navigate]);
  const character = useQuery({
    queryKey: [...queryKeys.characters.all, "lorebook-source-detail", draft.characterId],
    queryFn: () => api.call("characterGet", { character_id: draft.characterId! }),
    enabled: Boolean(draft.characterId),
  });
  const persona = useQuery({
    queryKey: [...queryKeys.personas.all, "lorebook-source-detail", draft.personaId],
    queryFn: () => api.call("personaGet", { persona_id: draft.personaId! }),
    enabled: Boolean(draft.personaId),
  });
  const chooseSaved = async (id: string) => {
    await refreshLibrary(client);
    if (!alive.current) return;
    client.removeQueries({ queryKey: briefDraftKey });
    client.removeQueries({ queryKey: ["lorebook-create-operation"] });
    void navigate({
      to: "/library",
      search: { kind: "lorebook", item: id },
      state: { libraryKind: "lorebook" },
      replace: true,
    });
  };
  const operation = (request: unknown) => {
    const key = ["lorebook-create-operation", JSON.stringify(request)];
    let id = client.getQueryData<string>(key);
    if (!id) {
      id = crypto.randomUUID();
      client.setQueryData(key, id);
    }
    return id;
  };
  const submit = async () => {
    if (running.current || filesBusy) return;
    running.current = true;
    setBusy(true);
    setError(null);
    retry.current = () => void submit();
    try {
      if (draft.mode === "empty") {
        const payload = {
          metadata: {
            name: draft.name.trim(),
            detection: "recent_messages" as const,
            icon_asset_id: null,
          },
          entries: [],
        };
        const result = await api.call("lorebookCreate", {
          ...payload,
          client_operation_id: operation(payload),
        });
        await chooseSaved(result.lorebook.id);
        return;
      }
      if (draft.mode === "import") {
        const files = await api.call("filesPickOpen", {
          kinds: ["json", "character_card"],
          multiple: false,
        });
        const source = files.sources[0];
        if (!source || !alive.current) return;
        const preview = await api.call("importPreview", { source });
        if (preview.content.kind !== "lorebook")
          throw new ApiFailure("unsupported", "not a lorebook import", null);
        if (alive.current) setImporting({ source, preview });
        return;
      }
      if (draft.step === "start") {
        update({ step: "brief" });
        return;
      }
      const sources: LorebookProjectSourceInput[] = briefSources(draft);
      if (draft.characterId) {
        const value = await api.call("characterGet", { character_id: draft.characterId });
        const text = [value.draft.profile.description, value.draft.profile.definition]
          .filter(Boolean)
          .join("\n\n")
          .trim();
        if (text)
          sources.unshift({
            type: "text",
            label: t("characterSource", { name: value.draft.profile.name }),
            text,
          });
      }
      if (draft.personaId) {
        const value = await api.call("personaGet", { persona_id: draft.personaId });
        const text = value.draft.description.trim();
        if (text)
          sources.splice(draft.characterId ? 1 : 0, 0, {
            type: "text",
            label: t("personaSource", { name: value.draft.title }),
            text,
          });
      }
      if (!alive.current) return;
      const payload = {
        brief: draft.brief.trim(),
        lorebook_name: draft.name.trim(),
        target_count: draft.target ?? defaults.data?.target_count ?? null,
        sources,
      };
      const created = await api.call("lorebookProjectCreate", {
        ...payload,
        client_operation_id: operation(payload),
      });
      if (!alive.current) return;
      client.setQueryData(projectKey(created.project_id), created);
      void client.invalidateQueries({ queryKey: queryKeys.lorebookProjects.all });
      let current = await api.call("lorebookProjectGet", { project_id: created.project_id });
      if (current.stage === "created" || current.stage === "plan_failed") {
        const plan = { project_id: current.project_id, expected_revision: current.revision };
        try {
          await api.call("lorebookProjectPlan", { ...plan, client_operation_id: operation(plan) });
        } catch (failure) {
          if (ApiFailure.fromRejection(failure).code !== "conflict") throw failure;
          current = await api.call("lorebookProjectGet", { project_id: created.project_id });
          if (current.stage === "created" || current.stage === "plan_failed") {
            const next = { project_id: current.project_id, expected_revision: current.revision };
            await api.call("lorebookProjectPlan", {
              ...next,
              client_operation_id: operation(next),
            });
          }
        }
      }
      if (!alive.current) return;
      client.setQueryData(projectKey(created.project_id), current);
      client.removeQueries({ queryKey: briefDraftKey });
      client.removeQueries({ queryKey: ["lorebook-create-operation"] });
      await client.invalidateQueries({ queryKey: queryKeys.lorebookProjects.all });
      void navigate({
        to: "/library/lorebooks/projects/$projectId",
        params: { projectId: created.project_id },
        state: { lorebookFlowPushed: flowPushed ?? false },
        replace: true,
      });
    } catch (failure) {
      if (alive.current) setError(failure);
    } finally {
      running.current = false;
      if (alive.current) setBusy(false);
    }
  };
  const open = (panel: "character" | "persona" | "paste", source?: string) =>
    void navigate({
      to: "/library/lorebooks/new",
      search: { panel, ...(source ? { source } : {}) },
      state: { lorebookPanelPushed: true, lorebookFlowPushed: flowPushed ?? false },
      resetScroll: false,
    });
  const close = () => {
    if (pushed) router.history.back();
    else
      void navigate({
        to: "/library/lorebooks/new",
        search: {},
        replace: true,
        resetScroll: false,
      });
  };
  const bytes = sourceBytes(draft.texts, draft.documents);
  const onDocuments = useCallback(
    (documents: DocumentSource[]) =>
      update((current) => ({
        ...current,
        documents: [
          ...current.documents,
          ...documents.filter((item) => !current.documents.some((other) => other.id === item.id)),
        ],
      })),
    [update],
  );
  const steps = ["brief", "outline", "entries", "save"] as const;
  const actions = (
    <ActionBar
      pinned={phone}
      className={phone ? "" : "px-8 pb-6 pt-2"}
      primary={
        <Button
          variant="primary"
          className={phone ? "flex-1" : undefined}
          loading={busy}
          disabled={
            busy ||
            filesBusy ||
            !draft.name.trim() ||
            (draft.step === "brief" &&
              (!draft.brief.trim() || !defaults.data || bytes > totalSourceLimit))
          }
          leading={
            draft.mode === "import" ? (
              <Upload size={icon.md} />
            ) : draft.step === "brief" ? (
              <Wand2 size={icon.md} />
            ) : undefined
          }
          trailing={
            draft.mode === "ai" && draft.step === "start" ? (
              <ArrowRight size={icon.md} />
            ) : undefined
          }
          onClick={() => void submit()}
        >
          {t(
            draft.mode === "import"
              ? "chooseFile"
              : draft.mode === "empty"
                ? "createLorebook"
                : draft.step === "start"
                  ? "continue"
                  : "planEntries",
          )}
        </Button>
      }
    />
  );
  return (
    <CrispStringsProvider
      locale={i18n.language}
      strings={{
        sheetClose: t("close"),
        stepperProgress: (current, count) => t("stepProgress", { current, count }),
        stepperBackTo: (label) => t("stepBack", { label }),
        numberInputIncrease: t("increase"),
        numberInputDecrease: t("decrease"),
        selectMenuPlaceholder: t("choose"),
        selectMenuEmpty: t("noOptions"),
        dropzoneIdle: t("dropFiles"),
        dropzoneOver: t("dropHere"),
        searchFieldClear: t("clearSearch"),
        tagInputRemove: (tag) => t("removeNamed", { name: tag }),
      }}
    >
      <Page
        title={draft.step === "start" ? t("newLorebook") : draft.name.trim() || t("newLorebook")}
        meta={draft.step === "brief" && !phone ? t("writtenWithAi") : undefined}
        onBack={() => {
          if (draft.step === "brief") update({ step: "start" });
          else if (flowPushed) router.history.back();
          else void navigate({ to: "/library", replace: true });
        }}
        backLabel={t(draft.step === "brief" ? "newLorebook" : "library")}
        contentClassName="max-w-3xl space-y-7"
        footer={actions}
      >
        {draft.step === "brief" && (
          <Stepper steps={steps.map((id) => ({ id, label: t(`steps.${id}`) }))} current={0} />
        )}
        <Field label={t("name")} required>
          <Input
            value={draft.name}
            disabled={busy}
            onChange={(event) => update({ name: event.target.value })}
            placeholder={t("namePlaceholder")}
            data-autofocus
          />
        </Field>
        {draft.step === "start" ? (
          <section className="space-y-2">
            <Text variant="label">{t("howToStart")}</Text>
            <div className="grid gap-2">
              {(["empty", "ai", "import"] as const).map((mode) => (
                <ChoiceCard
                  key={mode}
                  selected={draft.mode === mode}
                  onClick={() => update({ mode })}
                  icon={
                    mode === "empty" ? (
                      <FilePlus2 size={icon.lg} />
                    ) : mode === "ai" ? (
                      <Wand2 size={icon.lg} />
                    ) : (
                      <Upload size={icon.lg} />
                    )
                  }
                  title={t(`start.${mode}`)}
                  description={t(`start.${mode}Body`)}
                />
              ))}
            </div>
          </section>
        ) : (
          <>
            <Field label={t("about")} required hint={t("aboutHint")}>
              <Textarea
                rows={phone ? 5 : 6}
                value={draft.brief}
                disabled={busy}
                onChange={(event) => update({ brief: event.target.value })}
                placeholder={t("aboutPlaceholder")}
              />
            </Field>
            <section className="space-y-2">
              <ListGroup label={t("fromLibrary")}>
                {(["character", "persona"] as const).map((kind) => {
                  const data = kind === "character" ? character.data : persona.data;
                  const avatarId = data?.draft.media.find(
                    (link) => link.slot === "avatar_original" || link.slot === "avatar",
                  )?.asset_id;
                  const avatar = data?.assets.find((asset) => asset.asset_id === avatarId);
                  const selected = kind === "character" ? draft.characterId : draft.personaId;
                  const name =
                    kind === "character"
                      ? character.data?.draft.profile.name
                      : persona.data?.draft.title;
                  const pending =
                    selected && (kind === "character" ? character.isPending : persona.isPending);
                  return (
                    <ListRowButton
                      key={kind}
                      icon={
                        data ? (
                          <Avatar
                            src={avatar?.preview_url ?? avatar?.url ?? null}
                            name={name ?? ""}
                            size="sm"
                          />
                        ) : kind === "character" ? (
                          <User size={icon.lg} />
                        ) : (
                          <UserCircle size={icon.lg} />
                        )
                      }
                      title={t(kind)}
                      description={
                        pending ? (
                          <Skeleton className="h-4 w-28" />
                        ) : (
                          (name ?? t(selected ? "sourceMissing" : "none"))
                        )
                      }
                      disabled={busy}
                      onClick={() => open(kind)}
                    />
                  );
                })}
              </ListGroup>
              <Text variant="hint" className="px-1">
                {t("librarySourceHint")}
              </Text>
              {[character, persona].map(
                (query, index) =>
                  query.isError && (
                    <FailureState
                      key={index}
                      error={query.error}
                      onRetry={() => void query.refetch()}
                      size="sm"
                    />
                  ),
              )}
            </section>
            <section className="space-y-2">
              <ListGroup label={t("notesAndFiles")}>
                {draft.texts.map((source) => (
                  <ListRowButton
                    key={source.id}
                    icon={<FileText size={icon.lg} />}
                    title={source.label}
                    description={t("pastedCharacters", { count: source.text.length })}
                    chevron={false}
                    trailing={
                      <IconButton
                        label={t("removeNamed", { name: source.label })}
                        size="sm"
                        variant="ghost"
                        disabled={busy}
                        onClick={(event) => {
                          event.stopPropagation();
                          update((current) => ({
                            ...current,
                            texts: current.texts.filter((item) => item.id !== source.id),
                          }));
                        }}
                      >
                        <Trash2 size={icon.md} />
                      </IconButton>
                    }
                    onClick={() => open("paste", source.id)}
                  />
                ))}
                {draft.documents.map((source) => (
                  <ListRow
                    key={source.id}
                    icon={<FileText size={icon.lg} />}
                    title={source.label}
                    trailing={
                      <IconButton
                        label={t("removeNamed", { name: source.label })}
                        size="sm"
                        variant="ghost"
                        disabled={busy}
                        onClick={() =>
                          update((current) => ({
                            ...current,
                            documents: current.documents.filter((item) => item.id !== source.id),
                          }))
                        }
                      >
                        <Trash2 size={icon.md} />
                      </IconButton>
                    }
                  />
                ))}
                <ListRowButton
                  icon={<ClipboardPaste size={icon.lg} />}
                  title={t("pasteText")}
                  chevron={false}
                  disabled={busy}
                  onClick={() => open("paste")}
                />
              </ListGroup>
              <SourceDocuments
                bytes={bytes}
                disabled={busy}
                onAdd={onDocuments}
                onBusy={setFilesBusy}
                showSize={!draft.documents.some((document) => document.size === null)}
              />
            </section>
            <ListGroup label={t("size")}>
              <SettingsControlRow
                phone={phone}
                icon={<BookOpen size={icon.lg} />}
                title={t("entriesToPlan")}
                description={t("entriesTargetHint")}
                control={
                  defaults.data ? (
                    <div className={`flex items-center gap-3 ${phone ? "w-full" : "w-64"}`}>
                      <Slider
                        value={draft.target ?? defaults.data.target_count}
                        min={defaults.data.min_target_count}
                        max={defaults.data.max_target_count}
                        onChange={(target) => update({ target })}
                        className="min-w-0 flex-1"
                        aria-label={t("entriesToPlan")}
                      />
                      <SliderValue
                        value={draft.target ?? defaults.data.target_count}
                        text={String(draft.target ?? defaults.data.target_count)}
                        overridden
                        min={defaults.data.min_target_count}
                        max={defaults.data.max_target_count}
                        step={1}
                        label={t("entriesToPlan")}
                        onCommit={(target) => update({ target })}
                      />
                    </div>
                  ) : (
                    <Skeleton className={`h-8 ${phone ? "w-full" : "w-64"}`} />
                  )
                }
              />
            </ListGroup>
            {defaults.isError && (
              <FailureState error={defaults.error} onRetry={() => void defaults.refetch()} />
            )}
            <ModelRow />
          </>
        )}
        <CreationError error={error} busy={busy} retry={() => retry.current()} />
        {search.panel === "character" || search.panel === "persona" ? (
          <SourcePicker
            key={search.panel}
            kind={search.panel}
            current={search.panel === "character" ? draft.characterId : draft.personaId}
            onClose={close}
            onChoose={(id) => {
              update(search.panel === "character" ? { characterId: id } : { personaId: id });
              close();
            }}
          />
        ) : search.panel === "paste" ? (
          <PasteEditor
            key={search.source ?? "new"}
            value={draft.texts.find((value) => value.id === search.source) ?? null}
            phone={phone}
            onClose={close}
            onSave={(source) => {
              update((current) => ({
                ...current,
                texts: current.texts.some((item) => item.id === source.id)
                  ? current.texts.map((item) => (item.id === source.id ? source : item))
                  : [...current.texts, source],
              }));
              close();
            }}
          />
        ) : null}
        {importing && (
          <ImportSheet
            api={api}
            {...importing}
            onClose={() => setImporting(null)}
            onImported={(result) => {
              if (result.kind === "imported") void chooseSaved(result.id);
            }}
          />
        )}
      </Page>
    </CrispStringsProvider>
  );
}
