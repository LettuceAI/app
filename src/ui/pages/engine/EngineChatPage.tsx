import { useParams, useNavigate } from "react-router-dom";
import { ArrowLeft, Bot, User, SendHorizonal } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";

import { useEngineChatController } from "./hooks/useEngineChatController";
import { Routes } from "../../navigation";
import { readSettings } from "../../../core/storage/repo";
import type { ProviderCredential } from "../../../core/storage/schemas";
import type { EngineChatMessage } from "./hooks/engineChatReducer";
import { MarkdownRenderer } from "../chats/components/MarkdownRenderer";
import { radius, typography, interactive, shadows, cn } from "../../design-tokens";
import { getPlatform } from "../../../core/utils/platform";

// ── Page wrapper (credential loading) ───────────────────────────────────────

export function EngineChatPage() {
  const { credentialId, slug } = useParams<{ credentialId: string; slug: string }>();
  const navigate = useNavigate();
  const [credential, setCredential] = useState<ProviderCredential | null>(null);
  const [charName, setCharName] = useState<string>("");
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const settings = await readSettings();
        const cred = settings.providerCredentials.find((p) => p.id === credentialId);
        if (!cancelled && cred) {
          setCredential(cred);
          setCharName(slug?.replace(/[-_]/g, " ").replace(/\b\w/g, (c) => c.toUpperCase()) || "Chat");
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [credentialId, slug]);

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="h-8 w-8 animate-spin rounded-full border-2 border-white/10 border-t-white/60" />
      </div>
    );
  }

  if (!credential || !slug) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-3 px-4">
        <p className="text-sm text-white/60">Engine provider not found.</p>
        <button
          onClick={() => navigate(-1)}
          className="rounded-lg border border-white/10 bg-white/5 px-4 py-2 text-sm text-white/70 hover:bg-white/10"
        >
          Go Back
        </button>
      </div>
    );
  }

  return <ChatInner credential={credential} slug={slug} characterName={charName} />;
}

// ── Main chat inner ─────────────────────────────────────────────────────────

function ChatInner({
  credential,
  slug,
  characterName,
}: {
  credential: ProviderCredential;
  slug: string;
  characterName: string;
}) {
  const baseUrl = credential.baseUrl || "";
  const apiKey = credential.apiKey || "";
  const navigate = useNavigate();
  const { state, setDraft, sendMessage } = useEngineChatController(baseUrl, apiKey, slug);

  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const bottomRef = useRef<HTMLDivElement>(null);

  // Auto-scroll to bottom on new messages
  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [state.messages.length, state.sending]);

  // Character initials for avatar fallback
  const initials = useMemo(
    () => (characterName ? characterName.slice(0, 2).toUpperCase() : "?"),
    [characterName],
  );

  return (
    <div className="flex h-dvh flex-col bg-[#050505]">
      {/* ── Header (matches ChatHeader) ─────────────────────────────────── */}
      <header className="z-20 shrink-0 border-b border-white/10 bg-[#050505] px-4 pb-3 pt-10">
        <div className="flex items-center">
          <button
            onClick={() => navigate(Routes.engineHome(credential.id))}
            className="flex px-[0.6em] py-[0.3em] shrink-0 items-center justify-center -ml-2 text-white transition hover:text-white/80"
            aria-label="Back"
          >
            <ArrowLeft size={14} strokeWidth={2.5} />
          </button>

          <span className="min-w-0 flex-1 truncate text-xl font-bold text-white/90">
            {characterName}
          </span>

          {/* Avatar */}
          <div
            className="relative shrink-0 rounded-full overflow-hidden ring-1 ring-white/20"
            style={{ width: "36px", height: "36px", minWidth: "36px", minHeight: "36px", flexShrink: 0 }}
          >
            <div className="flex h-full w-full items-center justify-center rounded-full bg-white/10 text-xs font-semibold text-white">
              {initials}
            </div>
          </div>
        </div>
      </header>

      {/* ── Messages (matches Chat.tsx layout) ──────────────────────────── */}
      <main ref={scrollContainerRef} className="relative z-10 flex-1 overflow-y-auto">
        <div className="space-y-6 px-3 pb-24 pt-4">
          {state.loading ? (
            <div className="flex h-64 items-center justify-center">
              <div className="h-6 w-6 animate-spin rounded-full border-2 border-white/10 border-t-white/60" />
            </div>
          ) : (
            <>
              {state.messages.map((msg) => (
                <EngineMessage key={msg.id} message={msg} />
              ))}
              {state.sending && (
                <div className="relative flex gap-2 justify-start">
                  <div className="relative flex h-8 w-8 shrink-0 items-center justify-center overflow-hidden rounded-full border border-white/10 bg-linear-to-br from-white/5 to-white/10">
                    <Bot size={16} className="text-white/60" />
                  </div>
                  <div
                    className={cn(
                      "max-w-[82%] px-4 py-2.5 leading-relaxed",
                      radius.lg,
                      typography.body.size,
                      "border bg-gray-600/30 text-white/95 border-gray-400/40",
                    )}
                  >
                    <div className="flex items-center gap-1" aria-label="Assistant is typing" aria-live="polite">
                      <span className="typing-dot" />
                      <span className="typing-dot" style={{ animationDelay: "0.2s" }} />
                      <span className="typing-dot" style={{ animationDelay: "0.4s" }} />
                    </div>
                  </div>
                </div>
              )}
            </>
          )}
          <div ref={bottomRef} />
        </div>
      </main>

      {/* ── Footer (matches ChatFooter) ─────────────────────────────────── */}
      <EngineChatFooter
        draft={state.draft}
        setDraft={setDraft}
        error={state.error}
        sending={state.sending}
        onSendMessage={sendMessage}
      />
    </div>
  );
}

