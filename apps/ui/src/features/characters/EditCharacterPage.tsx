import { useLayoutEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useBlocker, useCanGoBack, useNavigate, useParams, useRouteContext, useRouter, useSearch } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { confirm, LoadingState, motion as m, Page, Tab, TabList, Tabs, toast, useCompactLayout } from "@lettuceai/crisp";
import { appStatusQuery } from "@/api/app-status-query";
import { isApiFailure } from "@/api/client";
import type { CharacterView_Serialize } from "@/api/generated/bindings";
import { startDirectChat } from "@/features/conversations";
import { exportEntry, refreshLibrary } from "@/features/library";
import { EditSaveBar } from "@/shared/ui/EditSaveBar";
import { FailureState } from "@/shared/ui/FailureState";
import { archiveCharacter, duplicateCharacter, restoreCharacter, saveCharacterEdits, type EditBaseline } from "./data/actions";
import { editTabs, formFromView, isDirty, lorebookIds, type EditTab } from "./data/edit";
import { draftToSave, wordCount, type CharacterForm } from "./data/form";
import { characterEditQuery } from "./data/queries";
import { ScenesStep } from "./components/ScenesStep";
import { CharacterActions } from "./edit/CharacterActions";
import { ChatTab } from "./edit/ChatTab";
import { LookTab } from "./edit/LookTab";
import { ProfileTab } from "./edit/ProfileTab";
import { SoulTab } from "./edit/SoulTab";

interface Stored extends EditBaseline {
  archived: boolean;
}

function storedFrom(view: CharacterView_Serialize, lorebooks: string[]): Stored {
  return { form: formFromView(view, lorebooks), revision: view.revision, archived: view.archived };
}

