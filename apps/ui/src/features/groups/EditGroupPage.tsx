import { useLayoutEffect, useRef, useState } from "react";
import { motion, useReducedMotion } from "framer-motion";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useBlocker, useCanGoBack, useNavigate, useParams, useRouteContext, useRouter, useSearch } from "@tanstack/react-router";
import { MessageSquarePlus, MoreHorizontal, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import {
  Alert, Button, confirm, dismissTopLayer, Field, icon, IconButton, Input, Menu, MenuContent,
  MenuItem, MenuSeparator, MenuTrigger, motion as m, Page, Segmented, Skeleton, Tab, TabList, Tabs, toast, useCompactLayout,
} from "@lettuceai/crisp";
import { appStatusQuery } from "@/api/app-status-query";
import { isApiFailure } from "@/api/client";
import { queryKeys } from "@/api/query-keys";
import type { GroupChatMode } from "@/api/generated/bindings";
import { EditSaveBar } from "@/shared/ui/EditSaveBar";
import { FailureState } from "@/shared/ui/FailureState";
import { BackgroundGroup, MemoryGroup, SpeakerGroup } from "./components/GroupFields";
import { EditMembersTab, type EditorMember } from "./components/EditMembersTab";
import { SceneStep } from "./components/SceneStep";
import { GroupAppearanceEditor } from "./components/GroupAppearanceEditor";
import { ExtrasStep } from "./components/ExtrasStep";
import { launchGroup, refreshGroups, saveGroup, type SaveAttempt } from "./data/actions";
import { editSections, formFromGroup, rebaseGroup, same, type EditSection, type GroupBaseline } from "./data/edit";
import { draftToSave, validStep } from "./data/form";
import { castQuery, groupEditQuery, loadGroupEdit } from "./data/queries";

type LoadedGroup = Awaited<ReturnType<typeof loadGroupEdit>>;

function EditFlow({ loaded, dynamicMemory }: { loaded: LoadedGroup; dynamicMemory: boolean }) {
  const { t, i18n } = useTranslation("groups");
  const { t: common } = useTranslation();
  const reduced = useReducedMotion();
  const api = useRouteContext({ from: "__root__", select: (context) => context.api });
  const queryClient = useQueryClient();
  const status = useQuery(appStatusQuery(api));
  const phone = useCompactLayout() || status.data?.platform === "android" || status.data?.platform === "ios";
  const cast = useQuery(castQuery(api));
  const navigate = useNavigate();
  const router = useRouter();
  const canGoBack = useCanGoBack();
  const search = useSearch({ from: "/groups/$groupId/edit" });
  const [section, setSection] = useState<EditSection>(search.section ?? "group");
  const [stored, setStored] = useState<GroupBaseline>(() => ({ id: loaded.view.id, revision: loaded.view.revision, form: formFromGroup(loaded.view, loaded.lorebookIds) }));
  const [form, setForm] = useState(stored.form);
  const [busy, setBusy] = useState<"save" | "reload" | "action" | null>(null);
  const [conflict, setConflict] = useState(false);
  const [saveError, setSaveError] = useState<unknown>(null);
  const [partial, setPartial] = useState(false);
  const attempt = useRef<SaveAttempt | null>(null);
  const launchAttempt = useRef<string | null>(null);
  const working = useRef(false);
  const leaving = useRef(false);
  const mainRef = useRef<HTMLElement>(null);
  const dirty = !same(form, stored.form);
  const live = useRef({ dirty, busy });
  useLayoutEffect(() => { live.current = { dirty, busy }; }, [dirty, busy]);
  const roleplay = form.draft.chat_mode === "roleplay";
  if (section === "scene" && !roleplay) setSection("group");
  const memberViews = new Map(loaded.members.map((member) => [member.id, member]));
  const allCast: EditorMember[] = [...(cast.data ?? []).map((member) => ({ ...member, archived: memberViews.get(member.id)?.archived ?? false }))];
  for (const view of loaded.members) {
    if (allCast.some((member) => member.id === view.id)) continue;
    const avatarId = view.draft.media.find((link) => link.slot === "avatar_original")?.asset_id;
    allCast.push({ id: view.id, name: view.draft.profile.name, description: view.draft.profile.description, avatar: view.assets.find((asset) => asset.asset_id === avatarId) ?? null, archived: view.archived });
  }
  const names = form.draft.members.flatMap((member) => allCast.filter((item) => item.id === member.character_id).map((item) => item.name));
  const fallbackName = names.length ? new Intl.ListFormat(i18n.language, { style: "long", type: "conjunction" }).format(names) : stored.form.draft.name;
  const membersChanged = !same(form.draft.members, stored.form.draft.members);
  const archived = allCast.find((member) => member.archived && form.draft.members.some((item) => item.character_id === member.id));
  const blocker = conflict ? t("edit.reloadFirst") : membersChanged && form.draft.members.length < 2 ? t("edit.needTwo") : membersChanged && form.draft.members.every((member) => member.muted) ? t("edit.lastAnswering") : membersChanged && archived ? t("edit.restoreMember", { name: archived.name }) : roleplay && !validStep("scene", form) ? t("create.completeScene") : null;
  const fail = (error: unknown) => toast.error(common(`errors.code.${isApiFailure(error) ? error.code : "internal"}`));
  const discardPrompt = () => confirm({ title: t("edit.leaveTitle"), message: t("edit.leaveBody"), confirmLabel: t("create.discard"), cancelLabel: t("create.keep"), destructive: true });
  useBlocker({
    shouldBlockFn: async ({ action }) => {
      if (leaving.current) return false;
      if (working.current) return true;
      if (action === "BACK" && dismissTopLayer()) return true;
      return live.current.dirty && !await discardPrompt();
    },
    enableBeforeUnload: () => !leaving.current && live.current.dirty,
  });
  const leave = () => { if (canGoBack) router.history.back(); else void navigate({ to: "/groups" }); };
  const go = (next: EditSection) => { setSection(next); mainRef.current?.scrollTo({ top: 0 }); };
  const applyLoaded = (next: LoadedGroup) => {
    queryClient.setQueryData(groupEditQuery(api, next.view.id).queryKey, next);
    return { id: next.view.id, revision: next.view.revision, form: formFromGroup(next.view, next.lorebookIds) };
  };
  const save = async () => {
    if (working.current || blocker || !dirty) return;
    const draft = draftToSave(form, fallbackName);
    const payload = JSON.stringify([draft, form.lorebookIds, stored.revision]);
    if (attempt.current?.payload !== payload) attempt.current = { payload, operationId: crypto.randomUUID() };
    const run = attempt.current;
    working.current = true; setBusy("save"); setSaveError(null);
    try {
      await saveGroup(api, queryClient, draft, form.lorebookIds, run, { id: stored.id, revision: stored.revision, lorebookIds: stored.form.lorebookIds });
      const next = applyLoaded(await loadGroupEdit(api, stored.id));
      setStored(next); setForm(next.form); setPartial(false); attempt.current = null;
      toast.success(t("edit.saved"));
    } catch (error) {
      setSaveError(error); setPartial(Boolean(run.saved));
      if (isApiFailure(error) && error.code === "conflict") setConflict(true);
      fail(error);
    } finally { working.current = false; setBusy(null); }
  };
  const reload = async (keepEdits = true) => {
    if (working.current) return;
    working.current = true; setBusy("reload");
    try {
      const next = applyLoaded(await loadGroupEdit(api, stored.id));
      setForm(keepEdits ? rebaseGroup(form, stored.form, next.form) : next.form); setStored(next);
      setConflict(false); setSaveError(null); setPartial(false); attempt.current = null;
      toast.success(t(keepEdits ? "edit.reloaded" : "edit.discarded"));
    } catch (error) { fail(error); }
    finally { working.current = false; setBusy(null); }
  };
  const discard = () => { if (partial) void reload(false); else { setForm(stored.form); setSaveError(null); attempt.current = null; } };
  const newChat = async () => {
    if (working.current || (dirty && !await discardPrompt())) return;
    working.current = true; setBusy("action");
    try {
      launchAttempt.current ??= crypto.randomUUID();
      const conversationId = await launchGroup(api, queryClient, stored.id, launchAttempt.current);
      leaving.current = true;
      await navigate({ to: "/chat/$conversationId", params: { conversationId } });
    } catch (error) { fail(error); }
    finally { working.current = false; setBusy(null); }
  };
  const deleteGroup = async () => {
    if (working.current) return;
    if (!await confirm({ title: t("menu.deleteTitle", { name: stored.form.draft.name }), message: t("menu.deleteBody"), confirmLabel: t("menu.delete"), cancelLabel: common("actions.cancel"), destructive: true })) return;
    working.current = true; setBusy("action");
    try {
      await api.call("groupDelete", { group_id: stored.id, expected_revision: stored.revision, client_operation_id: crypto.randomUUID() });
      leaving.current = true;
      await navigate({ to: "/groups", replace: true });
      await refreshGroups(queryClient);
    } catch (error) { fail(error); }
    finally { working.current = false; setBusy(null); }
  };
  const props = { api, form, update: setForm };
  const modeHint = form.draft.chat_mode === stored.form.draft.chat_mode ? t(`setup.${form.draft.chat_mode}Body`) : t(form.draft.chat_mode === "roleplay" ? "edit.modeRoleplay" : stored.form.draft.starting_scene ? "edit.modeRemovesScene" : "edit.modeConversation");
  const tabs = <Tabs value={section} onChange={go}><TabList aria-label={t("edit.sections")} className={phone ? "flex-wrap" : ""}>
    {editSections.filter((entry) => entry !== "scene" || roleplay).map((entry) => <Tab key={entry} value={entry} {...(entry === "members" ? { count: form.draft.members.length } : {})}>{t(`edit.tabs.${entry}`)}</Tab>)}
  </TabList></Tabs>;
  const actions = <div className="flex items-center gap-1.5">
    {!phone && <Button variant="secondary" size="sm" disabled={busy !== null} leading={<MessageSquarePlus size={icon.sm} />} onClick={() => void newChat()}>{t("menu.newChat")}</Button>}
    <Menu><MenuTrigger><IconButton label={t("edit.more")} disabled={busy !== null}><MoreHorizontal size={icon.lg} /></IconButton></MenuTrigger>
      <MenuContent align="end">{phone && <MenuItem icon={<MessageSquarePlus size={icon.md} />} onSelect={() => void newChat()}>{t("menu.newChat")}</MenuItem>}{phone && <MenuSeparator />}<MenuItem icon={<Trash2 size={icon.md} />} danger onSelect={() => void deleteGroup()}>{t("menu.delete")}</MenuItem></MenuContent>
    </Menu>
  </div>;
  return <Page title={<span className="block truncate">{form.draft.name.trim() || fallbackName}</span>} onBack={leave} backLabel={t("create.back")} mainRef={mainRef} padding="none" contentClassName={phone ? "space-y-5 px-4 pb-10 pt-5" : "space-y-5 px-8 pb-10 pt-5"} actions={actions} filters={tabs} footer={<>
    {phone && dirty && blocker && <p className="px-4 pt-3 text-sm text-fg-3" role="status">{blocker}</p>}
    <EditSaveBar visible={dirty} phone={phone} saving={busy === "save"} canSave={dirty && busy === null && !blocker} discardDisabled={busy !== null} status={blocker ?? t(busy === "save" ? "edit.saving" : "edit.unsaved")} discardLabel={t("create.discard")} saveLabel={t("edit.save")} onDiscard={discard} onSave={() => void save()} />
  </>}>
    {conflict ? <Alert tone="warning" title={t("edit.conflictTitle")} description={t("edit.conflictBody")} action={<Button size="sm" variant="secondary" loading={busy === "reload"} onClick={() => void reload()}>{t("edit.reload")}</Button>} /> : saveError !== null && <Alert tone="danger" title={common(`errors.code.${isApiFailure(saveError) ? saveError.code : "internal"}`)} description={t(partial ? "create.partial" : "edit.saveFailed")} />}
    <fieldset disabled={busy !== null || partial} className="@container min-w-0">
      <motion.div key={section} initial={reduced ? false : { opacity: 0, y: 6 }} animate={{ opacity: 1, y: 0 }} transition={m.quick}>
      {section === "group" ? <div className="grid grid-cols-1 gap-6 @4xl:grid-cols-2 @4xl:gap-x-10">
        <div className="min-w-0 space-y-6">
          <Field label={t("setup.name")} hint={t("setup.nameHint")}><Input value={form.draft.name} placeholder={fallbackName} onChange={(event) => setForm((current) => ({ ...current, draft: { ...current.draft, name: event.target.value } }))} /></Field>
          <Field label={t("setup.mode")} hint={modeHint}><Segmented<GroupChatMode> block aria-label={t("setup.mode")} value={form.draft.chat_mode} onChange={(chat_mode) => setForm((current) => ({ ...current, draft: { ...current.draft, chat_mode } }))} options={[{ value: "conversation", label: t("setup.conversation") }, { value: "roleplay", label: t("setup.roleplay") }]} /></Field>
        </div>
        <div className="min-w-0 space-y-6"><BackgroundGroup {...props} /></div>
      </div> : section === "members" ? <EditMembersTab {...props} cast={allCast} phone={phone} /> : section === "scene" ? <SceneStep {...props} autoFocus={false} /> : section === "appearance" ? <GroupAppearanceEditor draft={form.draft} onChange={(draft) => setForm((current) => ({ ...current, draft }))} phone={phone} /> : <div className="space-y-6">
        <p className="text-base text-fg-2">{t("edit.chatFollow")}</p>
        <div className="grid grid-cols-1 gap-6 @4xl:grid-cols-2 @4xl:gap-x-10"><div className="min-w-0 space-y-6"><SpeakerGroup {...props} /><MemoryGroup {...props} dynamicMemory={dynamicMemory} /></div><div className="min-w-0 space-y-6"><ExtrasStep {...props} libraryOnly /></div></div>
      </div>}
      </motion.div>
    </fieldset>
  </Page>;
}

export function EditGroupPage() {
  const { t } = useTranslation("groups");
  const api = useRouteContext({ from: "__root__", select: (context) => context.api });
  const { groupId } = useParams({ from: "/groups/$groupId/edit" });
  const navigate = useNavigate();
  const loaded = useQuery(groupEditQuery(api, groupId));
  const cast = useQuery(castQuery(api));
  const settings = useQuery({ queryKey: queryKeys.settings.detail(), queryFn: () => api.call("settingsGet") });
  const failed = [loaded, cast, settings].find((query) => query.isError && !query.data);
  if (failed || !loaded.data || !cast.data || !settings.data) return <Page title={t("create.editTitle")} onBack={() => void navigate({ to: "/groups" })} backLabel={t("create.back")} padding="none" contentClassName="space-y-6 px-4 pb-10 pt-5 @4xl:px-8">
    {failed ? <FailureState error={failed.error} onRetry={() => void failed.refetch()} /> : <div className="space-y-6" aria-busy="true"><Skeleton className="h-10 w-full" /><div className="grid gap-6 @4xl:grid-cols-2"><div className="space-y-6"><Skeleton className="h-16 w-full" /><Skeleton className="h-16 w-full" /></div><Skeleton className="h-20 w-full" /></div></div>}
  </Page>;
  return <EditFlow key={groupId} loaded={loaded.data} dynamicMemory={settings.data.global.dynamic_memory.enabled} />;
}
