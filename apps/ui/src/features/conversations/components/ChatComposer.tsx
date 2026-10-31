import { useEffect, useImperativeHandle, useMemo, useRef, useState, type FormEvent, type KeyboardEvent, type ReactNode, type Ref } from "react";
import { useBlocker, useNavigate } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import {
  Banner,
  Button,
  Composer,
  Sheet,
  toast,
  useCompactLayout,
  type ComposerAttachment,
} from "@lettuceai/crisp";
import { useChatLook } from "@/entities/preferences";
import { ApiFailure, type ApiClient } from "@/api/client";
import { dictationErrorKey, useDictation } from "../data/use-dictation";
import type { MemoryBlockedReason, RequiredModel } from "@/api/generated/bindings";
import { activeMention, insertMention, type ActiveMention } from "../data/mentions";
import { MentionPicker } from "./MentionPicker";
import type { Speaker } from "./SpeakerSheet";

const systemSendConfirmationKey = "lettuce.chat.systemSendConfirmationDisabled";

export interface Blockers {
  missingModels: readonly RequiredModel[];
  modelRequired: boolean;
  memoryBlocked: MemoryBlockedReason | null;
}

export function ChatBanners({
  blockers,
  swap,
  onEndSwap,
  onRetryMemory,
}: {
  blockers: Blockers;
  swap: boolean;
  onEndSwap: () => void;
  onRetryMemory: () => void;
}) {
  const { t } = useTranslation("conversations");
  const navigate = useNavigate();
  const models = (
    <Button
      variant="secondary"
      size="sm"
      onClick={() => void navigate({ to: "/settings/$section", params: { section: "models" } })}
    >
      {t("banner.openModels")}
    </Button>
  );
  const named = blockers.missingModels.map((model) => t(`requiredModel.${model}`));
  return (
    <div className="mb-2 space-y-2 empty:hidden">
      {named.length > 0 ? (
        <Banner tone="warning" action={models}>
          {named.length === 1
            ? t("banner.modelNamed", { model: named[0] })
            : t("banner.modelsMissing", { models: named.join(", ") })}
        </Banner>
      ) : (
        blockers.modelRequired && (
          <Banner tone="warning" action={models}>
            {t("banner.modelRequired")}
          </Banner>
        )
      )}
      {swap && (
        <Banner
          tone="info"
          action={
            <Button variant="secondary" size="sm" onClick={onEndSwap}>
              {t("banner.endSwap")}
            </Button>
          }
        >
          {t("banner.swapActive")}
        </Banner>
      )}
      {blockers.memoryBlocked && (
        <Banner
          tone="warning"
          action={
            <Button variant="secondary" size="sm" onClick={onRetryMemory}>
              {t("banner.memoryRetry")}
            </Button>
          }
        >
          {t("banner.memoryBlocked")}
        </Banner>
      )}
    </div>
  );
}

export interface ComposerHandle {
  read: () => string;
  replace: (text: string) => void;
  append: (text: string) => void;
  attach: (attachment: ComposerAttachment) => void;
}

export interface ChatComposerProps {
  api: ApiClient;
  conversationId: string;
  handle: Ref<ComposerHandle>;
  name: string;
  sending: boolean;
  disabled: boolean;
  canContinue: boolean;
  onSend: (text: string, attachments: ComposerAttachment[]) => Promise<boolean>;
  onSendSystem?: (text: string, attachments: ComposerAttachment[]) => Promise<boolean>;
  onContinue: () => void;
  onStop: () => void;
  onAttach: () => void;
  topSlot?: ReactNode;
  above?: ((draft: string, setDraft: (draft: string) => void) => ReactNode) | undefined;
  mentions?: readonly Speaker[] | undefined;
}