function EditFlow({ characterId, view, lorebooks }: { characterId: string; view: CharacterView_Serialize; lorebooks: string[] }) {
  const { t } = useTranslation("characters");
  const { t: common } = useTranslation("common");
  const api = useRouteContext({ from: "__root__", select: (context) => context.api });
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const router = useRouter();
  const canGoBack = useCanGoBack();
  const { data: status } = useQuery(appStatusQuery(api));
  const phone = useCompactLayout() || status?.platform === "android" || status?.platform === "ios";
  const [stored, setStored] = useState<Stored>(() => storedFrom(view, lorebooks));
  const [draft, setDraft] = useState<CharacterForm>(stored.form);
  const search = useSearch({ from: "/characters/$characterId/edit" });
  const [tab, setTab] = useState<EditTab>(search.tab ?? "profile");
  const [selectedSceneId, setSelectedSceneId] = useState<string | null>(() => stored.form.draft.scenes[0]?.id ?? null);
  const [sceneEditorOpen, setSceneEditorOpen] = useState(false);
  const [saving, setSaving] = useState(false);
  const [acting, setActing] = useState(false);
  const [changingMode, setChangingMode] = useState(false);
  const mainRef = useRef<HTMLElement>(null);
  const attempt = useRef<{ payload: string; id: string } | null>(null);
  const latest = useRef(stored);
  const busy = useRef(false);
  const leaving = useRef(false);

  const companion = draft.draft.defaults.interaction_mode === "companion" && draft.draft.defaults.companion_soul !== null;
  if (tab === "soul" && !companion) setTab("profile");
  const mobileSceneEditor = phone && tab === "scenes" && sceneEditorOpen;
  const selectedScene = draft.draft.scenes.find((scene) => scene.id === selectedSceneId);
  const dirty = useMemo(() => isDirty(draft, stored.form), [draft, stored.form]);
  const canSave = !saving && !changingMode && draft.draft.profile.name.trim().length > 0 && wordCount(draft.draft.profile.definition ?? "") > 0;
  const name = draft.draft.profile.name.trim();

  const live = useRef({ dirty, mobileSceneEditor });
  useLayoutEffect(() => {
    live.current = { dirty, mobileSceneEditor };
  }, [dirty, mobileSceneEditor]);

  useBlocker({
    shouldBlockFn: async ({ action }) => {
      if (leaving.current) return false;
      if (action === "BACK" && live.current.mobileSceneEditor) {
        setSceneEditorOpen(false);
        return true;
      }
      if (!live.current.dirty) return false;
      const discard = await confirm({
        title: t("edit.leaveTitle"),
        message: t("edit.leaveBody"),
        confirmLabel: t("edit.leaveDiscard"),
        cancelLabel: t("edit.leaveKeep"),
        destructive: true,
      });
      return !discard;
    },
    enableBeforeUnload: () => live.current.dirty,
  });

  const fail = (error: unknown) => toast.error(common(`errors.code.${isApiFailure(error) ? error.code : "internal"}`));
  const apply = (next: Stored) => {
    latest.current = next;
    setStored(next);
  };
  const update = (change: (current: CharacterForm) => CharacterForm) => setDraft(change);

  const go = (next: EditTab) => {
    setSceneEditorOpen(false);
    setTab(next);
    mainRef.current?.scrollTo({ top: 0 });
  };

  const leave = () => {
    if (canGoBack) router.history.back();
    else void navigate({ to: "/library" });
  };

  const back = () => {
    if (mobileSceneEditor) setSceneEditorOpen(false);
    else leave();
  };

  const save = async () => {
    if (!canSave || busy.current) return;
    const payload = JSON.stringify([draftToSave(draft), draft.lorebookIds, stored.revision]);
    if (attempt.current?.payload !== payload) attempt.current = { payload, id: crypto.randomUUID() };
    busy.current = true;
    setSaving(true);
    try {
      const saved = await saveCharacterEdits(api, characterId, stored, draft, attempt.current.id);
      const next: Stored = saved.view ? { ...storedFrom(saved.view, saved.lorebooks), revision: saved.revision } : { ...stored, revision: saved.revision, form: { ...stored.form, lorebookIds: saved.lorebooks } };
      apply(next);
      setDraft(saved.lorebooksFailed ? { ...next.form, lorebookIds: draft.lorebookIds } : next.form);
      setSelectedSceneId((current) => (next.form.draft.scenes.some((scene) => scene.id === current) ? current : (next.form.draft.scenes[0]?.id ?? null)));
      if (saved.lorebooksFailed) toast.warning(t("edit.lorebooksFailed"));
      else toast.success(t("edit.saved"));
      await refreshLibrary(queryClient);
    } catch (error) {
      fail(error);
    } finally {
      busy.current = false;
      setSaving(false);
    }
  };

  const discard = () => {
    setDraft(stored.form);
    setSceneEditorOpen(false);
    setSelectedSceneId(stored.form.draft.scenes[0]?.id ?? null);
  };

  const act = async (task: () => Promise<void>) => {
    if (busy.current) return;
    busy.current = true;
    setActing(true);
    try {
      await task();
    } catch (error) {
      fail(error);
    } finally {
      busy.current = false;
      setActing(false);
    }
  };

  const startChat = () => void act(async () => {
    if (live.current.dirty && !await confirm({ title: t("edit.leaveTitle"), message: t("edit.leaveBody"), confirmLabel: t("edit.leaveDiscard"), cancelLabel: t("edit.leaveKeep"), destructive: true })) return;
    const conversationId = await startDirectChat(api, queryClient, characterId);
    leaving.current = true;
    try {
      await navigate({ to: "/chat/$conversationId", params: { conversationId } });
    } finally {
      leaving.current = false;
    }
  });

  const exportCharacter = () => void act(async () => {
    const result = await exportEntry(api, { kind: "character", id: characterId, name: stored.form.draft.profile.name, description: "", image: null });
    if (!result) return;
    if (result.warnings.length > 0) toast.warning(t("edit.exportWarnings", { count: result.warnings.length }));
    else toast.success(t("edit.exported"));
  });

  const duplicate = () => void act(async () => {
    const copy = await duplicateCharacter(api, characterId);
    toast.success(t("edit.duplicated", { name: copy.name }));
    await refreshLibrary(queryClient);
  });

  const restore = async () => {
    const restored = await restoreCharacter(api, characterId, latest.current.revision);
    apply({ ...latest.current, revision: restored.revision, archived: false });
    await refreshLibrary(queryClient);
  };

  const toggleArchive = () => void act(async () => {
    const current = latest.current;
    if (current.archived) {
      await restore();
      toast.success(t("edit.restored", { name: current.form.draft.profile.name }));
      return;
    }
    const archived = await archiveCharacter(api, characterId, current.revision);
    apply({ ...latest.current, revision: archived.revision, archived: true });
    toast.success(t("edit.archived", { name: current.form.draft.profile.name }), undefined, {
      action: { label: t("edit.undo"), onClick: () => void act(restore) },
    });
    await refreshLibrary(queryClient);
  });

  const props = { api, form: draft, update };
  let body: ReactNode;
  if (tab === "profile") body = <ProfileTab {...props} onModeLoadingChange={setChangingMode} />;
  else if (tab === "scenes") body = <ScenesStep {...props} mode="edit" phone={phone} editorOpen={mobileSceneEditor} selectedId={selectedSceneId} onSelect={(id) => {
    setSelectedSceneId(id);
    setSceneEditorOpen(id !== null);
    if (phone) mainRef.current?.scrollTo({ top: 0 });
  }} />;
  else if (tab === "chat") body = <ChatTab {...props} />;
  else if (tab === "look") body = <LookTab {...props} />;
  else body = <SoulTab {...props} characterId={characterId} />;

  const tabs = mobileSceneEditor ? undefined : (
    <Tabs value={tab} onChange={(next) => go(next as EditTab)}>
      <TabList aria-label={t("edit.tabs.label")}>
        {editTabs.filter((entry) => entry !== "soul" || companion).map((entry) => <Tab key={entry} value={entry} {...(entry === "scenes" ? { count: draft.draft.scenes.length } : {})}>{t(`edit.tabs.${entry}`)}</Tab>)}
      </TabList>
    </Tabs>
  );

  const saveBar = (
    <EditSaveBar
      visible={dirty && !mobileSceneEditor}
      phone={phone}
      saving={saving}
      canSave={canSave}
      discardDisabled={saving || changingMode}
      status={t("edit.unsaved")}
      discardLabel={t("edit.discard")}
      saveLabel={t("edit.save")}
      onDiscard={discard}
      onSave={() => void save()}
    />
  );

  return (
    <Page
      title={mobileSceneEditor && selectedScene ? t("scenes.number", { number: selectedScene.ordinal + 1 }) : name || t("edit.title")}
      onBack={back}
      backLabel={t(mobileSceneEditor ? "scenes.back" : "edit.back")}
      mainRef={mainRef}
      padding="none"
      contentClassName={phone ? "px-4 pb-10 pt-5" : "px-8 pb-10 pt-5"}
      {...(mobileSceneEditor ? {} : { actions: <CharacterActions phone={phone} archived={stored.archived} busy={acting || saving} onStartChat={startChat} onExport={exportCharacter} onDuplicate={duplicate} onToggleArchive={toggleArchive} />, filters: tabs })}
      footer={saveBar}
    >
      <fieldset disabled={saving} className="@container min-w-0">
        <AnimatePresence mode="wait" initial={false}>
          <motion.div key={tab} initial={{ opacity: 0, y: 6 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0, y: -6 }} transition={m.quick}>{body}</motion.div>
        </AnimatePresence>
      </fieldset>
    </Page>
  );
}

export function EditCharacterPage() {
  const { t } = useTranslation("characters");
  const params = useParams({ strict: false });
  const characterId = "characterId" in params ? params.characterId : undefined;
  const api = useRouteContext({ from: "__root__", select: (context) => context.api });
  const loaded = useQuery({ ...characterEditQuery(api, characterId ?? ""), enabled: Boolean(characterId) });
  const [snapshot, setSnapshot] = useState<{ view: CharacterView_Serialize; lorebooks: string[] } | null>(null);
  const data = loaded.data;
  if (data && data.view.id === characterId && snapshot?.view.id !== characterId) setSnapshot({ view: data.view, lorebooks: lorebookIds(data.bindings) });
  if (!characterId) return null;
  if (snapshot?.view.id === characterId) return <EditFlow key={characterId} characterId={characterId} view={snapshot.view} lorebooks={snapshot.lorebooks} />;
  if (loaded.isError) return <Page title={t("edit.title")}><FailureState error={loaded.error} onRetry={() => void loaded.refetch()} /></Page>;
  return <Page title={t("edit.title")}><LoadingState /></Page>;
}
