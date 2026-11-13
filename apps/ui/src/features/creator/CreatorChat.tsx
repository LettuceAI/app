import { Fragment, type ReactNode, useCallback, useEffect, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Check, Copy, GitCompare, ImagePlus, RefreshCw, RotateCcw, UserRound, Users, Wand2, X } from "lucide-react";
import { Badge, Button, cn, Composer, EmptyState, icon, ListGroup, ListRowButton, MediaTile, Message, Prose, Sheet, Spinner, toast } from "@lettuceai/crisp";
import { isApiFailure, type ApiClient } from "@/api/client";
import type { AssetRef, CreatorKind, CreatorToolImage, CreatorToolView, CreatorTurnView, CreatorView } from "@/api/generated/bindings";
import { pickImages } from "@/entities/image-input";
import { watchJob } from "@/entities/job";
import { useMessageAppearance } from "@/entities/preferences";
import { FailureState } from "@/shared/ui/FailureState";
import { CreatorImageViewer } from "./CreatorImageViewer";
import { type PickedReference, ReferenceSheet } from "./ReferenceSheet";

export interface CreatorAttachments {
  references: PickedReference[];
  images: AssetRef[];
}

export type AttachPanel = "attach" | "character" | "persona";

/** The images and the referenced characters and personas sent with a message. */
function TurnAttachments({ api, turn }: { api: ApiClient; turn: CreatorTurnView }) {
  const { t } = useTranslation("creator");
  const [viewing, setViewing] = useState<number | null>(null);
  if (turn.images.length === 0 && turn.references.length === 0) return null;
  return (
    <div className="space-y-1.5">
      {viewing !== null && <CreatorImageViewer api={api} images={turn.images} index={viewing} alt={t("attach.image")} onIndexChange={setViewing} />}
      {turn.images.length > 0 && (
        <div className={cn("grid gap-1.5", turn.images.length > 1 ? "grid-cols-2" : "grid-cols-1")}>
          {turn.images.map((image, index) => (
            <MediaTile key={image.asset_id} src={image.preview_url ?? image.url} alt={t("attach.image")} className="max-h-56" onClick={() => setViewing(index)} />
          ))}
        </div>
      )}
      {turn.references.length > 0 && (
        <div className="flex flex-wrap gap-1.5">
          {turn.references.map((reference) => (
            <Badge key={`${reference.kind}:${reference.id}`} tone="neutral">
              {reference.name ?? t(`references.missing.${reference.kind}`)}
            </Badge>
          ))}
        </div>
      )}
    </div>
  );
}

/** A message body the way the chat column renders one. */
function Body({ text }: { text: string }) {
  return <Prose html size="inherit" quotes className="select-text">{text}</Prose>;
}

/** The text the creator has written in the rounds saved so far, as the stream delivered it. */
const savedText = (turn: CreatorTurnView) =>
  turn.steps.map((step) => (step.kind === "text" ? step.text : "")).join("");

function LiveReply({ api, turn, jobId, steps, settled, scroll }: {
  api: ApiClient;
  turn: CreatorTurnView;
  jobId: string;
  steps: ReactNode;
  settled: () => Promise<unknown>;
  scroll: () => void;
}) {
  const appearance = useMessageAppearance();
  const [text, setText] = useState(() => savedText(turn));
  const [reasoning, setReasoning] = useState(turn.reasoning);
  const [error, setError] = useState<unknown>(null);
  const [retry, setRetry] = useState(0);
  useEffect(() => {
    const abort = new AbortController();
    void watchJob(api, jobId, () => {}, abort.signal, (delta, thinking) => {
      if (delta) setText((value) => value + delta);
      if (thinking) setReasoning((value) => value + thinking);
    })
      .then(() => settled())
      .catch((failure: unknown) => {
        if (!abort.signal.aborted) setError(failure);
      });
    return () => abort.abort();
  }, [api, jobId, retry, settled]);
  useEffect(scroll, [text, reasoning, turn.steps.length, scroll]);
  if (error) return <FailureState size="sm" error={error} onRetry={() => { setError(null); setRetry((value) => value + 1); }} />;
  const saved = savedText(turn);
  const tail = text.startsWith(saved) ? text.slice(saved.length) : "";
  const empty = turn.steps.length === 0 && !tail;
  return (
    <Message appearance={appearance} role="assistant" state={empty ? "typing" : "streaming"} {...(reasoning ? { reasoning: { text: <Body text={reasoning} /> } } : {})}>
      {!empty && (
        <div className="space-y-2">
          {steps}
          {tail && <Body text={tail} />}
        </div>
      )}
    </Message>
  );
}

const knownTools = new Set(["write_definition", "write_scene", "set_name", "edit_scene", "delete_scene", "generate_image", "edit_avatar_image", "use_uploaded_image", "show_preview", "request_confirmation"]);

