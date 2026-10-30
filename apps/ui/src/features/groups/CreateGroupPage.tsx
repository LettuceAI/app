import { useLayoutEffect, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useBlocker, useCanGoBack, useNavigate, useRouteContext, useRouter } from "@tanstack/react-router";
import { ArrowRight, BookOpen, Check, MessageSquare, SlidersHorizontal, Sparkles, Users } from "lucide-react";
import { useTranslation } from "react-i18next";
import { ActionBar, Alert, Button, confirm, dismissTopLayer, icon, LoadingState, Page, Stepper, toast, useCompactLayout, Wizard } from "@lettuceai/crisp";
import { appStatusQuery } from "@/api/app-status-query";
import { isApiFailure } from "@/api/client";
import { queryKeys } from "@/api/query-keys";
import { FailureState } from "@/shared/ui/FailureState";
import { MembersStep } from "./components/MembersStep";
import { SetupStep } from "./components/SetupStep";
import { SceneStep } from "./components/SceneStep";
import { ExtrasStep } from "./components/ExtrasStep";
import { draftToSave, newGroupForm, stepsFor, validStep, type GroupForm } from "./data/form";
import { castQuery } from "./data/queries";
import { launchGroup, saveGroup, type SaveAttempt } from "./data/actions";
import { ImportGroupSheet } from "./components/ImportGroupSheet";

const stepIcons = { members: Users, setup: SlidersHorizontal, scene: BookOpen, extras: Sparkles };