// ── Message bubble (matches ChatMessage visual design) ──────────────────────

function EngineMessage({
  message,
}: {
  message: EngineChatMessage;
}) {
  const isUser = message.role === "user";
  const isAssistant = message.role === "assistant";

  return (
    <div className={cn("relative flex gap-2", isUser ? "justify-end" : "justify-start")}>
      {/* Avatar for assistant (left side) */}
      {isAssistant && (
        <div className="relative flex h-8 w-8 shrink-0 items-center justify-center overflow-hidden rounded-full border border-white/10 bg-linear-to-br from-white/5 to-white/10">
          <Bot size={16} className="text-white/60" />
        </div>
      )}

      <div
        className={cn(
          "max-w-[82%] px-4 py-2.5 leading-relaxed",
          radius.lg,
          typography.body.size,
          isUser
            ? "ml-auto bg-emerald-400/35 text-white border border-emerald-400/50"
            : "border bg-gray-600/30 text-white/95 border-gray-400/40",
        )}
      >
        <MarkdownRenderer
          content={message.content}
          className="text-inherit select-none"
        />

        {/* Emotion tag on assistant messages */}
        {isAssistant && message.emotion && (
          <div className="mt-2 flex items-center gap-1.5 text-[10px] text-white/35">
            <span className="capitalize">{message.emotion}</span>
            {message.emotionIntensity != null && (
              <div className="h-1 w-10 rounded-full bg-white/10 overflow-hidden">
                <div
                  className="h-full rounded-full bg-white/25"
                  style={{ width: `${Math.round(message.emotionIntensity * 100)}%` }}
                />
              </div>
            )}
          </div>
        )}
      </div>

      {/* Avatar for user (right side) */}
      {isUser && (
        <div className="relative flex h-8 w-8 shrink-0 items-center justify-center overflow-hidden rounded-full border border-white/10 bg-linear-to-br from-white/5 to-white/10">
          <User size={16} className="text-white/60" />
        </div>
      )}
    </div>
  );
}

// ── Footer (copied from ChatFooter, stripped to send-only) ──────────────────

function EngineChatFooter({
  draft,
  setDraft,
  error,
  sending,
  onSendMessage,
}: {
  draft: string;
  setDraft: (value: string) => void;
  error: string | null;
  sending: boolean;
  onSendMessage: () => Promise<void>;
}) {
  const hasDraft = draft.trim().length > 0;
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const isDesktop = useMemo(() => getPlatform().type === "desktop", []);

  useEffect(() => {
    if (textareaRef.current) {
      textareaRef.current.style.height = "auto";
      textareaRef.current.style.height = `${textareaRef.current.scrollHeight}px`;
    }
  }, [draft]);

  const handleKeyDown = (event: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (!isDesktop) return;
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      if (!sending && hasDraft) {
        void onSendMessage();
      }
    }
  };

  return (
    <footer className="z-20 shrink-0 px-4 pb-6 pt-3 bg-[#050505]">
      {error && (
        <div
          className={cn(
            "mb-3 px-4 py-2.5",
            radius.md,
            "border border-red-400/30 bg-red-400/10",
            typography.bodySmall.size,
            "text-red-200",
          )}
        >
          {error}
        </div>
      )}

      <div
        className={cn(
          "relative flex items-end gap-2.5 p-2",
          "rounded-4xl",
          "border border-white/15 bg-white/5 backdrop-blur-md",
          shadows.md,
        )}
      >
        <textarea
          ref={textareaRef}
          value={draft}
          onChange={(event) => setDraft(event.target.value)}
          onKeyDown={handleKeyDown}
          placeholder={" "}
          rows={1}
          className={cn(
            "max-h-32 flex-1 resize-none bg-transparent py-2.5",
            typography.body.size,
            "text-white placeholder:text-transparent",
            "focus:outline-none",
          )}
          disabled={sending}
        />

        {draft.length === 0 && (
          <span
            className={cn(
              "pointer-events-none absolute left-5",
              "top-1/2 -translate-y-1/2",
              "text-white/40",
              "transition-opacity duration-150",
            )}
          >
            Send a message...
          </span>
        )}

        <button
          onClick={() => void onSendMessage()}
          disabled={sending || !hasDraft}
          className={cn(
            "mb-0.5 flex h-10 w-11 shrink-0 items-center justify-center self-end",
            radius.full,
            hasDraft
              ? "border border-emerald-400/40 bg-emerald-400/20 text-emerald-100"
              : "border border-white/15 bg-white/10 text-white/70",
            interactive.transition.fast,
            interactive.active.scale,
            !sending && hasDraft && "hover:border-emerald-400/60 hover:bg-emerald-400/30",
            !sending && !hasDraft && "hover:border-white/25 hover:bg-white/15",
            "disabled:cursor-not-allowed disabled:opacity-40",
          )}
          title={hasDraft ? "Send message" : "Type a message"}
          aria-label={hasDraft ? "Send message" : "Type a message"}
        >
          {sending ? (
            <span className="w-4 h-4 animate-spin rounded-full border-2 border-current border-t-transparent" />
          ) : (
            <SendHorizonal size={18} />
          )}
        </button>
      </div>
    </footer>
  );
}
