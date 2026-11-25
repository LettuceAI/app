import { DraftMenu } from "./lorebook-create/DraftMenu";
import { projectsQuery } from "./data/lorebook-projects";
import { AssetAudio } from "@/shared/ui/AssetAudio";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useInfiniteQuery, useQuery } from "@tanstack/react-query";
import {
  useLocation,
  useNavigate,
  useRouteContext,
  useRouter,
  useSearch,
} from "@tanstack/react-router";
import { BookOpen, FileAudio, Image, MoreHorizontal, User, UserRound } from "lucide-react";
import { useTranslation } from "react-i18next";
import {
  Button,
  cn,
  FilterBar,
  icon,
  IconButton,
  MediaCard,
  MediaTile,
  Page,
  relativeTime,
  Spinner,
  useLongPress,
} from "@lettuceai/crisp";
import { FailureState } from "@/shared/ui/FailureState";
import { MobileSettingsButton } from "@/shared/ui/MobileSettingsButton";
import type { MediaLibraryItem } from "@/api/generated/bindings";
import { mediaQuery } from "@/api/media-library-query";
import { mediaName } from "./data/actions";
import {
  countsQuery,
  kindEntriesQuery,
  type Entry,
  type Filter,
  type LibrarySearch,
} from "./data/queries";
import { EntryMenu } from "./EntryMenu";
import { EntryEditor } from "./EntryEditor";
import { MediaMenu } from "./MediaMenu";
import { LibraryCreateSheet } from "./LibraryCreateSheet";
import { EmptyBody, entryGrid, imageGrid, LoadingBody } from "./LibraryBody";
import { LibraryViewport } from "./LibraryViewport";

const tones = { character: "info", persona: "secondary", lorebook: "warning" } as const;
const kindFilters = {
  character: "characters",
  persona: "personas",
  lorebook: "lorebooks",
} as const;

function EntryCard({ entry, held, onOpen }: { entry: Entry; held: boolean; onOpen: () => void }) {
  const { t } = useTranslation("library");
  const press = useLongPress(onOpen);
  return (
    <MediaCard
      title={entry.name}
      description={entry.description}
      image={entry.image}
      kind={t(`kinds.${entry.kind}`)}
      tone={tones[entry.kind]}
      tag={entry.isDefault ? t("you") : entry.draft ? t("draft") : undefined}
      aria-haspopup="dialog"
      {...press.handlers}
      onClick={press.guardClick(onOpen)}
      className={cn("transition-shadow motion-quick", held && "ring-2 ring-inset ring-fg/40")}
    />
  );
}
function PictureTile({ item, onOpen }: { item: MediaLibraryItem; onOpen: () => void }) {
  const { t } = useTranslation("library");
  const press = useLongPress(onOpen);
  return (
    <MediaTile
      src={item.asset.preview_url ?? item.asset.url}
      alt={mediaName(item)}
      {...(item.asset_kind === "background_image" ? { kind: t("menu.background") } : {})}
      aria-haspopup="dialog"
      {...press.handlers}
      onClick={press.guardClick(onOpen)}
    />
  );
}

