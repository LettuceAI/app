import { useLayoutEffect, useMemo, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useBlocker, useCanGoBack, useNavigate, useParams, useRouteContext, useRouter } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { confirm, ListGroup, ListRow, LoadingState, Page, Switch, toast, useCompactLayout } from "@lettuceai/crisp";
import { appStatusQuery } from "@/api/app-status-query";
import { isApiFailure } from "@/api/client";
import type { PersonaDefaultView, PersonaView_Serialize } from "@/api/generated/bindings";
import { exportEntry, refreshLibrary } from "@/features/library";
import { EditSaveBar } from "@/shared/ui/EditSaveBar";
import { FailureState } from "@/shared/ui/FailureState";
import { deletePersona, loadPersona, readDefaultSlot, savePersonaEdits, setPersonaDefault, type EditBaseline } from "./data/actions";
import { formFromView, isDirty, rebase } from "./data/edit";
import { draftToSave, type PersonaForm } from "./data/form";
import { personaEditQuery } from "./data/queries";
import { IdentityFields } from "./components/IdentityFields";
import { PersonaLook } from "./components/Look";
import { PersonaLorebooks } from "./components/Lorebooks";
import { PersonaActions } from "./components/PersonaActions";

function storedFrom(view: PersonaView_Serialize, lorebooks: string[]): EditBaseline {
  return { form: formFromView(view, lorebooks), revision: view.revision };
}

function EditFlow({ personaId, view, lorebooks, defaultSlot }: { personaId: string; view: PersonaView_Serialize; lorebooks: string[]; defaultSlot: PersonaDefaultView }) {
  const { t } = useTranslation("personas");
  const { t: common } = useTranslation("common");
  const api = useRouteContext({ from: "__root__", select: (context) => context.api });
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const router = useRouter();
  const canGoBack = useCanGoBack();
  const { data: status } = useQuery(appStatusQuery(api));
  const phone = useCompactLayout() || status?.platform === "android" || status?.platform === "ios";
  const [stored, setStored] = useState<EditBaseline>(() => storedFrom(view, lorebooks));
  const [draft, setDraft] = useState<PersonaForm>(stored.form);
  const [slot, setSlot] = useState<PersonaDefaultView>(defaultSlot);
  const [saving, setSaving] = useState(false);
  const [acting, setActing] = useState(false);
  const [togglingDefault, setTogglingDefault] = useState(false);
  const attempt = useRef<{ payload: string; id: string } | null>(null);
  const latest = useRef(stored);
  const busy = useRef(false);
  const leaving = useRef(false);

  const dirty = useMemo(() => isDirty(draft, stored.form), [draft, stored.form]);
  const canSave = !saving && draft.draft.title.trim().length > 0 && draft.draft.description.trim().length > 0;
  const name = draft.draft.title.trim();
  const isDefault = slot.persona_id === personaId;

  const live = useRef({ dirty });
  useLayoutEffect(() => {
    live.current = { dirty };
  }, [dirty]);

  useBlocker({
    shouldBlockFn: async () => {
      if (leaving.current || !live.current.dirty) return false;
      const discard = await confirm({
        title: t("edit.leaveTitle"),
        message: t("edit.leaveBody"),
        confirmLabel: t("edit.leaveDiscard"),
        cancelLabel: t("edit.leaveKeep"),
        destructive: true,
      });
      return !discard;
    },
    enableBeforeUnload: () => live.current.dirty && !leaving.current,
  });

  const fail = (error: unknown) => toast.error(common(`errors.code.${isApiFailure(error) ? error.code : "internal"}`));
  const apply = (next: EditBaseline) => {
    latest.current = next;
    setStored(next);
  };
  const update = (change: (current: PersonaForm) => PersonaForm) => setDraft(change);

  const leave = () => {
    if (canGoBack) router.history.back();
    else void navigate({ to: "/library" });
  };

  const reload = async () => {
    try {
      const fresh = await loadPersona(api, personaId);
      const old = latest.current.form;
      const next = storedFrom(fresh.view, fresh.lorebooks);
      apply(next);
      setDraft((current) => rebase(current, old, next.form));
    } catch (error) {
      fail(error);
    }
  };

  const save = async () => {
    if (!canSave || busy.current) return;
    const payload = JSON.stringify([draftToSave(draft), draft.lorebookIds, stored.revision]);
    if (attempt.current?.payload !== payload) attempt.current = { payload, id: crypto.randomUUID() };
    busy.current = true;
    setSaving(true);
    try {
      const saved = await savePersonaEdits(api, personaId, stored, draft, attempt.current.id);
      const next: EditBaseline = saved.view ? { ...storedFrom(saved.view, saved.lorebooks), revision: saved.revision } : { form: { ...stored.form, lorebookIds: saved.lorebooks }, revision: saved.revision };
      apply(next);
      setDraft(saved.lorebooksFailed ? { ...next.form, lorebookIds: draft.lorebookIds } : next.form);
      if (saved.lorebooksFailed) toast.warning(t("edit.lorebooksFailed"));
      else toast.success(t("edit.saved"));
      await refreshLibrary(queryClient);
    } catch (error) {
      fail(error);
      if (isApiFailure(error) && error.code === "conflict") await reload();
    } finally {
      busy.current = false;
      setSaving(false);
    }
  };

  const discard = () => setDraft(stored.form);

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

  const toggleDefault = async (on: boolean) => {
    if (togglingDefault) return;
    const before = slot;
    setTogglingDefault(true);
    setSlot({ ...before, persona_id: on ? personaId : null });
    let failed = false;
    try {
      setSlot(await setPersonaDefault(api, personaId, on, before.revision));
      await refreshLibrary(queryClient);
    } catch (error) {
      failed = true;
      setSlot(before);
      fail(error);
    }
    try {
      setSlot(await readDefaultSlot(api));
    } catch (error) {
      if (!failed) fail(error);
    } finally {
      setTogglingDefault(false);
    }
  };

  const exportPersona = () => void act(async () => {
    const result = await exportEntry(api, { kind: "persona", id: personaId, name: stored.form.draft.title, description: "", image: null });
    if (!result) return;
    if (result.warnings.length > 0) toast.warning(t("edit.exportWarnings", { count: result.warnings.length }));
    else toast.success(t("edit.exported"));
  });

  const remove = () => void act(async () => {
    const removed = stored.form.draft.title;
    const confirmed = await confirm({
      title: t("edit.deleteTitle", { name: removed }),
      message: t("edit.deleteBody"),
      confirmLabel: t("edit.deleteConfirm"),
      cancelLabel: t("edit.deleteCancel"),
      destructive: true,
    });
    if (!confirmed) return;
    await deletePersona(api, personaId, latest.current.revision);
    toast.success(t("edit.deleted", { name: removed }));
    leaving.current = true;
    await navigate({ to: "/library", replace: true, state: { libraryKind: "persona" } });
    await refreshLibrary(queryClient);
  });

  return (
    <Page
      title={name || t("edit.title")}
      onBack={leave}
      backLabel={t("edit.back")}
      padding="none"
      contentClassName={phone ? "px-4 pb-10 pt-5" : "px-8 pb-10 pt-5"}
      actions={<PersonaActions busy={acting || saving} onExport={exportPersona} onDelete={remove} />}
      footer={
        <EditSaveBar
          visible={dirty}
          phone={phone}
          saving={saving}
          canSave={canSave}
          discardDisabled={saving}
          status={t("edit.unsaved")}
          discardLabel={t("edit.discard")}
          saveLabel={t("edit.save")}
          onDiscard={discard}
          onSave={() => void save()}
        />
      }
    >
      <fieldset disabled={saving} className="@container min-w-0">
        <div className="grid grid-cols-1 gap-6 @4xl:grid-cols-2 @4xl:gap-x-10">
          <div className="min-w-0 space-y-6">
            <IdentityFields api={api} form={draft} update={update} />
          </div>
          <div className="min-w-0 space-y-6">
            <ListGroup>
              <ListRow title={t("identity.default")} description={t("identity.defaultBody")} trailing={<Switch checked={isDefault} disabled={togglingDefault} aria-label={t("identity.default")} onChange={(on) => void toggleDefault(on)} />} />
            </ListGroup>
            <PersonaLorebooks api={api} form={draft} update={update} />
            <div className="@container">
              <PersonaLook api={api} form={draft} update={update} />
            </div>
          </div>
        </div>
      </fieldset>
    </Page>
  );
}

