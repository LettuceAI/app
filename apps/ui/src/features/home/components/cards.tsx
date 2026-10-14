import type { ReactNode } from "react";
import { MessageSquare } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Avatar, Button, cn, icon, InlineMarkdown, MediaCard, relativeTime, Surface, useLongPress } from "@lettuceai/crisp";
import type { ApiClient } from "@/api/client";
import type { ConversationSummary } from "@/api/generated/bindings";
import { useCardColors } from "../data/gradient";
import type { HomeCharacter } from "../data/home-characters";

const coverFocus = "50% 32%";

export interface MenuProps {
  onOpen: () => void;
  onMenu: () => void;
  held: boolean;
}

export function GalleryCard({ home, onOpen, onMenu, held }: { home: HomeCharacter } & MenuProps) {
  const { t, i18n } = useTranslation("home");
  const { character } = home;
  const press = useLongPress(onMenu);
  return (
    <MediaCard
      title={character.name}
      description={character.description ? <InlineMarkdown>{character.description}</InlineMarkdown> : t("noDescription")}
      image={character.avatar?.url ?? null}
      meta={home.latest ? relativeTime(home.latest.updated_at, undefined, i18n.language) : undefined}
      aria-haspopup="dialog"
      {...press.handlers}
      onClick={press.guardClick(onOpen)}
      className={cn(held && "ring-2 ring-inset ring-fg/40")}
    />
  );
}

function Cover({ api, home, image, className, children, onMenu, held }: { api: ApiClient; home: HomeCharacter; image?: string | null; className: string; children: ReactNode } & Omit<MenuProps, "onOpen">) {
  const colors = useCardColors(api, home.character);
  const press = useLongPress(onMenu);
  const banner = image ?? home.character.banner?.url ?? null;
  return (
    <div {...press.handlers} className={cn("overflow-hidden rounded-3xl ring-1 ring-inset ring-line-2", held && "ring-2 ring-fg/40")}>
      <Surface image={banner} imagePosition={coverFocus} tint={banner ? null : (colors?.css ?? null)} scrim="auto" className={cn("flex items-end", className)}>
        {children}
      </Surface>
    </div>
  );
}

export function HeroCard({ api, home, compact, onOpen, onMenu, held }: { api: ApiClient; home: HomeCharacter; compact: boolean } & MenuProps) {
  const { t, i18n } = useTranslation("home");
  const { character } = home;
  const lastChat = home.latest ? t("lastChat", { when: relativeTime(home.latest.updated_at, undefined, i18n.language).toLowerCase() }) : null;
  const description = character.description ? <InlineMarkdown>{character.description}</InlineMarkdown> : t("noDescription");
  return (
    <Cover api={api} home={home} className={compact ? "min-h-64" : "min-h-72"} onMenu={onMenu} held={held}>
      {compact ? (
        <div className="w-full space-y-4 bg-gradient-to-t from-black/60 to-transparent p-5 pt-20">
          <div className="flex items-center gap-3.5">
            <Avatar src={character.avatar?.url ?? null} name={character.name} size="lg" className="ring-2 ring-white/20" />
            <div className="min-w-0">
              {lastChat && <p className="text-2xs text-fg-2">{lastChat}</p>}
              <h2 className="truncate text-2xl font-bold tracking-tight text-fg">{character.name}</h2>
            </div>
          </div>
          <p className="line-clamp-2 text-sm text-fg-2">{description}</p>
          <Button variant="primary" className="w-full" onClick={onOpen} leading={<MessageSquare size={icon.md} />}>
            {t("continueChat")}
          </Button>
        </div>
      ) : (
        <div className="flex w-full flex-wrap items-end justify-between gap-6 bg-gradient-to-t from-black/55 to-transparent p-8 pt-24">
          <div className="flex min-w-0 items-center gap-5">
            <Avatar src={character.avatar?.url ?? null} name={character.name} size="xl" className="ring-2 ring-white/20" />
            <div className="min-w-0 max-w-xl">
              {lastChat && <p className="text-sm text-fg-2">{lastChat}</p>}
              <h2 className="mt-1 truncate text-4xl font-bold tracking-tight text-fg">{character.name}</h2>
              <p className="mt-2 line-clamp-2 text-base text-fg-2">{description}</p>
            </div>
          </div>
          <Button variant="primary" size="lg" onClick={onOpen} leading={<MessageSquare size={icon.md} />}>
            {t("continueChat")}
          </Button>
        </div>
      )}
    </Cover>
  );
}

/** The chat you were last in, with its character's art: where the desktop home starts. */
export function ContinueCard({ api, home, chat, onOpen, onMenu, held }: { api: ApiClient; home: HomeCharacter; chat: ConversationSummary } & MenuProps) {
  const { t, i18n } = useTranslation("home");
  const { character } = home;
  return (
    <Cover api={api} home={home} image={chat.background?.url ?? null} className="min-h-64" onMenu={onMenu} held={held}>
      <div className="flex w-full flex-wrap items-end justify-between gap-6 bg-gradient-to-t from-black/65 via-black/25 to-transparent px-8 pb-7 pt-20">
        <div className="flex min-w-0 max-w-2xl flex-1 items-end gap-5">
          <Avatar src={character.avatar?.url ?? null} name={character.name} size="xl" className="ring-2 ring-white/20" />
          <div className="min-w-0 flex-1">
            <p className="text-sm text-fg-2">{t("continueMeta", { name: character.name, when: relativeTime(chat.updated_at, undefined, i18n.language).toLowerCase() })}</p>
            <h2 className="mt-0.5 truncate text-3xl font-bold tracking-tight text-fg">{chat.title}</h2>
            {chat.last_message_preview && <p className="mt-2 line-clamp-2 text-base leading-snug text-fg-2"><InlineMarkdown>{chat.last_message_preview}</InlineMarkdown></p>}
          </div>
        </div>
        <Button variant="primary" size="lg" onClick={onOpen} leading={<MessageSquare size={icon.md} />}>
          {t("continueChat")}
        </Button>
      </div>
    </Cover>
  );
}
