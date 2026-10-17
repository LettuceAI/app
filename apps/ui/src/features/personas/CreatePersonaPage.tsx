import { useLayoutEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useBlocker, useCanGoBack, useNavigate, useRouteContext, useRouter } from "@tanstack/react-router";
import { ArrowRight, Check, Sparkles, User } from "lucide-react";
import { useTranslation } from "react-i18next";
import { ActionBar, Button, confirm, icon, Page, Stepper, toast, useCompactLayout, Wizard } from "@lettuceai/crisp";
import { appStatusQuery } from "@/api/app-status-query";
import { isApiFailure } from "@/api/client";
import { newPersonaDraft, refreshLibrary } from "@/features/library";
import { createPersona, importPersona } from "./data/actions";
import { canLeave, isTouched, newPersonaForm, personaSteps, type PersonaDraft, type PersonaForm } from "./data/form";
import { ExtrasStep } from "./components/ExtrasStep";
import { IdentityStep } from "./components/IdentityStep";

/** Making a persona by hand: identity, extras, then save. */
export function CreatePersonaPage() {
  const { t } = useTranslation("personas");
  const { t: common } = useTranslation("common");
  const api = useRouteContext({ from: "__root__", select: (context) => context.api });
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const router = useRouter();
  const canGoBack = useCanGoBack();
  const { data: status } = useQuery(appStatusQuery(api));
  const phone = useCompactLayout() || status?.platform === "android" || status?.platform === "ios";
  const [start] = useState<PersonaDraft>(() => newPersonaDraft("", ""));
  const [form, setForm] = useState<PersonaForm>(() => newPersonaForm(start));
  const [index, setIndex] = useState(0);
  const [saving, setSaving] = useState(false);
  const finished = useRef(false);
  const mainRef = useRef<HTMLElement>(null);
  const attempt = useRef<{ payload: string; id: string } | null>(null);
  const step = personaSteps[index] ?? "identity";
  const last = index === personaSteps.length - 1;
  const touched = isTouched(form, start);
  const stepsMeta = useMemo(() => {
    const icons = { identity: User, extras: Sparkles };
    return personaSteps.map((id) => { const Icon = icons[id]; return { id, label: t(`steps.${id}`), icon: <Icon size={icon.sm} /> }; });
  }, [t]);
  const live = useRef({ index, touched });
  useLayoutEffect(() => {
    live.current = { index, touched };
  }, [index, touched]);

  const go = (target: number) => {
    setIndex(target);
    mainRef.current?.scrollTo({ top: 0 });
  };

  const discardConfirm = () => confirm({
    title: t("create.discardTitle"),
    message: t("create.discardBody"),
    confirmLabel: t("create.discard"),
    cancelLabel: t("create.keepEditing"),
    destructive: true,
  });

  useBlocker({
    shouldBlockFn: async ({ action }) => {
      if (finished.current) return false;
      if (action === "BACK" && live.current.index > 0) {
        go(live.current.index - 1);
        return true;
      }
      if (!live.current.touched) return false;
      return !await discardConfirm();
    },
    enableBeforeUnload: () => !finished.current && live.current.touched,
  });

  const update = (change: (current: PersonaForm) => PersonaForm) => setForm(change);

  const back = () => {
    if (index > 0) go(index - 1);
    else if (canGoBack) router.history.back();
    else void navigate({ to: "/library" });
  };

  const done = async () => {
    finished.current = true;
    await refreshLibrary(queryClient);
    await navigate({ to: "/library", replace: true, state: { libraryKind: "persona" } });
  };

  const create = async () => {
    if (saving || !personaSteps.every((entry) => canLeave(entry, form))) return;
    const payload = JSON.stringify(form);
    if (attempt.current?.payload !== payload) attempt.current = { payload, id: crypto.randomUUID() };
    setSaving(true);
    try {
      const name = form.draft.title.trim();
      const created = await createPersona({ api, queryClient }, form, attempt.current.id);
      toast.success(t("create.created", { name }));
      if (created.unattached.length > 0) toast.warning(t("create.unattached", { count: created.unattached.length }));
      if (created.defaultFailed) toast.warning(t("create.defaultFailed", { name }));
      await done();
    } catch (error) {
      toast.error(common(`errors.code.${isApiFailure(error) ? error.code : "internal"}`));
    } finally {
      setSaving(false);
    }
  };

  const importFile = async () => {
    if (saving) return;
    if (touched && !await discardConfirm()) return;
    setSaving(true);
    try {
      if (await importPersona({ api, queryClient })) {
        toast.success(t("create.imported"));
        await done();
      }
    } catch (error) {
      toast.error(common(`errors.code.${isApiFailure(error) ? error.code : "internal"}`));
    } finally {
      setSaving(false);
    }
  };

  const props = { api, form, update };
  let body: ReactNode;
  if (step === "identity") body = <IdentityStep {...props} onImport={() => void importFile()} importing={saving} />;
  else body = <ExtrasStep {...props} />;

  const next = personaSteps[index + 1];
  return (
    <Page
      title={t("create.title")}
      onBack={back}
      backLabel={t("create.back")}
      mainRef={mainRef}
      padding="none"
      contentClassName={phone ? "space-y-6 px-4 pb-10 pt-1" : "space-y-6 px-8 pb-10 pt-1"}
      footer={
        <ActionBar
          pinned={phone}
          className={phone ? "" : "px-8 pb-6 pt-2"}
          primary={<Button variant="primary" block={phone} disabled={!canLeave(step, form) || saving} loading={saving} trailing={last ? <Check size={icon.md} /> : <ArrowRight size={icon.md} />} onClick={() => { if (last) void create(); else go(index + 1); }}>
            {last || !next ? t("create.create") : t("create.continueTo", { step: t(`steps.${next}`).toLocaleLowerCase() })}
          </Button>}
        />
      }
    >
      <div className="@container">
        <Stepper steps={stepsMeta} current={index} onStepClick={(target) => { if (!saving && target < index) go(target); }} />
      </div>
      <fieldset disabled={saving} className="@container min-w-0">
        <Wizard steps={stepsMeta} index={index} progress="none">
          {body}
        </Wizard>
      </fieldset>
    </Page>
  );
}