function GroupFlow({ start, dynamicMemory }: { start: GroupForm; dynamicMemory: boolean }) {
  const { t, i18n } = useTranslation("groups");
  const { t: common } = useTranslation();
  const api = useRouteContext({ from: "__root__", select: (context) => context.api });
  const queryClient = useQueryClient();
  const status = useQuery(appStatusQuery(api));
  const phone = useCompactLayout() || status.data?.platform === "android" || status.data?.platform === "ios";
  const cast = useQuery(castQuery(api));
  const navigate = useNavigate();
  const router = useRouter();
  const canGoBack = useCanGoBack();
  const [form, setForm] = useState(start);
  const [index, setIndex] = useState(0);
  const [saving, setSaving] = useState(false);
  const [importing, setImporting] = useState(false);
  const [partiallySaved, setPartiallySaved] = useState(false);
  const [saveError, setSaveError] = useState<unknown>(null);
  const finished = useRef(false);
  const savingRef = useRef(false);
  const attempt = useRef<SaveAttempt | null>(null);
  const mainRef = useRef<HTMLElement>(null);
  const steps = stepsFor(form.draft.chat_mode);
  const step = steps[Math.min(index, steps.length - 1)]!;
  const last = index >= steps.length - 1;
  const touched = JSON.stringify(form) !== JSON.stringify(start);
  const live = useRef({ index, touched, importing });
  useLayoutEffect(() => { live.current = { index, touched, importing }; }, [index, touched, importing]);
  const go = (next: number) => { setIndex(next); mainRef.current?.scrollTo({ top: 0 }); };
  useBlocker({
    shouldBlockFn: async ({ action }) => {
      if (finished.current) return false;
      if (savingRef.current) return true;
      if (action === "BACK" && dismissTopLayer()) return true;
      if (action === "BACK" && live.current.importing) { setImporting(false); return true; }
      if (action === "BACK" && live.current.index > 0) { go(live.current.index - 1); return true; }
      if (!live.current.touched) return false;
      return !await confirm({ title: t("create.discardTitle"), message: t("create.discardBody"), confirmLabel: t("create.discard"), cancelLabel: t("create.keep"), destructive: true });
    },
    enableBeforeUnload: () => !finished.current && live.current.touched,
  });
  const names = form.draft.members.flatMap((member) => cast.data?.filter((item) => item.id === member.character_id).map((item) => item.name) ?? []);
  const fallbackName = names.length <= 3 ? new Intl.ListFormat(i18n.language, { style: "long", type: "conjunction" }).format(names) : t("create.autoName", { names: names.slice(0, 2).join(", "), count: names.length - 2, formatted: new Intl.NumberFormat(i18n.language).format(names.length - 2) });
  const ready = !cast.isPending && !cast.isError && validStep(step, form);
  const stepsMeta = steps.map((id) => { const Icon = stepIcons[id]; return { id, label: t(`steps.${id}`), icon: <Icon size={icon.sm} /> }; });
  const leave = () => { if (canGoBack) router.history.back(); else void navigate({ to: "/groups" }); };
  const back = () => { if (saving) return; if (index > 0) go(index - 1); else leave(); };
  const create = async (thenChat: boolean) => {
    if (savingRef.current || !steps.every((step) => validStep(step, form)) || !fallbackName) return;
    const draft = draftToSave(form, fallbackName);
    const payload = JSON.stringify([draft, form.lorebookIds]);
    if (attempt.current?.payload !== payload) attempt.current = { payload, operationId: crypto.randomUUID() };
    const run = attempt.current;
    savingRef.current = true;
    setSaving(true);
    setSaveError(null);
    try {
      const groupId = await saveGroup(api, queryClient, draft, form.lorebookIds, run);
      if (thenChat) {
        const conversationId = run.conversationId ?? await launchGroup(api, queryClient, groupId, `${run.operationId}:chat`);
        run.conversationId = conversationId;
        finished.current = true;
        await navigate({ to: "/chat/$conversationId", params: { conversationId }, replace: true });
      } else {
        finished.current = true;
        await navigate({ to: "/groups", replace: true });
      }
      toast.success(t("create.created", { name: draft.name }));
    } catch (error) {
      setPartiallySaved(Boolean(run.saved));
      setSaveError(error);
      toast.error(common(`errors.code.${isApiFailure(error) ? error.code : "internal"}`));
    } finally { savingRef.current = false; setSaving(false); }
  };
  const props = { api, form, update: setForm };
  const chatButton = <Button variant="secondary" className={phone ? "min-w-0 flex-1 px-2 text-sm" : ""} disabled={saving || !ready} onClick={() => void create(true)} leading={<MessageSquare size={icon.md} />}>{t("create.andChat")}</Button>;
  const nextButton = <Button variant="primary" className={phone && last ? "min-w-0 flex-1 px-2 text-sm" : ""} block={phone && !last} disabled={saving || !ready} loading={saving} trailing={last ? <Check size={icon.md} /> : <ArrowRight size={icon.md} />} onClick={() => last ? void create(false) : go(index + 1)}>{last ? t("create.submit") : t("create.continueTo", { step: t(`steps.${steps[index + 1]!}`).toLocaleLowerCase() })}</Button>;
  return <Page title={t("create.title")} onBack={back} backLabel={t("create.back")} mainRef={mainRef} padding="none" contentClassName={phone ? "space-y-6 px-4 pb-10 pt-1" : "space-y-6 px-8 pb-10 pt-1"} footer={<ActionBar pinned={phone} className={phone ? "" : "px-8 pb-6 pt-2"} leading={!ready ? <span className="text-sm text-fg-3">{t(step === "members" ? "create.pickTwo" : "create.completeScene")}</span> : undefined} secondary={!phone && last ? chatButton : undefined} primary={phone && last ? <div className="flex w-full min-w-0 gap-2">{chatButton}{nextButton}</div> : nextButton} />}>
    <div className="@container"><Stepper steps={stepsMeta} current={index} onStepClick={(target) => { if (!saving && target < index) go(target); }} /></div>
    {saveError !== null && <Alert tone="danger" title={common(`errors.code.${isApiFailure(saveError) ? saveError.code : "internal"}`)} description={partiallySaved ? t("create.partial") : t("create.failure")} />}
    <fieldset disabled={saving || partiallySaved} className="@container min-w-0"><Wizard steps={stepsMeta} index={index} progress="none">
      {step === "members" ? <MembersStep {...props} onImport={() => setImporting(true)} /> : step === "setup" ? <SetupStep {...props} fallbackName={fallbackName} dynamicMemory={dynamicMemory} /> : step === "scene" ? <SceneStep {...props} /> : <ExtrasStep {...props} />}
    </Wizard></fieldset>
    <ImportGroupSheet api={api} open={importing} onClose={() => setImporting(false)} onImported={() => { finished.current = true; void navigate({ to: "/groups", replace: true }); }} />
  </Page>;
}

export function CreateGroupPage() {
  const { t } = useTranslation("groups");
  const api = useRouteContext({ from: "__root__", select: (context) => context.api });
  const start = useQuery({ queryKey: queryKeys.characters.draftDefault("roleplay"), queryFn: () => api.call("characterDraftDefault", "roleplay") });
  const settings = useQuery({ queryKey: queryKeys.settings.detail(), queryFn: () => api.call("settingsGet") });
  const [initial, setInitial] = useState<GroupForm | null>(null);
  const failed = [start, settings].find((query) => query.isError);
  if (failed) return <Page title={t("create.title")}><FailureState error={failed.error} onRetry={() => void failed.refetch()} /></Page>;
  if (!start.data || !settings.data) return <Page title={t("create.title")}><LoadingState /></Page>;
  if (!initial) {
    setInitial(newGroupForm(start.data.presentation.chat_appearance, settings.data.global.dynamic_memory.enabled));
    return null;
  }
  return <GroupFlow start={initial} dynamicMemory={settings.data.global.dynamic_memory.enabled} />;
}
