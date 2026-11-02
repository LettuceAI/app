import { useEffect, useRef, useState } from "react";
import { ImagePlus, Settings, Upload } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Alert, Button, EmptyState, icon, MediaTile, Progress, Segmented, Sheet, Spinner, Textarea } from "@lettuceai/crisp";
import type { ApiClient } from "@/api/client";
import type { AssetRef } from "@/api/generated/bindings";
import type { AvatarContext, AvatarSession, AvatarSessionState, AvatarVariant } from "./avatar-session";
import { avatarBusy, avatarCanGenerate, avatarNotice, type AvatarNotice } from "./avatar-notice";
import { useAvatarSession } from "./use-avatar-session";

type Mode = "generate" | "edit";

export interface AvatarGenerationSheetProps {
  api: ApiClient;
  open: boolean;
  onClose: () => void;
  readContext: () => AvatarContext;
  onUse: (asset: AssetRef) => void;
  onConfigure: () => void;
}

function progressValue(state: AvatarSessionState): number | undefined {
  const progress = state.job?.progress;
  if (!progress?.total || progress.total <= 0) return undefined;
  return Math.max(0, Math.min(100, (progress.current / progress.total) * 100));
}

function numbered(variants: readonly AvatarVariant[]): Map<string, number> {
  const counts: Record<AvatarVariant["origin"], number> = { current: 0, upload: 0, generation: 0, edit: 0 };
  return new Map(variants.map((variant) => [variant.id, counts[variant.origin] += 1]));
}

function NoticeAlert({ notice, session, state, onConfigure }: { notice: AvatarNotice; session: AvatarSession; state: AvatarSessionState; onConfigure: () => void }) {
  const { t } = useTranslation("image-input");
  const { t: common } = useTranslation("common");
  const retry = () => {
    if (notice.retry === "options") void session.loadOptions();
    else if (notice.retry === "upload") void session.upload();
    else if (notice.retry === "cancellation") void session.cancel();
    else if (notice.retry === "generation") void session.retry();
  };
  const cancelling = state.phase === "cancelling" && state.errorSource !== "cancellation";
  const failure = common(`errors.code.${notice.code ?? "internal"}` as "errors.code.internal");
  const copy: Record<AvatarNotice["kind"], { title: string; body?: string }> = {
    options: { title: t("notice.options.title"), body: `${failure} ${t("notice.options.body")}` },
    disabled: { title: t("notice.disabled.title"), body: t("notice.disabled.body") },
    noModel: { title: t("notice.noModel.title"), body: t("notice.noModel.body") },
    incompatible: { title: t("notice.incompatible.title"), body: t("notice.incompatible.body") },
    upload: { title: t("notice.upload.title"), body: failure },
    generation: { title: t("notice.generation.title"), ...(notice.copy ? { body: t(`failure.${notice.copy}`) } : {}) },
    cancellation: { title: t("notice.cancellation.title"), body: t("notice.cancellation.body") },
    uncertain: { title: t("notice.uncertain.title"), body: t("notice.uncertain.body") },
    cancelled: { title: t("job.cancelled") },
  };
  const { title, body } = copy[notice.kind];
  const actions = (notice.retry || notice.cancel || notice.configure) && <div className="flex flex-wrap gap-2">
    {notice.retry && <Button size="sm" variant="secondary" disabled={cancelling} onClick={retry}>{t("actions.retry")}</Button>}
    {notice.cancel && <Button size="sm" variant="ghost" disabled={cancelling} loading={cancelling} onClick={() => void session.cancel()}>{t("actions.cancel")}</Button>}
    {notice.configure && <Button size="sm" variant="ghost" leading={<Settings size={icon.sm} />} onClick={onConfigure}>{t("settings.open")}</Button>}
  </div>;
  return <Alert tone={notice.tone} title={title} {...(body ? { description: body } : {})} {...(actions ? { action: actions } : {})} />;
}

