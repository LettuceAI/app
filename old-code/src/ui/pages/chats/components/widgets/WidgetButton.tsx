import { useMemo } from "react";
import {
  ArrowLeftRight,
  ArrowRightToLine,
  Brain,
  History,
  Plus,
  RefreshCw,
  Search,
  Square,
  Volume2,
} from "lucide-react";
import type { ButtonNode } from "../../../../../core/storage/chatWidgetSchemas";
import { cn, interactive } from "../../../../design-tokens";
import { useI18n, type TranslationKey } from "../../../../../core/i18n/context";
import { useWidgetContext } from "./WidgetContext";
import { widgetCardClass } from "./widgetSurface";

interface WidgetButtonProps {
  node: ButtonNode;
}

const DEFAULT_LABEL = {
  regenerate: "chats.widgets.button.regenerateLastReply",
  swap_places: "chats.widgets.button.swapPlaces",
  new_session: "chats.widgets.button.newSession",
  continue: "chats.widgets.button.continue",
  abort: "chats.widgets.button.stopGenerating",
  view_history: "chats.widgets.button.viewChatHistory",
  open_memories: "chats.widgets.button.memories",
  open_search: "chats.widgets.button.searchChat",
  toggle_voice_autoplay: "chats.widgets.button.voiceAutoplay",
} satisfies Record<ButtonNode["action"], TranslationKey>;

function ActionIcon({ action }: { action: ButtonNode["action"] }) {
  switch (action) {
    case "regenerate":
      return <RefreshCw size={14} strokeWidth={2.2} />;
    case "swap_places":
      return <ArrowLeftRight size={14} strokeWidth={2.2} />;
    case "new_session":
      return <Plus size={14} strokeWidth={2.2} />;
    case "continue":
      return <ArrowRightToLine size={14} strokeWidth={2.2} />;
    case "abort":
      return <Square size={13} strokeWidth={2.4} />;
    case "view_history":
      return <History size={14} strokeWidth={2.2} />;
    case "open_memories":
      return <Brain size={14} strokeWidth={2.2} />;
    case "open_search":
      return <Search size={14} strokeWidth={2.2} />;
    case "toggle_voice_autoplay":
      return <Volume2 size={14} strokeWidth={2.2} />;
  }
}

export function WidgetButton({ node }: WidgetButtonProps) {
  const { t } = useI18n();
  const ctx = useWidgetContext();
  const hasBackground = ctx.hasBackground;
  const { handler, disabled, isToggle, toggled } = useMemo(() => {
    switch (node.action) {
      case "regenerate":
        return {
          handler: ctx.onRegenerate,
          disabled: !ctx.canRegenerate,
          isToggle: false,
          toggled: false,
        };
      case "swap_places":
        return {
          handler: ctx.onToggleSwapPlaces,
          disabled: false,
          isToggle: true,
          toggled: ctx.swapPlacesActive,
        };
      case "new_session":
        return {
          handler: ctx.onNewSession,
          disabled: !ctx.character,
          isToggle: false,
          toggled: false,
        };
      case "continue":
        return {
          handler: ctx.onContinue,
          disabled: !ctx.canContinue,
          isToggle: false,
          toggled: false,
        };
      case "abort":
        return {
          handler: ctx.onAbort,
          disabled: !ctx.isGenerating,
          isToggle: false,
          toggled: false,
        };
      case "view_history":
        return {
          handler: ctx.onViewHistory,
          disabled: !ctx.character,
          isToggle: false,
          toggled: false,
        };
      case "open_memories":
        return {
          handler: ctx.onOpenMemories,
          disabled: !ctx.character || !ctx.session,
          isToggle: false,
          toggled: false,
        };
      case "open_search":
        return {
          handler: ctx.onOpenSearch,
          disabled: !ctx.character,
          isToggle: false,
          toggled: false,
        };
      case "toggle_voice_autoplay":
        return {
          handler: ctx.onToggleVoiceAutoplay,
          disabled: !ctx.session,
          isToggle: true,
          toggled: ctx.voiceAutoplayActive,
        };
    }
  }, [ctx, node.action]);

  const label = node.title ?? t(DEFAULT_LABEL[node.action]);
  return (
    <section className="flex flex-col gap-1.5">
      <button
        type="button"
        onClick={() => void handler()}
        disabled={disabled}
        className={cn(
          "flex items-center justify-between gap-2 rounded-lg px-3 py-2.5 text-left text-sm",
          interactive.transition.fast,
          disabled
            ? "cursor-not-allowed border border-fg/8 bg-fg/[0.03] text-fg/30"
            : isToggle && toggled
              ? "border border-accent/40 bg-accent/15 text-accent"
              : cn(
                  widgetCardClass(hasBackground, node.design),
                  "text-fg/80 hover:border-fg/30 hover:bg-fg/10",
                  interactive.active.scale,
                ),
        )}
      >
        <span className="min-w-0 flex-1 truncate">{label}</span>
        <span className="shrink-0">
          <ActionIcon action={node.action} />
        </span>
      </button>
      {node.description && (
        <p className="px-0.5 text-[11px] leading-snug text-fg/45">{node.description}</p>
      )}
    </section>
  );
}