/** The image an image call made, with its actions; nothing while it is being made or when it failed, since the tool's
 * status already says so. */
export function ToolImage({ api, image, canAct, onUseAvatar, onEditImage }: {
  api: ApiClient;
  image: CreatorToolImage;
  canAct: boolean;
  onUseAvatar: (asset: AssetRef) => void;
  onEditImage: (asset: AssetRef) => void;
}) {
  const { t } = useTranslation("creator");
  const [viewing, setViewing] = useState(false);
  if (image.asset) {
    const asset = image.asset;
    return (
      <div className="space-y-2">
        <MediaTile src={asset.preview_url ?? asset.url} alt={t("image.label")} className="w-48" onClick={() => setViewing(true)} />
        {viewing && <CreatorImageViewer api={api} images={[asset]} index={0} alt={t("image.label")} onIndexChange={(index) => setViewing(index !== null)} />}
        <div className="flex flex-wrap gap-1">
          <Button size="sm" variant="tonal" disabled={!canAct} onClick={() => onUseAvatar(asset)}>{t("image.useAvatar")}</Button>
          <Button size="sm" variant="ghost" disabled={!canAct} onClick={() => onEditImage(asset)}>{t("image.edit")}</Button>
        </div>
      </div>
    );
  }
  return null;
}

/** One tool call inline: its status icon and name; a failed one reads in the danger color. */
function ToolItem({ tool }: { tool: CreatorToolView }) {
  const { t } = useTranslation("creator");
  const name = knownTools.has(tool.name) ? t(`tools.name.${tool.name}` as "tools.name.set_name") : tool.name;
  return (
    <li className="inline-flex items-center gap-1.5">
      {tool.status === "running" ? (
        <Spinner size="sm" label={t("tools.running")} />
      ) : tool.status === "succeeded" ? (
        <Check size={icon.sm} className="text-accent" aria-label={t("tools.succeeded")} />
      ) : (
        <X size={icon.sm} className="text-danger" aria-label={t("tools.failed")} />
      )}
      <span className={tool.status === "failed" ? "text-danger" : undefined}>{name}</span>
    </li>
  );
}

/** Consecutive tool calls on one wrapping line, and the images they made below it. */
function ToolGroup({ api, tools, canAct, onUseAvatar, onEditImage }: {
  api: ApiClient;
  tools: { tool: CreatorToolView; index: number }[];
  canAct: boolean;
  onUseAvatar: (asset: AssetRef) => void;
  onEditImage: (asset: AssetRef) => void;
}) {
  const { t } = useTranslation("creator");
  return (
    <div className="space-y-2">
      <ul className="flex flex-wrap items-center gap-x-2 gap-y-1 text-sm text-fg-3" aria-label={t("tools.label")}>
        {tools.map(({ tool, index }, position) => (
          <Fragment key={index}>
            {position > 0 && <li aria-hidden className="h-1 w-1 rounded-full bg-fg-4" />}
            <ToolItem tool={tool} />
          </Fragment>
        ))}
      </ul>
      {tools.map(({ tool, index }) =>
        tool.image ? <ToolImage key={index} api={api} image={tool.image} canAct={canAct} onUseAvatar={onUseAvatar} onEditImage={onEditImage} /> : null,
      )}
    </div>
  );
}

/** A reply in the order the creator made it: its text, and each round's tool calls as quiet status lines after it. */
export function ReplySteps({ api, turn, canAct, onUseAvatar, onEditImage }: {
  api: ApiClient;
  turn: CreatorTurnView;
  canAct: boolean;
  onUseAvatar: (asset: AssetRef) => void;
  onEditImage: (asset: AssetRef) => void;
}) {
  const groups: ({ kind: "text"; text: string } | { kind: "tools"; tools: { tool: CreatorToolView; index: number }[] })[] = [];
  for (const step of turn.steps) {
    if (step.kind === "text") {
      groups.push({ kind: "text", text: step.text });
      continue;
    }
    const tool = turn.tools[step.index];
    if (!tool) continue;
    const last = groups.at(-1);
    if (last?.kind === "tools") last.tools.push({ tool, index: step.index });
    else groups.push({ kind: "tools", tools: [{ tool, index: step.index }] });
  }
  return groups.map((group, position) =>
    group.kind === "text" ? (
      <Body key={position} text={group.text} />
    ) : (
      <ToolGroup key={position} api={api} tools={group.tools} canAct={canAct} onUseAvatar={onUseAvatar} onEditImage={onEditImage} />
    ),
  );
}