function Preview({ session, state, numbers }: { session: AvatarSession | null; state: AvatarSessionState; numbers: Map<string, number> }) {
  const { t } = useTranslation("image-input");
  const selected = state.variants.find((variant) => variant.id === state.selectedId) ?? null;
  const parent = selected?.parentId ? state.variants.find((variant) => variant.id === selected.parentId) ?? null : null;
  const working = state.phase !== "idle";
  const rejected = state.phase === "idle" && state.job?.result?.type === "image_generation" ? state.job.result.rejected_outputs : 0;
  const label = (variant: AvatarVariant) => t(`versions.${variant.origin}`, { number: numbers.get(variant.id) ?? 1 });
  const detail = selected?.origin === "current" ? t("preview.unchanged")
    : selected?.origin === "edit" && parent ? t("preview.editedFrom", { version: label(parent), request: selected.request })
      : selected?.request ?? "";

  if (!selected) {
    return <div className={working
      ? "mx-auto flex aspect-square w-full max-w-52 items-center justify-center rounded-xl bg-surface-1 ring-1 ring-inset ring-line-2 @sm:max-w-72 [@media(max-height:44rem)]:max-w-40"
      : "mx-auto w-full max-w-72 rounded-xl bg-surface-1 ring-1 ring-inset ring-line-2"}>
      {working ? <Spinner size="lg" label={t(`busy.${state.phase as Exclude<AvatarSessionState["phase"], "idle">}`)} />
        : <EmptyState size="sm" icon={<ImagePlus size={icon.xl} />} title={t("preview.empty")} description={t("preview.emptyBody")}
          action={<Button size="sm" variant="secondary" leading={<Upload size={icon.sm} />} disabled={session === null || avatarBusy(state)} onClick={() => void session?.upload()}>{t("upload.action")}</Button>} />}
    </div>;
  }
  return <figure className="space-y-2">
    <img src={selected.asset.url} alt={t("preview.alt")} className="mx-auto aspect-square w-full max-w-52 rounded-xl bg-surface-1 object-cover ring-1 ring-inset ring-line-2 @sm:max-w-72 [@media(max-height:44rem)]:max-w-40" />
    <figcaption className="mx-auto max-w-72 text-center">
      <p className="truncate text-sm font-medium text-fg">{label(selected)}</p>
      {detail && <p className="truncate text-sm text-fg-3">{detail}</p>}
      {rejected > 0 && <p className="text-sm text-fg-3">{t("job.rejected", { count: rejected })}</p>}
    </figcaption>
  </figure>;
}

function Versions({ session, state, numbers }: { session: AvatarSession | null; state: AvatarSessionState; numbers: Map<string, number> }) {
  const { t } = useTranslation("image-input");
  const busy = avatarBusy(state);
  return <section className="space-y-2" aria-label={t("versions.label")}>
    <div className="flex items-center justify-between gap-3">
      <span className="text-sm font-medium text-fg-2">{t("versions.count", { count: state.variants.length })}</span>
      <Button size="sm" variant="ghost" leading={<Upload size={icon.sm} />} disabled={busy || session === null} onClick={() => void session?.upload()}>{t("upload.action")}</Button>
    </div>
    <div className="scrollbar-thin -mx-1 flex gap-2 overflow-x-auto px-1 pb-1 pt-0.5">
      {state.variants.map((variant) => {
        const number = numbers.get(variant.id) ?? 1;
        const chosen = variant.id === state.selectedId;
        return <div key={variant.id} className="w-16 shrink-0 space-y-1">
          <MediaTile src={variant.asset.preview_url ?? variant.asset.url} alt={t(`versions.${variant.origin}`, { number })} selected={chosen} disabled={busy} onClick={() => session?.select(variant.id)} />
          <p aria-hidden="true" className={chosen ? "truncate text-center text-2xs font-medium text-fg" : "truncate text-center text-2xs text-fg-3"}>{t(`versions.short.${variant.origin}`, { number })}</p>
        </div>;
      })}
    </div>
  </section>;
}

