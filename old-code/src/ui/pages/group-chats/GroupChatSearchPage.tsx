import { useState, useCallback, useRef } from "react";
import { ArrowLeft, Loader2, X, Search } from "lucide-react";
import { useNavigate, useParams } from "react-router-dom";
import { components, colors, interactive, radius, cn } from "../../design-tokens";
import { storageBridge } from "../../../core/storage/files";
import { Routes, useNavigationManager } from "../../navigation";
import { useI18n } from "../../../core/i18n/context";
import { useGroupChatLayoutContext } from "./GroupChatLayout";

interface GroupSearchResult {
  messageId: string;
  content: string;
  createdAt: number;
  role: string;
  speakerCharacterId: string | null;
}

export function GroupChatSearchPage() {
  const navigate = useNavigate();
  const { groupSessionId } = useParams<{ groupSessionId: string }>();
  const { backOrReplace } = useNavigationManager();
  const { t } = useI18n();
  const { characters } = useGroupChatLayoutContext();

  const [query, setQuery] = useState("");
  const [results, setResults] = useState<GroupSearchResult[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const searchTimeoutRef = useRef<number | null>(null);

  const handleSearch = useCallback(
    async (searchQuery: string) => {
      if (!groupSessionId || !searchQuery.trim()) {
        setResults([]);
        return;
      }
      setLoading(true);
      setError(null);
      try {
        const data = await storageBridge.groupSearchMessages(groupSessionId, searchQuery);
        setResults(data);
      } catch (err) {
        console.error("Group search failed:", err);
        setError(t("chats.search.failed"));
      } finally {
        setLoading(false);
      }
    },
    [groupSessionId, t],
  );

  const onQueryChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const newQuery = e.target.value;
    setQuery(newQuery);
    if (searchTimeoutRef.current) {
      window.clearTimeout(searchTimeoutRef.current);
    }
    searchTimeoutRef.current = window.setTimeout(() => {
      handleSearch(newQuery);
    }, 300);
  };

  const speakerName = (result: GroupSearchResult) => {
    if (result.role === "user") return t("chats.search.you");
    if (result.speakerCharacterId) {
      const c = characters.find((ch) => ch.id === result.speakerCharacterId);
      if (c) return c.name;
    }
    return t("chats.search.character");
  };

  const highlightMatch = (text: string, highlight: string) => {
    if (!highlight.trim()) return text;
    const parts = text.split(
      new RegExp(`(${highlight.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")})`, "gi"),
    );
    return (
      <span>
        {parts.map((part, i) =>
          part.toLowerCase() === highlight.toLowerCase() ? (
            <span key={i} className="bg-warning/30 text-warning rounded px-0.5 font-medium">
              {part}
            </span>
          ) : (
            part
          ),
        )}
      </span>
    );
  };

  return (
    <div className={cn("flex h-screen flex-col", colors.surface.base, colors.text.primary)}>
      <div
        className={cn(
          "flex items-center gap-3 border-b pl-3 pb-3 pt-[calc(env(safe-area-inset-top)+12px)] shrink-0 z-20",
          "pr-3",
          colors.glass.strong,
        )}
      >
        <button
          onClick={() =>
            backOrReplace(groupSessionId ? Routes.groupChat(groupSessionId) : Routes.groupChats)
          }
          className={cn(
            "flex shrink-0 items-center justify-center h-8 w-8",
            radius.full,
            "border bg-fg/5",
            colors.border.subtle,
            colors.text.primary,
            interactive.hover.brightness,
            interactive.active.scale,
          )}
          aria-label={t("chats.header.back")}
        >
          <ArrowLeft size={14} />
        </button>
        <div className="flex-1 relative">
          <Search className="absolute left-3 top-1/2 -translate-y-1/2 text-fg/40" size={16} />
          <input
            type="text"
            autoFocus
            value={query}
            onChange={onQueryChange}
            placeholder={t("chats.search.placeholder")}
            className={cn(
              "w-full pl-10 pr-10 py-2.5 text-fg placeholder:text-fg/40",
              components.input.base,
              radius.lg,
            )}
          />
          {query && (
            <button
              onClick={() => {
                setQuery("");
                setResults([]);
                if (searchTimeoutRef.current) clearTimeout(searchTimeoutRef.current);
              }}
              className="absolute right-3 top-1/2 -translate-y-1/2 text-fg/40 hover:text-fg"
            >
              <X size={16} />
            </button>
          )}
        </div>
      </div>

      <div className="flex-1 overflow-y-auto px-3 py-4">
        {loading ? (
          <div className="flex justify-center py-10">
            <Loader2 className="animate-spin text-fg/30" size={24} />
          </div>
        ) : error ? (
          <div className="text-center text-danger py-10">{error}</div>
        ) : results.length === 0 && query.trim() ? (
          <div className="text-center text-fg/30 py-10">{t("chats.search.noMessagesFound")}</div>
        ) : (
          <div className="space-y-4">
            {results.map((result) => (
              <button
                key={result.messageId}
                onClick={() => {
                  if (!groupSessionId) return;
                  navigate(
                    `${Routes.groupChat(groupSessionId)}?jumpToMessage=${encodeURIComponent(
                      result.messageId,
                    )}`,
                  );
                }}
                className={cn(
                  "w-full text-left p-4 space-y-2 border border-fg/10 bg-fg/5 hover:bg-fg/10 transition",
                  radius.lg,
                )}
              >
                <div className="flex justify-between items-center text-xs text-fg/40 uppercase font-medium tracking-wider">
                  <span>{speakerName(result)}</span>
                  <span>{new Date(result.createdAt).toLocaleDateString()}</span>
                </div>
                <p className="text-sm text-fg/80 line-clamp-3 leading-relaxed">
                  {highlightMatch(result.content, query)}
                </p>
              </button>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