export function Turn({ api, turn, canAct, live, steps, onChanges, onRegenerate }: {
  api: ApiClient;
  turn: CreatorTurnView;
  steps: ReactNode;
  live: ReactNode;
  canAct: boolean;
  onChanges: () => void;
  onRegenerate: () => void;
}) {
  const { t } = useTranslation("creator");
  const appearance = useMessageAppearance();
  const settled = turn.status !== "pending";
  const failed = turn.status === "failed" || turn.status === "cancelled" || turn.status === "interrupted";
  const changed = settled && turn.result_proposal_id !== null;
  const again = turn.regenerable && canAct;
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(turn.reply);
      toast.success(t("copied"));
    } catch {
      toast.error(t("copyFailed"));
    }
  };
  const menu = settled && (turn.reply || changed || again) ? (
    <ListGroup>
      {turn.reply && <ListRowButton icon={<Copy size={icon.lg} />} title={t("copy")} chevron={false} onClick={() => void copy()} />}
      {changed && <ListRowButton icon={<GitCompare size={icon.lg} />} title={t("changes")} onClick={onChanges} />}
      {again && (
        <ListRowButton
          icon={failed ? <RotateCcw size={icon.lg} /> : <RefreshCw size={icon.lg} />}
          title={t(failed ? "retry" : "regenerate")}
          chevron={false}
          onClick={onRegenerate}
        />
      )}
    </ListGroup>
  ) : undefined;
  const details = failed ? (
    <span className="flex flex-wrap items-center gap-x-2">
      <span className="text-warning">{t(`turn.${turn.status}`)}</span>
      {again && (
        <button type="button" className="font-medium text-accent hover:underline" onClick={onRegenerate}>
          {t("retry")}
        </button>
      )}
    </span>
  ) : changed ? (
    <button type="button" className="text-fg-3 underline-offset-2 hover:text-fg hover:underline" onClick={onChanges}>
      {t("changes")}
    </button>
  ) : undefined;
  return (
    <>
      <Message appearance={appearance} role="user" {...(turn.images.length > 0 || turn.references.length > 0 ? { attachments: <TurnAttachments api={api} turn={turn} /> } : {})}>
        <Body text={turn.message} />
      </Message>
      {settled ? (
        <Message appearance={appearance} role="assistant" menu={menu} details={details} {...(turn.reasoning ? { reasoning: { text: <Body text={turn.reasoning} /> } } : {})}>
          {turn.steps.length > 0 ? (
            <div className="space-y-2">{steps}</div>
          ) : (
            <Body text={turn.reply || t(`turn.${turn.status}`)} />
          )}
        </Message>
      ) : (
        live
      )}
    </>
  );
}

export interface CreatorChatProps {
  api: ApiClient;
  work: CreatorView;
  phone: boolean;
  busy: boolean;
  hasModel: boolean;
  onModel: () => void;
  onChanges: (proposalId: string) => void;
  send: (message: string, attachments: CreatorAttachments) => Promise<boolean>;
  panel: AttachPanel | null;
  onPanel: (panel: AttachPanel | null) => void;
  onUseAvatar: (asset: AssetRef) => void;
  regenerate: (turnId: string) => void;
  stop: () => void;
  settled: () => Promise<unknown>;
}

