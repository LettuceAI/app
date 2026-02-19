import { useState, useCallback, useRef } from "react";
import { ArrowLeft, Loader2, X, Search } from "lucide-react";
import { useNavigate, useSearchParams, useParams } from "react-router-dom";
import { components, colors, interactive, radius, cn } from "../../design-tokens";
import { storageBridge } from "../../../core/storage/files";
import { Routes, useNavigationManager } from "../../navigation";

interface SearchResult {
    messageId: string;
    content: string;
    createdAt: number;
    role: string;
}

export function SearchMessagesPage() {
    const navigate = useNavigate();
    const { characterId } = useParams<{ characterId: string }>();
    const { backOrReplace } = useNavigationManager();
    const [searchParams] = useSearchParams();
    const sessionId = searchParams.get("sessionId");

    const [query, setQuery] = useState("");
    const [results, setResults] = useState<SearchResult[]>([]);
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState<string | null>(null);

    const searchTimeoutRef = useRef<number | null>(null);

    const handleSearch = useCallback(async (searchQuery: string) => {
        if (!sessionId || !searchQuery.trim()) {
            setResults([]);
            return;
        }

        setLoading(true);
        setError(null);
        try {
            const data = await storageBridge.searchMessages(sessionId, searchQuery);
            setResults(data);
        } catch (err) {
            console.error("Search failed:", err);
            setError("Failed to search messages");
        } finally {
            setLoading(false);
        }
    }, [sessionId]);

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

    const highlightMatch = (text: string, highlight: string) => {
        if (!highlight.trim()) return text;
        const parts = text.split(new RegExp(`(${highlight.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')})`, 'gi'));
        return (
            <span>
                {parts.map((part, i) =>
                    part.toLowerCase() === highlight.toLowerCase() ? (
                        <span key={i} className="bg-warning/30 text-warning rounded px-0.5 font-medium">{part}</span>
                    ) : (
                        part
                    )
                )}
            </span>
        );
    };

    return (
        <div className={cn("flex h-screen flex-col", colors.surface.base, colors.text.primary)}>
            {/* Header */}
            <div className={cn(
                "flex items-center gap-3 border-b px-3 pb-3 pt-[calc(env(safe-area-inset-top)+12px)] sticky top-0 z-20",
                colors.glass.strong
            )}>
                <button
                    onClick={() => backOrReplace(characterId ? Routes.chatSession(characterId, sessionId) : Routes.chat)}
                    className={cn(
                        "flex shrink-0 items-center justify-center h-8 w-8",
                        radius.full,
                        "border bg-white/5",
                        colors.border.subtle,
                        colors.text.primary,
                        interactive.hover.brightness,
                        interactive.active.scale,
                        interactive.focus.ring
                    )}
                    aria-label="Back"
                >
                    <ArrowLeft size={14} />
                </button>
                <div className="flex-1 relative">
                    <Search className="absolute left-3 top-1/2 -translate-y-1/2 text-white/40" size={16} />
                    <input
                        type="text"
                        autoFocus
                        value={query}
                        onChange={onQueryChange}
                        placeholder="Search conversation..."
                        className={cn(
                            "w-full pl-10 pr-10 py-2.5 placeholder:text-white/30",
                            components.input.base,
                            radius.lg
                        )}
                    />
                    {query && (
                        <button
                            onClick={() => {
                                setQuery("");
                                setResults([]);
                                if (searchTimeoutRef.current) clearTimeout(searchTimeoutRef.current);
                            }}
                            className="absolute right-3 top-1/2 -translate-y-1/2 text-white/40 hover:text-white"
                        >
                            <X size={16} />
                        </button>
                    )}
                </div>
            </div>

            {/* Results */}
            <div className="flex-1 overflow-y-auto px-3 py-4">
                {loading ? (
                    <div className="flex justify-center py-10">
                        <Loader2 className="animate-spin text-white/30" size={24} />
                    </div>
                ) : error ? (
                    <div className="text-center text-danger py-10">{error}</div>
                ) : results.length === 0 && query.trim() ? (
                    <div className="text-center text-white/30 py-10">No messages found</div>
                ) : (
                    <div className="space-y-4">
                        {results.map((result) => (
                            <button
                                key={result.messageId}
                                onClick={() => {
                                    if (!characterId) return;
                                    navigate(Routes.chatSession(characterId, sessionId, { jumpToMessage: result.messageId }));
                                }}
                                className={cn(
                                    "w-full text-left p-4 space-y-2 border border-white/10 bg-white/5 hover:bg-white/10 transition",
                                    radius.lg
                                )}
                            >
                                <div className="flex justify-between items-center text-xs text-white/40 uppercase font-medium tracking-wider">
                                    <span>{result.role === 'user' ? 'You' : 'Character'}</span>
                                    <span>{new Date(result.createdAt).toLocaleDateString()}</span>
                                </div>
                                <p className="text-sm text-white/80 line-clamp-3 leading-relaxed">
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