export function LibraryPage() {
  const { t, i18n } = useTranslation("library");
  const api = useRouteContext({ from: "__root__", select: (context) => context.api });
  const navigate = useNavigate();
  const router = useRouter();
  const search = useSearch({ from: "/library" });
  const pushed = useLocation({ select: (location) => location.state.libraryOverlayPushed });
  const routeKey = useLocation({ select: (location) => location.state.__TSR_key });
  const mainRef = useRef<HTMLElement>(null);
  const endRef = useRef<HTMLDivElement>(null);
  const filter = useLocation({
    select: ({ state }) => state.libraryFilter ?? (state.libraryKind ? kindFilters[state.libraryKind] : "all"),
  });
  const [query, setQuery] = useState("");
  const [term, setTerm] = useState("");
  const [createMenu, setCreateMenu] = useState(false);
  useEffect(() => {
    const timer = window.setTimeout(() => {
      setTerm(query.trim());
      mainRef.current?.scrollTo({ top: 0 });
    }, 250);
    return () => window.clearTimeout(timer);
  }, [query]);
  const counts = useQuery(countsQuery(api));
  const projects = useQuery(projectsQuery(api));
  const draftCount = projects.data?.length;
  const totals = counts.data;
  const mediaView = filter === "images" || filter === "audio";
  const charactersQuery = useInfiniteQuery({
    ...kindEntriesQuery(api, "character", term),
    enabled: filter === "all" || filter === "characters",
  });
  const personasQuery = useInfiniteQuery({
    ...kindEntriesQuery(api, "persona", term),
    enabled: filter === "all" || filter === "personas",
  });
  const lorebooksQuery = useInfiniteQuery({
    ...kindEntriesQuery(api, "lorebook", term),
    enabled: filter === "all" || filter === "lorebooks",
  });
  const media = useInfiniteQuery({
    ...mediaQuery(api, filter === "audio" ? "audio" : "image", term),
    enabled: mediaView,
  });
  const activeEntityQueries = useMemo(
    () =>
      filter === "characters"
        ? [charactersQuery]
        : filter === "personas"
          ? [personasQuery]
          : filter === "lorebooks"
            ? [lorebooksQuery]
            : filter === "all"
              ? [charactersQuery, personasQuery, lorebooksQuery]
              : [],
    [filter, charactersQuery, personasQuery, lorebooksQuery],
  );
  const entries = useMemo(
    () => [
      ...(filter === "all" || filter === "lorebooks"
        ? (projects.data ?? [])
            .filter(
              (project) =>
                !term ||
                `${project.lorebook_name ?? ""} ${project.brief}`
                  .toLocaleLowerCase()
                  .includes(term.toLocaleLowerCase()),
            )
            .map(
              (project): Entry => ({
                kind: "lorebook",
                id: project.project_id,
                name: project.lorebook_name ?? t("untitledDraft"),
                description: project.brief,
                image: null,
                draft: project,
              }),
            )
        : []),
      ...activeEntityQueries.flatMap((entryQuery) =>
        entryQuery.data?.pages.flatMap((page) => page.items) ?? [],
      ),
    ],
    [activeEntityQueries, projects.data, filter, term, t],
  );
  const cachedEntries = useMemo(
    () =>
      [charactersQuery, personasQuery, lorebooksQuery].flatMap(
        (entryQuery) => entryQuery.data?.pages.flatMap((page) => page.items) ?? [],
      ),
    [charactersQuery, personasQuery, lorebooksQuery],
  );
  const pictures = useMemo(
    () => media.data?.pages.flatMap((page) => page.items) ?? [],
    [media.data],
  );
  const hasNextPage = mediaView
    ? media.hasNextPage
    : activeEntityQueries.some((entryQuery) => entryQuery.hasNextPage);
  const isFetching = mediaView
    ? media.isFetching
    : activeEntityQueries.some((entryQuery) => entryQuery.isFetching);
  const isFetchingNextPage = mediaView
    ? media.isFetchingNextPage
    : activeEntityQueries.some((entryQuery) => entryQuery.isFetchingNextPage);
  const isFetchNextPageError = mediaView
    ? media.isFetchNextPageError
    : activeEntityQueries.some((entryQuery) => entryQuery.isFetchNextPageError);
  const fetchNextPage = useCallback(() => {
    if (mediaView) return media.fetchNextPage();
    return Promise.all(
      activeEntityQueries
        .filter((entryQuery) => entryQuery.hasNextPage && !entryQuery.isFetching)
        .map((entryQuery) => entryQuery.fetchNextPage()),
    );
  }, [mediaView, media, activeEntityQueries]);
  const refetchActive = () => {
    if (mediaView) return media.refetch();
    const errored = activeEntityQueries.filter((entryQuery) => entryQuery.isError);
    const targets = errored.length > 0 ? errored : activeEntityQueries;
    return Promise.all(targets.map((entryQuery) => entryQuery.refetch()));
  };
  const typing = query.trim() !== term;
  const pending =
    (mediaView
      ? media.isPending
      : activeEntityQueries.some((entryQuery) => entryQuery.isPending)) ||
    typing ||
    ((filter === "all" || filter === "lorebooks") && projects.isPending);
  const failed = mediaView
    ? media.isError && !media.data
    : activeEntityQueries.some((entryQuery) => entryQuery.isError && !entryQuery.data);
  const activeError = mediaView
    ? media.error
    : activeEntityQueries.find((entryQuery) => entryQuery.isError)?.error;
  const pages = mediaView
    ? (media.data?.pages.length ?? 0)
    : activeEntityQueries.reduce((sum, entryQuery) => sum + (entryQuery.data?.pages.length ?? 0), 0);
  useEffect(() => {
    const end = endRef.current;
    const root = mainRef.current;
    if (
      !end ||
      !root ||
      !hasNextPage ||
      isFetching ||
      isFetchNextPageError ||
      typing ||
      typeof IntersectionObserver === "undefined"
    )
      return;
    const observer = new IntersectionObserver(
      (observed) => {
        if (observed.some((item) => item.isIntersecting)) void fetchNextPage();
      },
      { root },
    );
    observer.observe(end);
    return () => observer.disconnect();
  }, [hasNextPage, isFetching, isFetchNextPageError, fetchNextPage, pages, filter, typing]);
  const changeFilter = (next: Filter) => {
    void navigate({
      to: "/library",
      search: true,
      state: (previous) => ({ ...previous, libraryFilter: next }),
      replace: true,
      resetScroll: false,
    });
    mainRef.current?.scrollTo({ top: 0 });
  };
  const open = (next: LibrarySearch) => {
    if (next.create === "character") void navigate({ to: "/characters/new" });
    else if (next.create === "persona") void navigate({ to: "/personas/new" });
    else if (next.create === "lorebook")
      void navigate({ to: "/library/lorebooks/new", state: { lorebookFlowPushed: true } });
    else
      void navigate({
        to: "/library",
        search: next,
        state: { libraryOverlayPushed: true, libraryFilter: filter },
        resetScroll: false,
      });
  };
  const close = () => {
    if (pushed) router.history.back();
    else void navigate({
      to: "/library",
      search: {},
      state: { libraryFilter: filter },
      replace: true,
      resetScroll: false,
    });
  };
  const listedEntry = entries.find(
    (entry) => !entry.draft && entry.kind === search.kind && entry.id === search.item,
  );
  const requestedLorebook = useQuery({
    queryKey: ["lorebooks", "detail", search.item, "library"],
    queryFn: async () => ({
      kind: "lorebook" as const,
      view: await api.call("lorebookGet", { lorebook_id: search.item! }),
    }),
    enabled: Boolean(search.kind === "lorebook" && search.item && !listedEntry),
  });
  const loadedBook = requestedLorebook.data?.view.lorebook;
  const activeEntry =
    listedEntry ??
    (search.kind === "lorebook" && loadedBook
      ? {
          kind: "lorebook" as const,
          id: loadedBook.id,
          name: loadedBook.name,
          description: "",
          image: loadedBook.icon?.preview_url ?? loadedBook.icon?.url ?? null,
        }
      : undefined);
  const activeMedia = pictures.find((item) => item.asset.asset_id === search.media);
  const active = search.create
    ? { key: routeKey, type: "create" as const, kind: search.create }
    : activeEntry
      ? { key: routeKey, type: "entry" as const, entry: activeEntry }
      : activeMedia
        ? { key: routeKey, type: "media" as const, item: activeMedia }
        : null;
  const [retained, setRetained] = useState(active);
  if (active && (active.key !== retained?.key || active.type !== retained?.type))
    setRetained(active);
  const defaultPersona = personasQuery.data?.pages.find((page) => page.defaultPersona)?.defaultPersona;
  const options = [
    {
      value: "all" as const,
      label: t("filters.all"),
      count:
        totals && draftCount !== undefined
          ? totals.characters + totals.personas + totals.lorebooks + draftCount
          : undefined,
    },
    {
      value: "characters" as const,
      label: t("filters.characters"),
      count: totals?.characters,
      icon: <User size={icon.sm} />,
    },
    {
      value: "personas" as const,
      label: t("filters.personas"),
      count: totals?.personas,
      icon: <UserRound size={icon.sm} />,
    },
    {
      value: "lorebooks" as const,
      label: t("filters.lorebooks"),
      count: totals && draftCount !== undefined ? totals.lorebooks + draftCount : undefined,
      icon: <BookOpen size={icon.sm} />,
    },
    {
      value: "images" as const,
      label: t("filters.images"),
      count: totals?.images,
      icon: <Image size={icon.sm} />,
    },
    {
      value: "audio" as const,
      label: t("filters.audio"),
      count: totals?.audio,
      icon: <FileAudio size={icon.sm} />,
    },
  ].map(({ count, ...option }) => ({ ...option, ...(count === undefined ? {} : { count }) }));
  const showing = mediaView ? pictures.length : entries.length;
  let body;
  if (failed) {
    body = <FailureState error={activeError} onRetry={() => void refetchActive()} />;
  } else if (pending) {
    body = <LoadingBody filter={filter} />;
  } else if (showing === 0) {
    body = (
      <EmptyBody
        filter={filter}
        query={query.trim()}
        onClear={() => setQuery("")}
        onEverything={() => changeFilter("all")}
        onCreate={(kind) => open({ create: kind })}
        onImport={() => setCreateMenu(true)}
      />
    );
  } else if (filter === "images") {
    body = (
      <LibraryViewport
        items={pictures}
        itemKey={(item) => item.asset.asset_id}
        scrollRef={mainRef}
        gridClass={imageGrid}
        aspectRatio={1}
      >
        {(item) => <PictureTile item={item} onOpen={() => open({ media: item.asset.asset_id })} />}
      </LibraryViewport>
    );
  } else if (filter === "audio") {
    body = (
      <LibraryViewport
        items={pictures}
        itemKey={(item) => item.asset.asset_id}
        scrollRef={mainRef}
        gridClass="grid max-w-3xl grid-cols-1 gap-2"
      >
        {(item) => (
          <AssetAudio
            title={mediaName(item)}
            description={relativeTime(item.created_at, undefined, i18n.language)}
            badge={{
              label:
                item.asset_kind === "synthesized_speech" ? t("menu.generated") : t("menu.uploaded"),
              tone: item.asset_kind === "synthesized_speech" ? "secondary" : "info",
            }}
            src={item.asset.url}
            actions={
              <IconButton
                label={t("menu.more", { name: mediaName(item) })}
                size="sm"
                onClick={() => open({ media: item.asset.asset_id })}
              >
                <MoreHorizontal size={icon.md} />
              </IconButton>
            }
          />
        )}
      </LibraryViewport>
    );
  } else {
    body = (
      <LibraryViewport
        items={entries}
        itemKey={(entry) => `${entry.kind}:${entry.id}`}
        scrollRef={mainRef}
        gridClass={entryGrid}
        aspectRatio={3 / 4}
      >
        {(entry) => (
          <EntryCard
            entry={entry}
            held={entry.id === activeEntry?.id && entry.kind === activeEntry.kind}
            onOpen={() =>
              open(entry.draft ? { draft: entry.id } : { item: entry.id, kind: entry.kind })
            }
          />
        )}
      </LibraryViewport>
    );
  }
  return (
    <>
      <Page
        title={t("title")}
        searchValue={query}
        onSearchChange={setQuery}
        searchPlaceholder={t("search")}
        mainRef={mainRef}
        actions={<MobileSettingsButton />}
        filters={
          <FilterBar
            aria-label={t("show")}
            options={options}
            value={filter}
            onChange={changeFilter}
          />
        }
      >
        {counts.isError && (
          <FailureState error={counts.error} onRetry={() => void counts.refetch()} size="sm" />
        )}
        {projects.isError && (filter === "all" || filter === "lorebooks") && (
          <FailureState error={projects.error} onRetry={() => void projects.refetch()} size="sm" />
        )}
        {requestedLorebook.isError && (
          <FailureState
            error={requestedLorebook.error}
            onRetry={() => void requestedLorebook.refetch()}
            size="sm"
          />
        )}
        {body}
        {!pending && !failed && (
          <div ref={endRef} className="flex justify-center py-4">
            {isFetchNextPageError ? (
              <FailureState error={activeError} onRetry={() => void fetchNextPage()} size="sm" />
            ) : isFetchingNextPage ? (
              <Spinner />
            ) : hasNextPage ? (
              <Button variant="ghost" onClick={() => void fetchNextPage()}>
                {t("loadMore")}
              </Button>
            ) : null}
          </div>
        )}
      </Page>
      {retained?.type === "entry" && (
        <EntryMenu
          key={retained.key}
          api={api}
          entry={retained.entry}
          defaultPersona={defaultPersona}
          open={active?.type === "entry"}
          onClose={close}
        />
      )}
      {retained?.type === "media" && (
        <MediaMenu
          key={retained.key}
          api={api}
          item={retained.item}
          entries={cachedEntries}
          open={active?.type === "media"}
          onClose={close}
        />
      )}
      {retained?.type === "create" && retained.kind === "lorebook" && (
        <EntryEditor
          key={retained.key}
          api={api}
          kind={retained.kind}
          open={active?.type === "create"}
          onClose={close}
          onCreated={(kind) => {
            setQuery("");
            setTerm("");
            changeFilter(kindFilters[kind]);
          }}
        />
      )}
      {search.draft && <DraftMenu key={search.draft} id={search.draft} onClose={close} />}
      <LibraryCreateSheet api={api} open={createMenu} onClose={() => setCreateMenu(false)} />
    </>
  );
}