/** The conversation with the creator: past turns, the reply being written, and the composer. */
export function CreatorChat({ api, work, phone, busy, hasModel, onModel, onChanges, send, regenerate, stop, settled, panel, onPanel, onUseAvatar }: CreatorChatProps) {
  const { t } = useTranslation("creator");
  const { t: common } = useTranslation();
  const client = useQueryClient();
  const cacheKey = ["creator", "compose", work.summary.id];
  const attachKey = ["creator", "attachments", work.summary.id];
  const [text, setText] = useState(() => client.getQueryData<string>(cacheKey) ?? "");
  const [attached, setAttached] = useState<CreatorAttachments>(() => client.getQueryData<CreatorAttachments>(attachKey) ?? { references: [], images: [] });
  const [picking, setPicking] = useState(false);
  const change = (value: string) => { setText(value); client.setQueryData(cacheKey, value); };
  const attach = (next: CreatorAttachments) => { setAttached(next); client.setQueryData(attachKey, next); };
  const pickPictures = async () => {
    onPanel(null);
    setPicking(true);
    try {
      const picked = await pickImages(api, { role: "reference_image", multiple: true });
      const current = client.getQueryData<CreatorAttachments>(attachKey) ?? attached;
      const known = new Set(current.images.map((image) => image.asset_id));
      attach({ ...current, images: [...current.images, ...picked.map((image) => image.asset).filter((asset) => !known.has(asset.asset_id))] });
    } catch (error) {
      toast.error(common(`errors.code.${isApiFailure(error) ? error.code : "internal"}`));
    } finally {
      setPicking(false);
    }
  };
  const referenceKind: CreatorKind | null = panel === "character" ? "character" : panel === "persona" ? "persona" : null;
  const [submitting, setSubmitting] = useState(false);
  const scroller = useRef<HTMLDivElement>(null);
  const sticky = useRef(true);
  const scroll = useCallback(() => {
    const element = scroller.current;
    if (element && sticky.current) element.scrollTop = element.scrollHeight;
  }, []);
  const turns = work.turns.filter((turn) => !turn.regenerated);
  const active = work.active_job_id !== null;
  const toolCount = turns.reduce((count, turn) => count + turn.tools.length, 0);
  useEffect(scroll, [turns.length, toolCount, scroll]);

  const submit = async () => {
    if (submitting || !text.trim()) return;
    setSubmitting(true);
    try {
      if (await send(text, attached)) {
        change("");
        attach({ references: [], images: [] });
      }
    } finally {
      setSubmitting(false);
    }
  };
  const notice = !hasModel ? (
    <span className="flex flex-wrap items-center gap-x-2">
      {t("modelMissing")}
      <button type="button" className="font-medium text-accent hover:underline" onClick={onModel}>
        {t("chooseModel")}
      </button>
    </span>
  ) : undefined;

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div
        ref={scroller}
        className="min-h-0 flex-1 overflow-y-auto px-3 py-5 @xl:px-6"
        onWheel={(event) => { if (event.deltaY < 0) sticky.current = false; }}
        onTouchMove={() => { sticky.current = false; }}
        onScroll={(event) => {
          const element = event.currentTarget;
          sticky.current = element.scrollHeight - element.scrollTop - element.clientHeight < 24;
        }}
      >
        {turns.length === 0 && !active ? (
          <div className="flex h-full items-center justify-center">
            <EmptyState icon={<Wand2 size={icon.lg} />} title={t("startTitle")} description={t("startBody")} />
          </div>
        ) : (
          <div className="space-y-4">
            {turns.map((turn) => {
              const canAct = !busy && !active;
              const steps = (
                <ReplySteps
                  api={api}
                  turn={turn}
                  canAct={canAct}
                  onUseAvatar={onUseAvatar}
                  onEditImage={(asset) => {
                    const current = client.getQueryData<CreatorAttachments>(attachKey) ?? attached;
                    if (!current.images.some((image) => image.asset_id === asset.asset_id)) attach({ ...current, images: [...current.images, asset] });
                  }}
                />
              );
              return (
                <Turn
                  key={turn.id}
                  api={api}
                  turn={turn}
                  canAct={canAct}
                  steps={steps}
                  live={
                    turn.job_id === work.active_job_id ? (
                      <LiveReply key={turn.job_id} api={api} turn={turn} jobId={turn.job_id} steps={steps} settled={settled} scroll={scroll} />
                    ) : null
                  }
                  onChanges={() => { if (turn.result_proposal_id) onChanges(turn.result_proposal_id); }}
                  onRegenerate={() => regenerate(turn.id)}
                />
              );
            })}
          </div>
        )}
      </div>
      <div className="shrink-0 px-3 pb-[calc(var(--safe-bottom)+10px)] pt-2 @xl:px-6">
        <Composer
          value={text}
          onChange={change}
          onSend={() => void submit()}
          onStop={stop}
          sending={active}
          disabled={busy || submitting || (!active && !hasModel)}
          enterSends={!phone}
          placeholder={t("placeholder")}
          onAttach={() => onPanel("attach")}
          attachments={[
            ...attached.images.map((image) => ({ id: `image:${image.asset_id}`, src: image.preview_url ?? image.url, name: t("attach.image") })),
            ...attached.references.map((reference) => ({ id: `${reference.kind}:${reference.id}`, name: reference.name })),
          ]}
          onRemoveAttachment={(id) => attach({
            images: attached.images.filter((image) => `image:${image.asset_id}` !== id),
            references: attached.references.filter((reference) => `${reference.kind}:${reference.id}` !== id),
          })}
          {...(notice ? { notice } : {})}
        />
      </div>
      <Sheet open={panel === "attach"} onClose={() => onPanel(null)} size="sm" title={t("attach.title")}>
        <div className="pb-1">
          <ListGroup>
            <ListRowButton icon={<ImagePlus size={icon.lg} />} title={t("attach.image")} description={t("attach.imageBody")} disabled={picking} onClick={() => void pickPictures()} />
            <ListRowButton icon={<Users size={icon.lg} />} title={t("attach.character")} description={t("attach.characterBody")} onClick={() => onPanel("character")} />
            <ListRowButton icon={<UserRound size={icon.lg} />} title={t("attach.persona")} description={t("attach.personaBody")} onClick={() => onPanel("persona")} />
          </ListGroup>
        </div>
      </Sheet>
      <ReferenceSheet
        api={api}
        kind={referenceKind}
        value={attached.references}
        onChange={(references) => attach({ ...attached, references })}
        onClose={() => onPanel(null)}
      />
    </div>
  );
}