function Composer({ session, state, numbers, usable }: { session: AvatarSession | null; state: AvatarSessionState; numbers: Map<string, number>; usable: boolean }) {
  const { t } = useTranslation("image-input");
  const [mode, setMode] = useState<Mode>("generate");
  const [prompt, setPrompt] = useState("");
  const [instruction, setInstruction] = useState("");
  const selected = state.variants.find((variant) => variant.id === state.selectedId) ?? null;
  const busy = avatarBusy(state);
  const editing = mode === "edit" && state.variants.length > 0;
  const text = editing ? instruction : prompt;
  const ready = session !== null && avatarCanGenerate(state) && !busy && text.trim().length > 0 && (!editing || selected !== null);
  const running = state.phase === "generating" || state.phase === "editing";
  const phase = state.phase !== "idle" ? state.phase : null;
  const progress = progressValue(state);
  const submit = () => {
    if (!ready) return;
    if (editing) void session.edit(instruction);
    else void session.generate(prompt);
  };
  const hint = editing && selected
    ? t("mode.editHint", { version: t(`versions.${selected.origin}`, { number: numbers.get(selected.id) ?? 1 }) })
    : t("mode.generateHint");

  return <div className="space-y-3">
    {state.variants.length > 0 && <div className="space-y-1.5">
      <Segmented<Mode> size="sm" block aria-label={t("mode.label")} value={mode} onChange={setMode} options={[{ value: "generate", label: t("mode.generate") }, { value: "edit", label: t("mode.edit") }]} />
      <p className="truncate text-sm text-fg-3">{hint}</p>
    </div>}
    {editing
      ? <Textarea key="edit" autoGrow rows={1} maxRows={4} value={instruction} aria-label={t("instruction.label")} placeholder={t("instruction.placeholder")} onChange={(event) => setInstruction(event.target.value)} />
      : <Textarea key="generate" autoGrow rows={1} maxRows={4} value={prompt} aria-label={t("prompt.label")} placeholder={t("prompt.placeholder")} onChange={(event) => setPrompt(event.target.value)} />}
    {phase
      ? <div className="control-md flex items-center gap-3">
        <div className="min-w-0 flex-1 space-y-1.5" role="status">
          <p className="truncate text-sm text-fg-2">{t(`busy.${phase}`)}</p>
          <Progress size="sm" label={t(`busy.${phase}`)} {...(progress !== undefined ? { value: progress } : {})} />
        </div>
        {(running || state.phase === "cancelling") && <Button variant="secondary" disabled={state.phase === "cancelling"} onClick={() => void session?.cancel()}>{t("actions.cancel")}</Button>}
      </div>
      : <Button variant={usable ? "secondary" : "primary"} block disabled={!ready} onClick={submit}>{editing ? t("actions.edit") : t("actions.generate")}</Button>}
  </div>;
}

/** The avatar panel: phone bottom sheet or desktop drawer, one session behind both. Nothing here saves anything; Use hands the chosen image to the owner's draft. */
export function AvatarGenerationSheet({ api, open, onClose, readContext, onUse, onConfigure }: AvatarGenerationSheetProps) {
  const { t } = useTranslation("image-input");
  const { session, state } = useAvatarSession(api, open, readContext, onClose);
  const body = useRef<HTMLDivElement>(null);
  const selected = state.variants.find((variant) => variant.id === state.selectedId) ?? null;
  const usable = session !== null && selected !== null && selected.origin !== "current" && !avatarBusy(state);
  const notice = avatarNotice(state);
  const numbers = numbered(state.variants);
  const noticeKind = notice?.kind;
  useEffect(() => {
    if (noticeKind) body.current?.scrollTo({ top: 0, behavior: "smooth" });
  }, [noticeKind]);
  const use = () => {
    const asset = session?.selectedAsset();
    if (!asset) return;
    onUse(asset);
    onClose();
  };
  return <Sheet open={open} onClose={onClose} placement="auto" size="md" scrollRef={body} title={t("title")}
    rightAction={<Button size="sm" variant="primary" disabled={!usable} onClick={use}>{t("actions.use")}</Button>}
    footer={<Composer session={session} state={state} numbers={numbers} usable={usable} />}>
    <div className="@container space-y-4 pb-1">
      {notice && session && <NoticeAlert notice={notice} session={session} state={state} onConfigure={onConfigure} />}
      <Preview session={session} state={state} numbers={numbers} />
      {state.variants.length > 0 && <Versions session={session} state={state} numbers={numbers} />}
    </div>
  </Sheet>;
}