export function ChatComposer({
  api,
  conversationId,
  handle,
  name,
  sending,
  disabled,
  canContinue,
  onSend,
  onSendSystem,
  onContinue,
  onStop,
  onAttach,
  topSlot,
  above,
  mentions,
}: ChatComposerProps) {
  const look = useChatLook();
  const { t } = useTranslation("conversations");
  const { t: common } = useTranslation("common");
  const phone = useCompactLayout();
  const [draft, setDraft] = useState("");
  const [attachments, setAttachments] = useState<ComposerAttachment[]>([]);
  const [systemMenu, setSystemMenu] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const submitBusy = useRef(false);
  const systemConfirmed = useRef(false);
  if (systemMenu && (sending || disabled)) setSystemMenu(false);
  const draftRef = useRef(draft);
  useEffect(() => {
    draftRef.current = draft;
  });

  useImperativeHandle(
    handle,
    () => ({
      read: () => draftRef.current,
      replace: (text) => setDraft(text),
      attach: (attachment) => setAttachments((current) => [...current, attachment]),
      append: (text) =>
        setDraft((current) => (current.trimEnd() ? `${current.trimEnd()} ${text}` : text)),
    }),
    [],
  );

  const dictation = useDictation(api, conversationId, {
    onText: (text) => setDraft(text.trim()),
    onError: (error: ApiFailure) => {
      const key = dictationErrorKey(error);
      toast.error(key ? t(`mic.error.${key}`) : common(`errors.code.${error.code}`));
    },
    onJobError: (key) => toast.error(t(`mic.error.${key}`)),
  });

  const area = useRef<HTMLTextAreaElement | null>(null);
  const [mention, setMention] = useState<ActiveMention | null>(null);
  const matches = useMemo(() => {
    if (!mentions || !mention) return [];
    const needle = mention.query.toLowerCase();
    return mentions.filter((member) => member.name.toLowerCase().includes(needle) || (member.description ?? "").toLowerCase().includes(needle));
  }, [mentions, mention]);
  const track = (event: FormEvent<HTMLElement>) => {
    if (event.target instanceof HTMLTextAreaElement) area.current = event.target;
  };
  const change = (value: string) => {
    setDraft(value);
    setMention(mentions ? activeMention(value, area.current?.selectionStart ?? value.length) : null);
  };
  const pickMention = (member: Speaker) => {
    const target = area.current;
    if (!mention || !target) return;
    const next = insertMention(target.value, mention, target.selectionStart, member.name);
    setDraft(next.text);
    setMention(null);
    requestAnimationFrame(() => {
      target.focus();
      target.setSelectionRange(next.cursor, next.cursor);
    });
  };
  const mentionKeys = (event: KeyboardEvent<HTMLElement>) => {
    const first = matches[0];
    if (!first || event.nativeEvent.isComposing) return;
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      setMention(null);
    } else if (event.key === "Tab" || (event.key === "Enter" && !event.shiftKey)) {
      event.preventDefault();
      event.stopPropagation();
      pickMention(first);
    }
  };

  const send = (submit = onSend) => {
    if (submitBusy.current || sending || disabled) return;
    const text = draft.trim();
    if (!text && attachments.length === 0) return;
    const sent = attachments;
    submitBusy.current = true;
    setSubmitting(submit === onSendSystem);
    setDraft("");
    setAttachments([]);
    setMention(null);
    void submit(text, sent).catch((error: unknown) => {
      toast.error(common(`errors.code.${ApiFailure.fromRejection(error).code}`));
      return false;
    }).then((accepted) => {
      if (accepted) return;
      setDraft((current) => (current.length === 0 ? text : current));
      setAttachments((current) => (current.length === 0 ? sent : current));
    }).finally(() => {
      submitBusy.current = false;
      setSubmitting(false);
    });
  };

  const sendAsSystem = () => {
    if (!onSendSystem || submitBusy.current || sending || disabled || (!draft.trim() && attachments.length === 0)) return;
    let confirmed = systemConfirmed.current;
    try {
      confirmed ||= window.localStorage.getItem(systemSendConfirmationKey) === "1";
    } catch { /* Keep the confirmation available when storage is blocked. */ }
    if (confirmed) send(onSendSystem);
    else setSystemMenu(true);
  };

  const confirmSystemSend = () => {
    setSystemMenu(false);
    if (!onSendSystem || submitBusy.current || sending || disabled) return;
    systemConfirmed.current = true;
    try {
      window.localStorage.setItem(systemSendConfirmationKey, "1");
    } catch { /* The acknowledgement still lasts for this mounted chat. */ }
    send(onSendSystem);
  };

  return (
    <div className="relative">
      {matches.length === 0 && above && <div className="pointer-events-none absolute inset-x-0 bottom-full z-10">{above(draft, setDraft)}</div>}
      <div className="relative" onFocusCapture={track} onInputCapture={track} onKeyDownCapture={mentionKeys} onBlurCapture={() => setMention(null)}>
        <MentionPicker matches={matches} query={mention?.query ?? ""} onPick={pickMention} />
        <Composer
          fontSize={look.composerFontSize}
          {...(look.footerInputColorHex ? { color: look.footerInputColorHex } : {})}
          topSlot={topSlot}
          value={draft}
          onChange={change}
          onSend={() => send()}
          {...(onSendSystem ? { onSendAsSystem: sendAsSystem } : {})}
          onStop={onStop}
          onAttach={onAttach}
          attachments={attachments}
          onRemoveAttachment={(id) =>
            setAttachments((current) => current.filter((attachment) => attachment.id !== id))
          }
          {...(canContinue ? { onContinue } : {})}
          mic={{
            active: dictation.phase !== "idle",
            transcribing: dictation.phase === "transcribing",
            elapsedMs: dictation.elapsedMs,
            analyser: null,
            level: dictation.level,
            onStart: () => void dictation.start(),
            onStop: () => void dictation.stop(),
            onCancel: dictation.cancel,
            disabled: sending || disabled || submitting,
          }}
          sending={sending}
          disabled={disabled || submitting}
          enterSends={!phone}
          placeholder={t("composer.placeholder", { name })}
        />
      </div>
      {onSendSystem && <SystemSendSheet open={systemMenu} onClose={() => setSystemMenu(false)} onConfirm={confirmSystemSend} />}
    </div>
  );
}

/** First-use acknowledgement for sending a visible system message without a reply. */
function SystemSendSheet({ open, onClose, onConfirm }: { open: boolean; onClose: () => void; onConfirm: () => void }) {
  const { t } = useTranslation("conversations");
  const { t: common } = useTranslation("common");
  useBlocker({
    shouldBlockFn: ({ action }) => {
      if (action !== "BACK" || !open) return false;
      onClose();
      return true;
    },
    enableBeforeUnload: false,
  });
  return (
    <Sheet open={open} onClose={onClose} size="sm" title={t("composer.systemSend.title")} description={t("composer.systemSend.description")}>
      <div className="flex gap-2">
        <Button variant="secondary" className="flex-1" onClick={onClose}>{common("actions.cancel")}</Button>
        <Button variant="primary" className="flex-1" onClick={onConfirm}>{t("composer.systemSend.send")}</Button>
      </div>
    </Sheet>
  );
}