export function EditPersonaPage() {
  const { t } = useTranslation("personas");
  const params = useParams({ strict: false });
  const personaId = "personaId" in params ? params.personaId : undefined;
  const api = useRouteContext({ from: "__root__", select: (context) => context.api });
  const navigate = useNavigate();
  const loaded = useQuery({ ...personaEditQuery(api, personaId ?? ""), enabled: Boolean(personaId) });
  const [snapshot, setSnapshot] = useState<{ view: PersonaView_Serialize; lorebooks: string[]; defaultSlot: PersonaDefaultView } | null>(null);
  const data = loaded.data;
  if (data && data.view.id === personaId && snapshot?.view.id !== personaId) setSnapshot(data);
  if (!personaId) return null;
  if (snapshot?.view.id === personaId) return <EditFlow key={personaId} personaId={personaId} view={snapshot.view} lorebooks={snapshot.lorebooks} defaultSlot={snapshot.defaultSlot} />;
  const toLibrary = () => void navigate({ to: "/library", replace: true, state: { libraryKind: "persona" } });
  if (loaded.isError) {
    const missing = isApiFailure(loaded.error) && loaded.error.code === "not_found";
    return <Page title={t("edit.title")} onBack={toLibrary} backLabel={t("edit.backToLibrary")}><FailureState error={loaded.error} {...(missing ? {} : { onRetry: () => void loaded.refetch() })} /></Page>;
  }
  return <Page title={t("edit.title")} onBack={toLibrary} backLabel={t("edit.backToLibrary")}><LoadingState /></Page>;
}
