import { Fragment, useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { useInfiniteQuery, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useBlocker, useNavigate, useParams, useRouter, useSearch } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { ChevronDown, FileText, History, Library, Pencil, Trash2, Wand2 } from "lucide-react";
import {
  AppBar,
  Badge,
  Button,
  Chip,
  CopyButton,
  IconButton,
  EmptyState,
  icon,
  Menu,
  MenuContent,
  MenuItem,
  MenuLabel,
  MenuTrigger,
  Resizable,
  Segmented,
  Sheet,
  Skeleton,
  Tab,
  TabList,
  Tabs,
  Tooltip,
  cn,
} from "@lettuceai/crisp";
import type { LorebookEntriesMutateRequest, LorebookEntryView } from "@/api/generated/bindings";
import { useApi } from "@/api/use-api";
import { queryKeys } from "@/api/query-keys";
import { usePageLayout } from "@/shared/hooks/usePageLayout";
import { FailureState } from "@/shared/ui/FailureState";
import { literalSpans, type Inspection, type WordSpan } from "./highlights";
import { SavedLorebookEntryEditor } from "./SavedLorebookEntryEditor";

function pieces(
  text: string,
  spans: WordSpan[],
  render: (text: string, span: WordSpan, index: number) => ReactNode,
) {
  const result: ReactNode[] = [];
  let cursor = 0;
  spans.forEach((span, index) => {
    result.push(
      text.slice(cursor, span.start),
      render(text.slice(span.start, span.end), span, index),
    );
    cursor = span.end;
  });
  result.push(text.slice(cursor));
  return result;
}

function Words({
  text,
  entries,
  messageId,
  focus,
  onPick,
  onHover,
}: {
  text: string;
  entries: Inspection[];
  messageId: string;
  focus: string | null;
  onPick: (id: string) => void;
  onHover: (id: string | null) => void;
}) {
  const { t } = useTranslation("lorebook-preview");
  return (
    <>
      {pieces(text, literalSpans(text, entries, messageId), (word, span, index) => (
        <Menu key={index}>
          <MenuTrigger>
            <Button
              variant="ghost"
              className={cn(
                "inline h-auto min-h-0 rounded px-0 py-0 font-normal text-inherit",
                !focus || span.entries.some((item) => item.entry.id === focus)
                  ? "bg-accent/25 ring-1 ring-accent/35"
                  : "bg-fill-2",
              )}
              onPointerEnter={() => onHover(span.entries[0]?.entry.id ?? null)}
              onPointerLeave={() => onHover(null)}
            >
              {word}
            </Button>
          </MenuTrigger>
          <MenuContent>
            <MenuLabel>{t("wordEntries", { count: span.entries.length })}</MenuLabel>
            {span.entries.map((item) => (
              <MenuItem
                key={item.entry.id}
                icon={<FileText size={icon.sm} />}
                onSelect={() => onPick(item.entry.id)}
              >
                {item.entry.title || t("untitled")}
              </MenuItem>
            ))}
          </MenuContent>
        </Menu>
      ))}
    </>
  );
}

function LoadingPanel() {
  return (
    <div role="status" className="space-y-5 p-4">
      {[0, 1, 2].map((row) => (
        <div key={row} className="space-y-2">
          <Skeleton className="h-4 w-32" />
          <Skeleton className="h-4 w-full" />
          <Skeleton className="h-4 w-3/4" />
        </div>
      ))}
    </div>
  );
}

export function LorebookPreviewPage() {
  const { t } = useTranslation("lorebook-preview");
  const api = useApi();
  const client = useQueryClient();
  const navigate = useNavigate();
  const router = useRouter();
  const { phone } = usePageLayout();
  const { lorebookId } = useParams({ from: "/library/lorebooks/$lorebookId/preview" });
  const search = useSearch({ from: "/library/lorebooks/$lorebookId/preview" });
  const mode = search.mode ?? "conversation";
  const [phoneTab, setPhoneTab] = useState<"content" | "entries">("content");
  const [text, setText] = useState("");
  const [debouncedText, setDebouncedText] = useState("");
  const [selected, setSelected] = useState<string | null>(null);
  const [hover, setHover] = useState<string | null>(null);
  const [injectionOpen, setInjectionOpen] = useState(false);
  const [editing, setEditing] = useState<LorebookEntryView | null>(null);
  const [editorOpen, setEditorOpen] = useState(false);
  const [editorSession, setEditorSession] = useState(0);
  const [deleting, setDeleting] = useState<LorebookEntryView | null>(null);
  const [deleteOpen, setDeleteOpen] = useState(false);
  const deleteRequest = useRef<LorebookEntriesMutateRequest | null>(null);
  const deletion = useMutation({
    mutationFn: () => api.call("lorebookEntriesMutate", deleteRequest.current!),
    onSuccess: async () => {
      setDeleteOpen(false);
      setSelected(null);
      await client.invalidateQueries({ queryKey: queryKeys.lorebooks.all });
    },
  });
  useBlocker({
    shouldBlockFn: ({ action }) => {
      if (!deleteOpen) return false;
      if (action === "BACK") {
        if (!deletion.isPending) setDeleteOpen(false);
        return true;
      }
      return deletion.isPending;
    },
    enableBeforeUnload: deleteOpen && deletion.isPending,
  });
  const inspectorRef = useRef<HTMLDivElement>(null);
  const chatRef = useRef<HTMLDivElement>(null);
  const chatScroll = useRef<{
    source: string;
    pages: number;
    height: number;
    top: number;
  } | null>(null);
  const overlayRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const timeout = setTimeout(() => setDebouncedText(text), 200);
    return () => clearTimeout(timeout);
  }, [text]);
  const book = useQuery({
    queryKey: [...queryKeys.lorebooks.all, "detail", lorebookId, "preview"],
    queryFn: () => api.call("lorebookGet", { lorebook_id: lorebookId }),
  });
  const chats = useInfiniteQuery({
    queryKey: [...queryKeys.conversations.lists(), "lorebook-preview"],
    initialPageParam: null as string | null,
    queryFn: ({ pageParam }) =>
      api.call("conversationsList", {
        lifecycle: "active",
        kind: null,
        character_id: null,
        source_group_id: null,
        cursor: pageParam,
        limit: 50,
      }),
    getNextPageParam: (page) => page.next_cursor,
    enabled: mode === "conversation",
  });
  const conversations = chats.data?.pages.flatMap((page) => page.items) ?? [];
  const chat = search.chat
    ? conversations.find((value) => value.id === search.chat)
    : conversations[0];
  const conversationId = search.chat ?? chat?.id;
  const messages = useInfiniteQuery({
    queryKey: [...queryKeys.conversations.messages(conversationId ?? ""), "lorebook-preview"],
    initialPageParam: null as string | null,
    queryFn: ({ pageParam }) =>
      api.call("conversationMessages", {
        conversation_id: conversationId!,
        before_cursor: pageParam,
        limit: 100,
      }),
    getNextPageParam: (page) => page.next_cursor,
    enabled: mode === "conversation" && Boolean(conversationId),
  });
  const previewKey = [
    ...queryKeys.lorebooks.all,
    "trigger-preview",
    lorebookId,
    mode,
    conversationId ?? null,
    mode === "editor" ? debouncedText : "",
  ];
  const preview = useQuery({
    queryKey: previewKey,
    queryFn: () =>
      api.call(
        "lorebookTriggerPreview",
        mode === "editor"
          ? { mode: "editor", lorebook_id: lorebookId, text: debouncedText }
          : {
              mode: "conversation",
              conversation_id: conversationId!,
              composer_text: null,
              speaker_character_id: null,
            },
      ),
    enabled: Boolean(book.data) && (mode === "editor" || Boolean(conversationId)),
  });
  const changeSource = (next: "conversation" | "editor", chatId = search.chat) => {
    setSelected(null);
    setHover(null);
    setPhoneTab("content");
    void navigate({
      to: "/library/lorebooks/$lorebookId/preview",
      params: { lorebookId },
      search: { mode: next, ...(chatId ? { chat: chatId } : {}) },
      replace: true,
    });
  };
  const timeline = messages.data?.pages.toReversed().flatMap((page) => page.items) ?? [];
  const chatSource = `${conversationId}:${mode}:${phoneTab}`;
  const messagePages = messages.data?.pages.length ?? 0;
  useLayoutEffect(() => {
    const node = chatRef.current;
    if (!node) return;
    const previous = chatScroll.current;
    if (!previous || previous.source !== chatSource || previous.pages === 0) {
      node.scrollTop = node.scrollHeight;
    } else if (messagePages > previous.pages) {
      node.scrollTop = previous.top + node.scrollHeight - previous.height;
    }
    chatScroll.current = {
      source: chatSource,
      pages: messagePages,
      height: node.scrollHeight,
      top: node.scrollTop,
    };
  }, [chatSource, messagePages, messages.dataUpdatedAt]);
  const matches = preview.data?.entries.filter((entry) => entry.lorebook_id === lorebookId) ?? [];
  const byId = new Map(book.data?.entries.map((entry) => [entry.id, entry]));
  const active: Inspection[] = matches.flatMap((match) => {
    const entry = byId.get(match.entry_id);
    return entry ? [{ entry, match }] : [];
  });
  const activeIds = new Set(active.map((item) => item.entry.id));
  const inactive: Inspection[] = (book.data?.entries ?? [])
    .filter((entry) => !activeIds.has(entry.id))
    .map((entry) => ({ entry, match: undefined }));
  const inspections = [...active, ...inactive];
  const focus = hover ?? selected;
  const totalTokens = matches.reduce((total, match) => total + match.token_count, 0);
  const injection = active
    .map(({ entry }) => entry.content.trim())
    .filter(Boolean)
    .join("\n\n");
  const scanned = new Set(preview.data?.scanned_message_ids ?? []);
  const firstScanned = timeline.findIndex((message) => scanned.has(message.id));
  const unused =
    mode === "conversation" &&
    preview.data &&
    !preview.data.active_lorebook_ids.includes(lorebookId);
  const pick = (id: string) => {
    setSelected(id);
    if (phone) setPhoneTab("entries");
    requestAnimationFrame(() => {
      const row = Array.from(
        inspectorRef.current?.querySelectorAll<HTMLElement>("[data-entry-id]") ?? [],
      ).find((element) => element.dataset.entryId === id);
      row?.scrollIntoView({ block: "nearest", behavior: "smooth" });
    });
  };
  const reason = ({ entry, match }: Inspection) => {
    if (match?.always_active) return t("alwaysReason");
    if (match)
      return t(mode === "conversation" ? "matchedChat" : "matchedCompose", {
        keywords: match.matched_keywords.slice(0, 3).join(", "),
        count: match.matched_message_ids.length,
      });
    if (!entry.enabled) return t("offReason");
    if (!entry.keywords.length) return t("noKeywordsReason");
    return t("waitingReason", {
      keywords: entry.keywords.slice(0, 3).join(", "),
      remaining:
        entry.keywords.length > 3 ? t("moreKeywords", { count: entry.keywords.length - 3 }) : "",
    });
  };
  const section = (label: string, items: Inspection[], isActive = false) =>
    items.length > 0 && (
      <section>
        <div
          className={cn(
            "sticky top-0 z-1 flex items-center gap-2 border-b border-line bg-surface px-4 py-1.5 text-xs font-medium",
            isActive ? "text-accent" : "text-fg-3",
          )}
        >
          <span>{t("sectionCount", { label, count: items.length })}</span>
          {isActive && (
            <span className="ml-auto text-fg-3">{t("tokens", { count: totalTokens })}</span>
          )}
        </div>
        <div className="divide-y divide-line">
          {items.map((item) => {
            const { entry, match } = item;
            const open = selected === entry.id;
            const status = match
              ? match.always_active
                ? "alwaysStatus"
                : "matchedStatus"
              : !entry.enabled
                ? "offStatus"
                : !entry.keywords.length
                  ? "noKeywordsStatus"
                  : "noMatchStatus";
            return (
              <div
                key={entry.id}
                data-entry-id={entry.id}
                onPointerEnter={() => setHover(entry.id)}
                onPointerLeave={() => setHover(null)}
                className={cn("relative", (open || hover === entry.id) && "bg-fill")}
              >
                {focus === entry.id && match && (
                  <span className="absolute inset-y-0 left-0 w-0.5 bg-accent" />
                )}
                <Button
                  variant="ghost"
                  className="h-auto w-full min-w-0 justify-start rounded-none px-4 py-3 text-left [&>span]:w-full [&>span]:min-w-0"
                  aria-expanded={open}
                  onClick={() => setSelected(open ? null : entry.id)}
                >
                  <span className="min-w-0 flex-1">
                    <span className="flex min-w-0 items-center gap-2">
                      <span className="min-w-0 truncate text-sm font-medium text-fg">
                        {entry.title || t("untitled")}
                      </span>
                      <Badge className="shrink-0" tone={match ? (match.always_active ? "info" : "accent") : "neutral"}>
                        {t(status)}
                      </Badge>
                    </span>
                    <span className="mt-1 block truncate text-xs font-normal text-fg-3">
                      {reason(item)}
                    </span>
                  </span>
                  {match && (
                    <span className="shrink-0 text-xs font-normal text-fg-3">
                      {t("tokens", { count: match.token_count })}
                    </span>
                  )}
                  <ChevronDown
                    size={icon.sm}
                    className={cn("shrink-0 text-fg-3 transition-transform", open && "rotate-180")}
                  />
                </Button>
                {open && (
                  <div className="space-y-3 px-4 pb-4">
                    <div className="flex flex-wrap gap-1.5">
                      {entry.keywords.map((keyword) => (
                        <Chip
                          key={keyword}
                          selected={Boolean(match?.matched_keywords.includes(keyword))}
                        >
                          {keyword}
                        </Chip>
                      ))}
                    </div>
                    <p className="line-clamp-4 whitespace-pre-wrap text-sm text-fg-2">
                      {entry.content}
                    </p>
                    <div className="flex items-center justify-end gap-2">
                      <IconButton
                        label={t("deleteEntry")}
                        variant="ghost"
                        className="mr-auto text-danger"
                        onClick={() => {
                          if (!book.data) return;
                          setDeleting(entry);
                          deleteRequest.current = {
                            lorebook_id: lorebookId,
                            expected_revision: book.data.lorebook.revision,
                            client_operation_id: crypto.randomUUID(),
                            mutations: [{ type: "remove", entry_id: entry.id }],
                          };
                          deletion.reset();
                          setDeleteOpen(true);
                        }}
                      >
                        <Trash2 size={icon.sm} />
                      </IconButton>
                      <Button
                        size="sm"
                        variant="secondary"
                        leading={<Pencil size={icon.sm} />}
                        onClick={() => {
                          setEditing(entry);
                          setEditorSession((value) => value + 1);
                          setEditorOpen(true);
                        }}
                      >
                        {t("edit")}
                      </Button>
                    </div>
                  </div>
                )}
              </div>
            );
          })}
        </div>
      </section>
    );
  const previewState =
    mode === "conversation" && chats.isError ? (
      <FailureState error={chats.error} onRetry={() => void chats.refetch()} />
    ) : mode === "conversation" && chats.isSuccess && !conversationId ? (
      <EmptyState
        icon={<History size={icon.xl} />}
        title={t("noChatsTitle")}
        description={t("noChatsBody")}
      />
    ) : preview.isError ? (
      <FailureState error={preview.error} onRetry={() => void preview.refetch()} />
    ) : preview.isPending || preview.isFetching || (mode === "editor" && text !== debouncedText) ? (
      <LoadingPanel />
    ) : unused ? (
      <EmptyState
        icon={<Library size={icon.xl} />}
        title={t("unusedTitle")}
        description={t("unusedBody")}
      />
    ) : null;
  const inspector = (
    <aside className="flex h-full min-h-0 flex-col bg-surface-1/40">
      <div ref={inspectorRef} className="min-h-0 flex-1 overflow-y-auto">
        {previewState ??
          (!book.data?.entries.length ? (
            <EmptyState
              icon={<Library size={icon.xl} />}
              title={t("noEntriesTitle")}
              description={t("noEntriesBody")}
              className="pt-16"
            />
          ) : (
            <>
              {section(t("comesIn"), active, true)}
              {section(t("staysOut"), inactive)}
            </>
          ))}
      </div>
      <div
        className={cn("border-t border-line bg-surface", phone && "pb-[var(--safe-bottom,0px)]")}
      >
        <Button
          variant="ghost"
          className="h-auto w-full justify-start rounded-none px-4 py-2.5"
          aria-expanded={injectionOpen}
          onClick={() => setInjectionOpen((value) => !value)}
          leading={<FileText size={icon.sm} />}
        >
          <span className="text-sm font-medium">{t("whatAdded")}</span>
          <span className="ml-auto text-xs font-normal text-fg-3">
            {t("addedSummary", { count: active.length, tokens: totalTokens })}
          </span>
          <ChevronDown
            size={icon.sm}
            className={cn("transition-transform", !injectionOpen && "rotate-180")}
          />
        </Button>
        {injectionOpen && (
          <div className="space-y-2 border-t border-line px-4 py-3">
            {injection && !previewState ? (
              <>
                <pre className="max-h-40 overflow-auto whitespace-pre-wrap break-words rounded-lg border border-line bg-surface-1 px-3 py-2.5 font-mono text-xs leading-relaxed text-fg-2">
                  {injection}
                </pre>
                <div className="flex items-center justify-between gap-2">
                  <span className="text-xs text-fg-3">{t("injectionHint")}</span>
                  <CopyButton size="sm" variant="secondary" value={injection}>
                    {t("copy")}
                  </CopyButton>
                </div>
              </>
            ) : (
              <p className="py-2 text-center text-sm text-fg-3">{t("nothingAdded")}</p>
            )}
          </div>
        )}
      </div>
    </aside>
  );
  const chatPanel = (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex flex-wrap items-center gap-2 border-b border-line px-3 py-2">
        <Menu>
          <MenuTrigger>
            <Button
              variant="ghost"
              size="sm"
              leading={<History size={icon.sm} />}
              trailing={<ChevronDown size={icon.sm} />}
            >
              <span className="truncate">{chat?.title || t("chooseChat")}</span>
              {!phone && chat && (
                <span className="shrink-0 text-xs font-normal text-fg-3">
                  {t("messageCount", { count: chat.message_count })}
                </span>
              )}
            </Button>
          </MenuTrigger>
          <MenuContent>
            <MenuLabel>{t("recentChats")}</MenuLabel>
            {conversations.map((value) => (
              <MenuItem
                key={value.id}
                selected={value.id === conversationId}
                onSelect={() => changeSource("conversation", value.id)}
              >
                <span className="flex flex-col">
                  <span>{value.title || t("untitledChat")}</span>
                  <span className="text-xs text-fg-3">
                    {t("messageCount", { count: value.message_count })}
                  </span>
                </span>
              </MenuItem>
            ))}
            {chats.hasNextPage && (
              <MenuItem
                disabled={chats.isFetchingNextPage}
                onSelect={() => void chats.fetchNextPage()}
              >
                {t("moreChats")}
              </MenuItem>
            )}
          </MenuContent>
        </Menu>
        {preview.data && (
          <Tooltip content={t("scanHint")}>
            <span className="ml-auto shrink-0 text-xs text-fg-3">
              {t("scanDepth", { count: preview.data.scan_depth })}
            </span>
          </Tooltip>
        )}
      </div>
      <div
        ref={chatRef}
        className="min-h-0 flex-1 overflow-y-auto px-4 pb-6 pt-1"
        onScroll={(event) => {
          chatScroll.current = {
            source: chatSource,
            pages: messagePages,
            height: event.currentTarget.scrollHeight,
            top: event.currentTarget.scrollTop,
          };
        }}
      >
        {chats.isError ? (
          <FailureState error={chats.error} onRetry={() => void chats.refetch()} />
        ) : chats.isPending ? (
          <LoadingPanel />
        ) : !conversationId ? (
          <EmptyState
            icon={<History size={icon.xl} />}
            title={t("noChatsTitle")}
            description={t("noChatsBody")}
            action={
              <Button variant="secondary" onClick={() => changeSource("editor")}>
                {t("compose")}
              </Button>
            }
          />
        ) : messages.isError ? (
          <FailureState error={messages.error} onRetry={() => void messages.refetch()} />
        ) : messages.isPending ? (
          <LoadingPanel />
        ) : !timeline.length ? (
          <EmptyState
            icon={<History size={icon.xl} />}
            title={t("noMessagesTitle")}
            description={t("noMessagesBody")}
          />
        ) : (
          <>
            {messages.hasNextPage && (
              <Button
                variant="ghost"
                size="sm"
                loading={messages.isFetchingNextPage}
                onClick={() => void messages.fetchNextPage()}
              >
                {t("olderMessages")}
              </Button>
            )}
            {timeline.map((message, index) => (
              <Fragment key={message.id}>
                {index === firstScanned && index > 0 && (
                  <div className="my-2 flex items-center gap-3 text-xs font-medium text-accent">
                    <span className="h-px flex-1 bg-accent/30" />
                    {t("scannedFrom")}
                    <span className="h-px flex-1 bg-accent/30" />
                  </div>
                )}
                <article
                  data-message-id={message.id}
                  data-scanned={scanned.has(message.id)}
                  className={cn(
                    "relative border-b border-line py-4 pl-3",
                    preview.data && !scanned.has(message.id) && "opacity-45",
                  )}
                >
                  {inspections.some(
                    (item) =>
                      item.entry.id === focus &&
                      item.match?.matched_message_ids.includes(message.id),
                  ) && <span className="absolute inset-y-4 left-0 w-0.5 rounded-full bg-accent" />}
                  <div
                    className={cn(
                      "mb-2 text-xs font-medium",
                      message.role === "user" ? "text-accent" : "text-fg-3",
                    )}
                  >
                    {t(`roles.${message.role}`)}
                  </div>
                  <div
                    className={cn(
                      "whitespace-pre-wrap break-words text-sm leading-relaxed text-fg-2",
                      message.role === "scene" && "italic",
                    )}
                  >
                    <Words
                      text={message.parts
                        .filter((part) => part.type === "text")
                        .map((part) => part.text)
                        .join("\n")}
                      entries={previewState ? [] : inspections}
                      messageId={message.id}
                      focus={focus}
                      onPick={pick}
                      onHover={setHover}
                    />
                  </div>
                </article>
              </Fragment>
            ))}
          </>
        )}
      </div>
    </div>
  );
  const composeSpans = literalSpans(text, previewState ? [] : inspections, null);
  const composePanel = (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex items-center gap-2 border-b border-line px-4 py-2.5 text-xs text-fg-3">
        <span>{t("composeHint")}</span>
        <span className="ml-auto shrink-0">{t("matches", { count: active.length })}</span>
      </div>
      <div className="min-h-0 flex-1 p-4">
        <div className="relative h-full min-h-48 overflow-hidden rounded-xl border border-line bg-surface-1 focus-within:border-accent/50">
          <div
            ref={overlayRef}
            aria-hidden="true"
            className="pointer-events-none absolute inset-0 overflow-hidden whitespace-pre-wrap break-words px-4 py-3 text-sm leading-relaxed text-transparent"
          >
            {pieces(text, composeSpans, (word, span, index) => (
              <mark
                key={index}
                className={cn(
                  "rounded text-transparent",
                  !focus || span.entries.some((item) => item.entry.id === focus)
                    ? "bg-accent/25 ring-1 ring-accent/35"
                    : "bg-fill-2",
                )}
              >
                {word}
              </mark>
            ))}
            {String.fromCharCode(8203)}
          </div>
          <textarea
            value={text}
            onChange={(event) => setText(event.target.value)}
            onScroll={(event) => {
              if (overlayRef.current) overlayRef.current.scrollTop = event.currentTarget.scrollTop;
            }}
            spellCheck={false}
            aria-label={t("textLabel")}
            placeholder={t("textPlaceholder")}
            className="relative h-full w-full resize-none bg-transparent px-4 py-3 text-sm leading-relaxed text-fg caret-fg outline-none placeholder:text-fg-4"
          />
        </div>
      </div>
    </div>
  );
  const sourcePanel = mode === "conversation" ? chatPanel : composePanel;
  return (
    <div className="relative flex h-full min-h-0 flex-col">
      <AppBar
        title={t("title")}
        meta={phone ? undefined : book.data?.lorebook.name}
        onBack={() => {
          if (editorOpen) setEditorOpen(false);
          else if (router.history.canGoBack()) router.history.back();
          else void navigate({ to: "/library", search: { item: lorebookId, kind: "lorebook" } });
        }}
        backLabel={t("back")}
        filters={
          phone ? (
            <Tabs
              value={phoneTab === "entries" ? "entries" : mode}
              onChange={(value) => {
                if (value === "entries") setPhoneTab("entries");
                else changeSource(value as "conversation" | "editor");
              }}
            >
              <TabList>
                <Tab value="conversation">{t("chat")}</Tab>
                <Tab value="editor">{t("compose")}</Tab>
                <Tab value="entries">
                  {t("entriesTab", { count: active.length, total: book.data?.entries.length ?? 0 })}
                </Tab>
              </TabList>
            </Tabs>
          ) : (
            <div className="flex flex-wrap items-center gap-3">
              <Segmented
                size="sm"
                aria-label={t("source")}
                value={mode}
                onChange={changeSource}
                options={[
                  { value: "conversation", label: t("fromChat"), icon: <History size={icon.sm} /> },
                  { value: "editor", label: t("compose"), icon: <Wand2 size={icon.sm} /> },
                ]}
              />
              <span className="ml-auto text-sm text-fg-2">
                {t("summary", {
                  count: active.length,
                  total: book.data?.entries.length ?? 0,
                  tokens: totalTokens,
                })}
              </span>
            </div>
          )
        }
      />
      <div className="min-h-0 flex-1 border-t border-line">
        {book.isError ? (
          <FailureState error={book.error} onRetry={() => void book.refetch()} />
        ) : book.isPending ? (
          <LoadingPanel />
        ) : phone ? (
          phoneTab === "entries" ? (
            inspector
          ) : (
            sourcePanel
          )
        ) : (
          <Resizable
            side="right"
            defaultSize={420}
            minSize={340}
            maxSize={720}
            panel={inspector}
            className="h-full"
          >
            {sourcePanel}
          </Resizable>
        )}
      </div>
      <Sheet
        open={deleteOpen}
        onClose={() => {
          if (!deletion.isPending) setDeleteOpen(false);
        }}
        dismissible={!deletion.isPending}
        size="sm"
        title={t("deleteTitle", { title: deleting?.title || t("untitled") })}
        description={t("deleteBody")}
        footer={
          <div className="flex justify-end gap-2">
            <Button
              variant="ghost"
              disabled={deletion.isPending}
              onClick={() => setDeleteOpen(false)}
            >
              {t("keep")}
            </Button>
            <Button variant="danger" loading={deletion.isPending} onClick={() => deletion.mutate()}>
              {t("delete")}
            </Button>
          </div>
        }
      >
        {deletion.isError && (
          <FailureState error={deletion.error} onRetry={() => deletion.mutate()} size="sm" />
        )}
      </Sheet>
      {editing && book.data && (
        <SavedLorebookEntryEditor
          key={`${editing.id}:${editorSession}`}
          book={book.data}
          entry={editing}
          open={editorOpen}
          phone={phone}
          onClose={() => setEditorOpen(false)}
          onSaved={() => {
            void client.invalidateQueries({
              queryKey: [...queryKeys.lorebooks.all, "trigger-preview", lorebookId],
            });
          }}
        />
      )}
    </div>
  );
}
