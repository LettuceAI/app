import { useLayoutEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useBlocker, useCanGoBack, useNavigate, useRouteContext, useRouter } from "@tanstack/react-router";
import { ArrowRight, BookOpen, Check, FileText, MessageSquare, Sparkles, User } from "lucide-react";
import { useTranslation } from "react-i18next";
import { ActionBar, Button, confirm, icon, LoadingState, Page, Stepper, toast, useCompactLayout, Wizard } from "@lettuceai/crisp";
import { appStatusQuery } from "@/api/app-status-query";
import { isApiFailure } from "@/api/client";
import { startDirectChat } from "@/features/conversations";
import { importCharacterCard } from "@/features/home";
import { FailureState } from "@/shared/ui/FailureState";
import { createCharacter } from "./data/actions";
import { canLeave, createSteps, draftToSave, isTouched, newForm, withAddedScene, type CharacterDraft, type CharacterForm } from "./data/form";
import { draftDefaultQuery, dynamicMemoryEnabledQuery } from "./data/queries";
import { DetailsStep } from "./components/DetailsStep";
import { ExtrasStep } from "./components/ExtrasStep";
import { IdentityStep } from "./components/IdentityStep";
import { ScenesStep } from "./components/ScenesStep";

function CreateFlow({ start }: { start: CharacterDraft }) {
  const { t } = useTranslation("characters");
  const { t: common } = useTranslation("common");
  const api = useRouteContext({ from: "__root__", select: (context) => context.api });
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const router = useRouter();
  const canGoBack = useCanGoBack();
  const { data: status } = useQuery(appStatusQuery(api));
  const phone = useCompactLayout() || status?.platform === "android" || status?.platform === "ios";
  const [form, setForm] = useState<CharacterForm>(() => {
    const initial = newForm(start);
    return start.scenes.length > 0 ? initial : withAddedScene(initial, { text: "", direction: "" }, crypto.randomUUID());
  });
  const [selectedSceneId, setSelectedSceneId] = useState<string | null>(() => form.draft.scenes[0]?.id ?? null);
  const [sceneEditorOpen, setSceneEditorOpen] = useState(false);
  const [index, setIndex] = useState(0);
  const [saving, setSaving] = useState(false);
  const [changingMode, setChangingMode] = useState(false);
  const finished = useRef(false);
  const mainRef = useRef<HTMLElement>(null);
  const attempt = useRef<{ payload: string; id: string } | null>(null);
  const step = createSteps[index] ?? "identity";
  const mobileSceneEditor = phone && step === "scenes" && sceneEditorOpen;
  const selectedScene = form.draft.scenes.find((scene) => scene.id === selectedSceneId);
  const last = index === createSteps.length - 1;
  const touched = isTouched(form, start);
  const stepsMeta = useMemo(() => {
    const icons = { identity: User, scenes: BookOpen, details: FileText, extras: Sparkles };
    return createSteps.map((id) => { const Icon = icons[id]; return { id, label: t(`steps.${id}`), icon: <Icon size={icon.sm} /> }; });
  }, [t]);
  const live = useRef({ index, touched, mobileSceneEditor });
  useLayoutEffect(() => {
    live.current = { index, touched, mobileSceneEditor };
  }, [index, touched, mobileSceneEditor]);

  const go = (target: number) => {
    setSceneEditorOpen(false);
    setIndex(target);
    mainRef.current?.scrollTo({ top: 0 });
  };

  useBlocker({
    shouldBlockFn: async ({ action }) => {
      if (finished.current) return false;
      if (action === "BACK" && live.current.mobileSceneEditor) {
        setSceneEditorOpen(false);
        return true;
      }
      if (action === "BACK" && live.current.index > 0) {
        go(live.current.index - 1);
        return true;
      }
      if (!live.current.touched) return false;
      const discard = await confirm({
        title: t("create.discardTitle"),
        message: t("create.discardBody"),
        confirmLabel: t("create.discard"),
        cancelLabel: t("create.keepEditing"),
        destructive: true,
      });
      return !discard;
    },
    enableBeforeUnload: () => !finished.current && live.current.touched,
  });

  const update = (change: (current: CharacterForm) => CharacterForm) => setForm(change);

  const leave = () => {
    if (canGoBack) router.history.back();
    else void navigate({ to: "/" });
  };

  const back = () => {
    if (mobileSceneEditor) setSceneEditorOpen(false);
    else if (index > 0) go(index - 1);
    else leave();
  };

  const create = async (thenChat: boolean) => {
    if (saving || changingMode || !createSteps.every((step) => canLeave(step, form))) return;
    const payload = JSON.stringify([draftToSave(form), form.lorebookIds]);
    if (attempt.current?.payload !== payload) attempt.current = { payload, id: crypto.randomUUID() };
    setSaving(true);
    try {
      const created = await createCharacter({ api, queryClient }, form, attempt.current.id);
      finished.current = true;
      toast.success(t("create.created", { name: form.draft.profile.name.trim() }));
      if (created.unattached.length > 0) toast.warning(t("create.unattached", { count: created.unattached.length }));
      if (thenChat) {
        const conversationId = await startDirectChat(api, queryClient, created.id);
        await navigate({ to: "/chat/$conversationId", params: { conversationId }, replace: true });
      } else {
        await navigate({ to: "/", replace: true });
      }
    } catch (error) {
      toast.error(common(`errors.code.${isApiFailure(error) ? error.code : "internal"}`));
    } finally {
      setSaving(false);
    }
  };

  const importCard = async () => {
    if (saving) return;
    if (touched && !await confirm({ title: t("create.discardTitle"), message: t("create.discardBody"), confirmLabel: t("create.discard"), cancelLabel: t("create.keepEditing"), destructive: true })) return;
    setSaving(true);
    try {
      if (await importCharacterCard({ api, queryClient })) {
        finished.current = true;
        toast.success(common("create.imported"));
        await navigate({ to: "/", replace: true });
      }
    } catch (error) {
      toast.error(common(`errors.code.${isApiFailure(error) ? error.code : "internal"}`));
    } finally {
      setSaving(false);
    }
  };

  const ready = !changingMode && canLeave(step, form);
  const props = { api, form, update };
  let body: ReactNode;
  if (step === "identity") body = <IdentityStep {...props} onImport={() => void importCard()} importing={saving || changingMode} onModeLoadingChange={setChangingMode} />;
  else if (step === "details") body = <DetailsStep {...props} />;
  else if (step === "scenes") body = <ScenesStep {...props} phone={phone} editorOpen={mobileSceneEditor} selectedId={selectedSceneId} onSelect={(id) => {
    setSelectedSceneId(id);
    setSceneEditorOpen(id !== null);
    if (phone) mainRef.current?.scrollTo({ top: 0 });
  }} />;
  else body = <ExtrasStep {...props} />;

  const chatButton = <Button variant="secondary" className={phone ? "min-w-0 flex-1 px-2 text-sm" : ""} disabled={saving} onClick={() => void create(true)} leading={<MessageSquare size={icon.md} />}>{t("create.createAndChat")}</Button>;
  const nextButton = <Button variant="primary" className={phone && last ? "min-w-0 flex-1 px-2 text-sm" : ""} block={phone && !last} disabled={!ready || saving} loading={saving} trailing={last ? <Check size={icon.md} /> : <ArrowRight size={icon.md} />} onClick={() => { if (last) void create(false); else go(index + 1); }}>
    {last ? t("create.create") : step === "scenes" && form.draft.scenes.length === 0 ? t("create.skip") : t("create.continueTo", { step: t(`steps.${createSteps[index + 1]!}`).toLocaleLowerCase() })}
  </Button>;

  return (
    <Page
      title={mobileSceneEditor && selectedScene ? t("scenes.number", { number: selectedScene.ordinal + 1 }) : t("create.title")}
      onBack={back}
      backLabel={t(mobileSceneEditor ? "scenes.back" : "create.back")}
      mainRef={mainRef}
      padding="none"
      contentClassName={phone ? "space-y-6 px-4 pb-10 pt-1" : "space-y-6 px-8 pb-10 pt-1"}
      footer={mobileSceneEditor ? undefined :
        <ActionBar
          pinned={phone}
          className={phone ? "" : "px-8 pb-6 pt-2"}
          secondary={!phone && last ? chatButton : undefined}
          primary={phone && last ? <div className="flex w-full min-w-0 gap-2">{chatButton}{nextButton}</div> : nextButton}
        />
      }
    >
      {!mobileSceneEditor && <div className="@container">
        <Stepper steps={stepsMeta} current={index} onStepClick={(target) => { if (!saving && target < index) go(target); }} />
      </div>}
      <fieldset disabled={saving} className="@container min-w-0">
        <Wizard steps={stepsMeta} index={index} progress="none">
          {body}
        </Wizard>
      </fieldset>
    </Page>
  );
}

/** Making a character by hand: identity, starting scenes, details, extras, then save. */
export function CreateCharacterPage() {
  const { t } = useTranslation("characters");
  const api = useRouteContext({ from: "__root__", select: (context) => context.api });
  const start = useQuery(draftDefaultQuery(api));
  const dynamicMemory = useQuery(dynamicMemoryEnabledQuery(api));
  if (start.isPending || dynamicMemory.isPending) return <Page title={t("create.title")}><LoadingState /></Page>;
  if (start.isError || dynamicMemory.isError) {
    const failed = start.isError ? start : dynamicMemory;
    return <Page title={t("create.title")}><FailureState error={failed.error} onRetry={() => void failed.refetch()} /></Page>;
  }
  return <CreateFlow start={{ ...start.data, defaults: { ...start.data.defaults, memory_policy: dynamicMemory.data ? "dynamic" : "manual" } }} />;
}
