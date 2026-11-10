import type { ReactNode } from "react";
import { ArrowLeftRight, Check, Cpu, FastForward, History, MessageSquarePlus, NotebookPen, RefreshCw, Search, Sparkles, Square, User } from "lucide-react";
import { useTranslation } from "react-i18next";
import { cn, icon, ListRowButton, Skeleton } from "@lettuceai/crisp";
import type { LibraryButtonAction } from "@/api/generated/bindings";
import { useChatModels } from "../data/use-models";
import { useWidgets } from "./context";
import type { WidgetNode } from "./nodes";
import { BoxScope, WidgetCard } from "./WidgetCard";

type Node<T extends WidgetNode["type"]> = Extract<WidgetNode, { type: T }>;

export function DividerWidget({ node }: { node: Node<"divider"> }) {
  return node.style === "space" ? <div aria-hidden className="h-4" /> : <div aria-hidden className="h-px w-full bg-line" />;
}

export function BoxWidget({ node, children }: { node: Node<"box">; children: ReactNode }) {
  return (
    <WidgetCard tint={node.variant ?? "default"} title={node.title} description={node.description}>
      {node.children.length > 0 && (
        <BoxScope value>
          <div className={node.children.every((child) => child.type === "button") ? "space-y-1.5" : "space-y-3"}>{children}</div>
        </BoxScope>
      )}
    </WidgetCard>
  );
}

const actionIcon = {
  regenerate: RefreshCw,
  swap_places: ArrowLeftRight,
  new_session: MessageSquarePlus,
  continue: FastForward,
  abort: Square,
  view_history: History,
  open_memories: Sparkles,
  open_search: Search,
  toggle_voice_autoplay: ArrowLeftRight,
} satisfies Record<LibraryButtonAction, unknown>;

export function ButtonWidget({ node }: { node: Node<"button"> }) {
  const { t } = useTranslation("conversations");
  const widgets = useWidgets();
  const Icon = actionIcon[node.action];
  const labels = {
    regenerate: t("widgets.button.regenerateLastReply"),
    swap_places: t("widgets.button.swapPlaces"),
    new_session: t("widgets.button.newSession"),
    continue: t("widgets.button.continue"),
    abort: t("widgets.button.stopGenerating"),
    view_history: t("widgets.button.viewChatHistory"),
    open_memories: t("widgets.button.memories"),
    open_search: t("widgets.button.searchChat"),
    toggle_voice_autoplay: t("widgets.button.voiceAutoplay"),
  } satisfies Record<LibraryButtonAction, string>;
  const wait = t("widgets.button.waiting");
  const run = {
    regenerate: { act: widgets.regenerate, off: !widgets.canRegenerate && (widgets.sending ? wait : t("widgets.button.noReply")) },
    swap_places: { act: widgets.toggleSwap, off: widgets.isGroup && t("widgets.button.directOnly") },
    new_session: { act: widgets.newChat, off: widgets.sending && wait },
    continue: { act: widgets.continueReply, off: !widgets.canContinue && (widgets.sending ? wait : t("widgets.button.noReply")) },
    abort: { act: widgets.stop, off: !widgets.sending && t("widgets.button.nothingToStop") },
    view_history: { act: widgets.openHistory, off: false },
    open_memories: { act: widgets.openMemories, off: false },
    open_search: { act: widgets.openSearch, off: false },
    toggle_voice_autoplay: { act: () => undefined, off: t("widgets.button.voiceAutoplay") },
  } satisfies Record<LibraryButtonAction, { act: () => void; off: string | false }>;
  const { act, off } = run[node.action];
  const active = node.action === "swap_places" && widgets.swap;
  return (
    <div {...(off ? { title: off } : {})}>
      <WidgetCard design={node.design} flush framed>
        <ListRowButton
          title={node.title || labels[node.action]}
          {...(node.description ? { description: node.description } : {})}
          trailing={active ? <Check size={icon.md} className="text-accent" /> : <Icon size={icon.md} />}
          chevron={false}
          disabled={off !== false}
          aria-pressed={node.action === "swap_places" ? widgets.swap : undefined}
          onClick={act}
          className={cn(active && "text-accent")}
        />
      </WidgetCard>
    </div>
  );
}

export function SelectorWidget({ node }: { node: Node<"selector"> }) {
  const { t } = useTranslation("conversations");
  const widgets = useWidgets();
  const models = useChatModels(widgets.api);
  const { settings } = widgets;
  let title: string;
  let value: string | undefined;
  let Icon = User;
  let open = widgets.openPersona;
  if (node.kind === "persona") {
    title = t("widgets.selector.persona");
    value = widgets.personaLoading ? t("persona.loading") : (widgets.persona?.name ?? t("persona.none"));
  } else if (node.kind === "model") {
    title = t("widgets.selector.model");
    Icon = Cpu;
    open = widgets.openModel;
    value = widgets.isGroup ? t("widgets.selector.perMember") : settings ? (models.find(settings.model.id)?.name ?? t("settings.model.none")) : undefined;
  } else {
    title = t("widgets.selector.authorNote");
    Icon = NotebookPen;
    open = widgets.openNote;
    value = settings ? settings.author_note?.trim() || t("note.inlinePlaceholder") : undefined;
  }
  return (
    <WidgetCard design={node.design} description={node.description} flush>
      <ListRowButton
        icon={<Icon size={icon.lg} />}
        title={node.title || title}
        description={value ?? <Skeleton shape="text" width={96} />}
        disabled={node.kind === "model" && widgets.isGroup}
        onClick={open}
      />
    </WidgetCard>
  );
}

export function ImageWidget({ node }: { node: Node<"image"> }) {
  const { t } = useTranslation("conversations");
  const { character, members, persona, assets } = useWidgets();
  const source = node.source;
  let url: string | null = null;
  let name = node.title ?? t("widgets.image.alt");
  if (source.kind === "character_avatar") {
    const person = character ?? members[0];
    url = person?.avatar ?? null;
    name = person?.name ?? name;
  } else if (source.kind === "persona_avatar") {
    url = persona?.avatar ?? null;
    name = persona?.name ?? name;
  } else if (source.kind === "logical_asset") {
    url = assets.get(source.asset_id) ?? null;
  }
  const shape = node.shape ?? "auto";
  return (
    <WidgetCard design={node.design} title={node.title} description={node.description}>
      <div className={cn("overflow-hidden", shape === "circle" ? "mx-auto aspect-square w-1/2 rounded-full" : "rounded-lg")}>
        {url ? (
          <img src={url} alt={name} className={cn("block w-full object-cover", shape === "auto" ? "h-auto" : "h-full", shape === "square" && "aspect-square", shape === "wide" && "aspect-video", shape === "circle" && "aspect-square")} />
        ) : (
          <div className={cn("flex items-center justify-center text-sm text-fg-3", shape === "circle" ? "aspect-square" : "aspect-video")}>
            {t("widgets.image.noImage")}
          </div>
        )}
      </div>
    </WidgetCard>
  );
}
