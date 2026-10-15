import { useMemo, useRef, useState, type ReactNode } from "react";
import { useQuery } from "@tanstack/react-query";
import { useRouteContext } from "@tanstack/react-router";
import { AnimatePresence, motion } from "framer-motion";
import { GalleryVerticalEnd, House, List, Search, User } from "lucide-react";
import { useTranslation } from "react-i18next";
import {
  Button,
  CharacterCardSkeleton,
  cn,
  EmptyState,
  icon,
  IconButton,
  MediaCardSkeleton,
  Menu,
  MenuContent,
  MenuItem,
  MenuSeparator,
  MenuTrigger,
  motion as m,
  Page,
  Skeleton,
  useCompactLayout,
} from "@lettuceai/crisp";
import { useChatNavigation } from "@/features/conversations";
import { appStatusQuery } from "@/api/app-status-query";
import { FailureState } from "@/shared/ui/FailureState";
import { MobileSettingsButton } from "@/shared/ui/MobileSettingsButton";
import { useSidebarOnDetailPages } from "@/shared/hooks/useSidebarOnDetailPages";
import { ContinueCard, GalleryCard, HeroCard } from "./components/cards";
import { CharacterMenu } from "./components/CharacterMenu";
import { ChatMenu, type ChatMenuTarget } from "./components/ChatMenu";
import { HomeCharacterCard } from "./components/HomeCharacterCard";
import { LayoutSheet } from "./components/LayoutSheet";
import { layoutIcons } from "./components/layout-icons";
import { RecentList } from "./components/RecentList";
import { homeCharactersQuery, matchesSearch, type HomeCharacter } from "./data/home-characters";
import { desktopViews, useDesktopView, usePhoneLayout, type DesktopView } from "./data/layout";

const viewIcons: Record<DesktopView, ReactNode> = {
  home: <House size={icon.lg} />,
  list: <List size={icon.lg} />,
  hero: <GalleryVerticalEnd size={icon.lg} />,
};

function RowSkeletons({ count }: { count: number }) {
  return (
    <div className="space-y-2">
      {Array.from({ length: count }, (_, index) => <CharacterCardSkeleton key={index} />)}
    </div>
  );
}

const gridClass = "relative grid grid-cols-2 gap-3 @3xl:grid-cols-3 @5xl:grid-cols-4 @6xl:grid-cols-5";

function Cards({ items, render }: { items: readonly HomeCharacter[]; render: (home: HomeCharacter) => ReactNode }) {
  return (
    <AnimatePresence mode="popLayout" initial={false}>
      {items.map((home) => (
        <motion.div key={home.character.id} layout exit={{ opacity: 0, scale: 0.97 }} transition={m.settle}>
          {render(home)}
        </motion.div>
      ))}
    </AnimatePresence>
  );
}

export function HomePage() {
  const { t } = useTranslation("home");
  const api = useRouteContext({ from: "__root__", select: (context) => context.api });
  const { data: status } = useQuery(appStatusQuery(api));
  const phone = useCompactLayout() || status?.platform === "android" || status?.platform === "ios";
  const mainRef = useRef<HTMLElement>(null);
  const [query, setQuery] = useState("");
  const [layoutSheet, setLayoutSheet] = useState(false);
  const [menuFor, setMenuFor] = useState<HomeCharacter | null>(null);
  const [chatMenu, setChatMenu] = useState<ChatMenuTarget | null>(null);
  const chats = useChatNavigation();
  const [desktopView, setDesktopView] = useDesktopView(api);
  const detailSidebar = useSidebarOnDetailPages(api);
  const [phoneLayout, setPhoneLayout] = usePhoneLayout(api);
  const characters = useQuery(homeCharactersQuery(api));
  const all = characters.data;
  const shown = useMemo(() => (all ?? []).filter((home) => matchesSearch(home, query)), [all, query]);
  const byId = useMemo(() => new Map((all ?? []).map((home) => [home.character.id, home.character])), [all]);
  const searching = query.trim().length > 0;
  const layout = phone ? phoneLayout : desktopView === "home" ? "gallery" : desktopView;
  const count = all ? (searching ? t("count", { shown: shown.length, total: all.length }) : String(all.length)) : undefined;

  const characterMenu = (home: HomeCharacter) => ({ onOpen: () => void chats.openCharacter(home.character.id, home.latest?.id ?? null), onMenu: () => setMenuFor(home), held: menuFor?.character.id === home.character.id });
  const recent = (
    <RecentList api={api} characters={byId} search={query} onOpen={(chat) => chats.openConversation(chat.id)} heldId={chatMenu?.chat.id ?? null} onMenu={(chat, character) => setChatMenu({ chat, character })} />
  );
  const menus = (
    <>
      <CharacterMenu api={api} home={menuFor} onClose={() => setMenuFor(null)} />
      <ChatMenu api={api} target={chatMenu} onClose={() => setChatMenu(null)} />
    </>
  );

  const showView = (next: DesktopView) => {
    setDesktopView(next);
    mainRef.current?.scrollTo({ top: 0 });
  };

  let body: ReactNode;
  if (characters.isPending) {
    body =
      layout === "gallery" ? (
        <div className={gridClass}>{Array.from({ length: 10 }, (_, index) => <MediaCardSkeleton key={index} />)}</div>
      ) : layout === "hero" ? (
        <div className="space-y-3">
          <Skeleton shape="block" height={phone ? 256 : 288} className="rounded-3xl" />
          <div className="grid gap-3 @4xl:grid-cols-2">{Array.from({ length: 4 }, (_, index) => <CharacterCardSkeleton key={index} />)}</div>
        </div>
      ) : layout === "recent" ? (
        recent
      ) : (
        <div className={cn(!phone && "mx-auto max-w-5xl")}>
          <RowSkeletons count={5} />
        </div>
      );
  } else if (characters.isError) {
    body = <FailureState error={characters.error} onRetry={() => void characters.refetch()} />;
  } else if (layout === "recent") {
    body = recent;
  } else if (shown.length === 0) {
    body = searching ? (
      <EmptyState
        icon={<Search size={icon.xl} />}
        title={t("noMatch.title", { query: query.trim() })}
        description={t("noMatch.body")}
        action={<Button variant="secondary" onClick={() => setQuery("")}>{t("noMatch.clear")}</Button>}
      />
    ) : (
      <EmptyState icon={<User size={icon.xl} />} title={t("empty.title")} description={t("empty.body")} />
    );
  } else if (layout === "gallery") {
    body = (
      <div className={gridClass}>
        <Cards items={shown} render={(home) => <GalleryCard home={home} {...characterMenu(home)} />} />
      </div>
    );
  } else if (layout === "hero") {
    const [first, ...rest] = shown;
    body = (
      <div className="space-y-3">
        {first && <HeroCard api={api} home={first} compact={phone} {...characterMenu(first)} />}
        <div className="relative grid gap-3 @4xl:grid-cols-2">
          <Cards items={rest} render={(home) => <HomeCharacterCard api={api} character={home.character} {...characterMenu(home)} />} />
        </div>
      </div>
    );
  } else {
    body = (
      <div className={cn("relative space-y-2", !phone && "mx-auto max-w-5xl")}>
        <Cards items={shown} render={(home) => <HomeCharacterCard api={api} character={home.character} {...characterMenu(home)} />} />
      </div>
    );
  }

  const viewSwitch = (
    <Menu>
      <MenuTrigger>
        <IconButton label={t("view.current", { view: t(`view.${desktopView}`) })}>{viewIcons[desktopView]}</IconButton>
      </MenuTrigger>
      <MenuContent align="end">
        {desktopViews.map((view) => (
          <MenuItem key={view} icon={viewIcons[view]} selected={view === desktopView} onSelect={() => showView(view)}>
            {t(`view.${view}`)}
          </MenuItem>
        ))}
        <MenuSeparator />
        <MenuItem selected={detailSidebar.enabled} disabled={detailSidebar.pending} onSelect={() => detailSidebar.set(!detailSidebar.enabled)}>
          {t("view.detailSidebar")}
        </MenuItem>
      </MenuContent>
    </Menu>
  );

  if (!phone && desktopView === "home") {
    const continuing = searching ? undefined : all?.find((home) => home.latest !== null);
    return (
      <Page
        title={t("homeTitle")}
        searchValue={query}
        onSearchChange={setQuery}
        searchPlaceholder={t("searchCharacters")}
        actions={<>{viewSwitch}<MobileSettingsButton /></>}
        mainRef={mainRef}
        padding="none"
        contentClassName="space-y-8 px-6 pb-10 pt-2"
      >
        {characters.isPending ? (
          <Skeleton shape="block" height={256} className="rounded-3xl" />
        ) : (
          continuing?.latest && (
            <ContinueCard
              api={api}
              home={continuing}
              chat={continuing.latest}
              held={chatMenu?.chat.id === continuing.latest.id}
              onOpen={() => chats.openConversation(continuing.latest!.id)}
              onMenu={() => continuing.latest && setChatMenu({ chat: continuing.latest, character: continuing.character })}
            />
          )
        )}
        <section aria-labelledby="characters-heading">
          <div className="mb-4 flex items-baseline gap-3">
            <h2 id="characters-heading" className="text-xl font-semibold tracking-tight text-fg">{t("characters")}</h2>
            {count && <span className="text-sm tabular-nums text-fg-3">{count}</span>}
          </div>
          {body}
        </section>
        {menus}
      </Page>
    );
  }

  return (
    <Page
      title={t("title")}
      meta={searching ? count : undefined}
      searchValue={query}
      onSearchChange={setQuery}
      searchPlaceholder={t("search")}
      mainRef={mainRef}
      padding="none"
      contentClassName={cn("pb-10 pt-1", phone ? "px-3" : "px-6")}
      actions={
        <>{phone ? (
          <IconButton label={t("layout.current", { layout: t(`layout.${phoneLayout}.title`) })} onClick={() => setLayoutSheet(true)}>
            {layoutIcons[phoneLayout]}
          </IconButton>
        ) : (
          viewSwitch
        )}<MobileSettingsButton /></>
      }
    >
      {body}
      {menus}
      <LayoutSheet
        open={layoutSheet}
        value={phoneLayout}
        onClose={() => setLayoutSheet(false)}
        onChange={(next) => {
          setPhoneLayout(next);
          mainRef.current?.scrollTo({ top: 0 });
        }}
      />
    </Page>
  );
}
