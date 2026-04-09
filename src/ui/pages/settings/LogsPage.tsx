import { invoke } from "@tauri-apps/api/core";
import { getName } from "@tauri-apps/api/app";
import React, { Component, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import {
  RefreshCw,
  Trash2,
  Download,
  FolderOpen,
  Loader2,
  FileCode,
  Clipboard,
  Copy,
  Sparkles,
  ArrowDownToLine,
  ArrowUpToLine,
  Search,
  ChevronUp,
  ChevronDown,
  X,
  MoreVertical,
} from "lucide-react";
import { logManager } from "../../../core/utils/logger";
import { getPlatform } from "../../../core/utils/platform";
import { getEmbeddingModelInfo, readSettings, runEmbeddingTest } from "../../../core/storage/repo";
import {
  createDefaultAdvancedModelSettings,
  type AdvancedModelSettings,
} from "../../../core/storage/schemas";
import { sanitizeAdvancedModelSettings } from "../../components/AdvancedModelSettingsForm";
import { interactive, typography, cn } from "../../design-tokens";
import { confirmBottomMenu } from "../../components/ConfirmBottomMenu";
import { BottomMenu, MenuButton } from "../../components/BottomMenu";
// import { useI18n } from "../../../core/i18n/context";

const logger = logManager({ component: "LogsPage" });

const LOG_LINE_HEIGHT_BASE = 20;
const FONT_SIZE_BASE = 11;
const FONT_SIZES = [8, 9, 10, 11, 12, 13, 14, 16, 18, 20];
const DEFAULT_FONT_IDX = FONT_SIZES.indexOf(FONT_SIZE_BASE);


const PAGE_SIZE = 2000;

type LogPageResult = { total: number; lines: string[] };
type LogSearchResultType = { matches: number[]; total: number };

function ColoredLine({
  text,
  searchQuery,
  isSearchMatch,
  isActiveMatch,
}: {
  text: string;
  searchQuery: string;
  isSearchMatch: boolean;
  isActiveMatch: boolean;
}) {
  // Parse: [timestamp] LEVEL component at=location | message
  const match = text.match(
    /^(\[[^\]]*\])\s+(DEBUG|INFO|WARN|ERROR)\s+(\S+)\s+(at=\S+)\s*\|\s*(.*)$/,
  );

  const levelColor = (level: string) => {
    switch (level) {
      case "ERROR": return "text-danger";
      case "WARN": return "text-warning";
      case "DEBUG": return "text-fg/35";
      default: return "text-info/70";
    }
  };

  const highlight = (str: string) => {
    if (!searchQuery) return str;
    const lower = str.toLowerCase();
    const q = searchQuery.toLowerCase();
    const idx = lower.indexOf(q);
    if (idx === -1) return str;
    const parts: React.ReactNode[] = [];
    let cursor = 0;
    let pos = idx;
    let key = 0;
    while (pos !== -1) {
      if (pos > cursor) parts.push(str.slice(cursor, pos));
      parts.push(
        <span key={key++} className="bg-warning/40 text-fg rounded-sm px-px">
          {str.slice(pos, pos + q.length)}
        </span>,
      );
      cursor = pos + q.length;
      pos = lower.indexOf(q, cursor);
    }
    if (cursor < str.length) parts.push(str.slice(cursor));
    return <>{parts}</>;
  };

  const content = match ? (
    <>
      <span className="text-fg/25" style={{ fontSize: "0.85em" }}>{highlight(match[1])}</span>{" "}
      <span className={cn("font-semibold", levelColor(match[2]))}>{match[2]}</span>{" "}
      <span className="text-accent/70 font-medium">{highlight(match[3])}</span>{" "}
      <span className="text-fg/20" style={{ fontSize: "0.85em" }}>{highlight(match[4])}</span>
      <span className="text-fg/15"> | </span>
      <span className="text-fg/80">{highlight(match[5])}</span>
    </>
  ) : (
    <span className="text-fg/60">{highlight(text)}</span>
  );

  return (
    <span
      className={cn(
        "break-all",
        isActiveMatch && "rounded-sm ring-1 ring-warning/60",
        isSearchMatch && !isActiveMatch && "bg-warning/10",
      )}
    >
      {content}
    </span>
  );
}

function VirtualizedLogViewer({
  filename,
  loading,
}: {
  filename: string | null;
  loading: boolean;
}) {
  const parentRef = useRef<HTMLDivElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const [lines, setLines] = useState<string[]>([]);
  const [total, setTotal] = useState(0);
  const [loadingMore, setLoadingMore] = useState(false);
  const [fontIdx, setFontIdx] = useState(DEFAULT_FONT_IDX);
  const fontSize = FONT_SIZES[fontIdx];
  const lineHeight = Math.round(LOG_LINE_HEIGHT_BASE * (fontSize / FONT_SIZE_BASE));

  useEffect(() => {
    const handleWheel = (e: WheelEvent) => {
      if (!e.ctrlKey && !e.metaKey) return;
      if (!containerRef.current?.contains(e.target as Node)) return;
      e.preventDefault();
      setFontIdx((prev) =>
        e.deltaY < 0 ? Math.min(prev + 1, FONT_SIZES.length - 1) : Math.max(prev - 1, 0),
      );
    };
    const handleKey = (e: KeyboardEvent) => {
      if (!e.ctrlKey && !e.metaKey) return;
      if (e.key === "=" || e.key === "+") {
        e.preventDefault();
        setFontIdx((prev) => Math.min(prev + 1, FONT_SIZES.length - 1));
      } else if (e.key === "-") {
        e.preventDefault();
        setFontIdx((prev) => Math.max(prev - 1, 0));
      } else if (e.key === "0") {
        e.preventDefault();
        setFontIdx(DEFAULT_FONT_IDX);
      }
    };
    window.addEventListener("wheel", handleWheel, { passive: false });
    window.addEventListener("keydown", handleKey);
    return () => {
      window.removeEventListener("wheel", handleWheel);
      window.removeEventListener("keydown", handleKey);
    };
  }, []);

  type LogLevel = "ERROR" | "WARN" | "INFO" | "DEBUG";
  const [hiddenLevels, setHiddenLevels] = useState<Set<LogLevel>>(new Set());
  const [hiddenComponents, setHiddenComponents] = useState<Set<string>>(new Set());
  const [showComponentFilter, setShowComponentFilter] = useState(false);
  const [timeFrom, setTimeFrom] = useState("");
  const [timeTo, setTimeTo] = useState("");

  const extractComponent = (line: string): string | null => {
    const parts = line.split(/\s+/, 4);
    return parts.length >= 3 ? parts[2] : null;
  };

  const components = useMemo(() => {
    const set = new Set<string>();
    for (const line of lines) {
      const c = extractComponent(line);
      if (c) set.add(c);
    }
    return Array.from(set).sort();
  }, [lines]);

  const toggleComponent = useCallback((comp: string) => {
    setHiddenComponents((prev) => {
      const next = new Set(prev);
      if (next.has(comp)) next.delete(comp);
      else next.add(comp);
      return next;
    });
  }, []);

  const [isolatedLines, setIsolatedLines] = useState<Set<number> | null>(null);
  const [contextMenu, setContextMenu] = useState<{
    x: number;
    y: number;
    lineIndices: Set<number>;
  } | null>(null);
  const [loadingRelevant, setLoadingRelevant] = useState(false);

  const toggleLevel = useCallback((level: LogLevel) => {
    setHiddenLevels((prev) => {
      const next = new Set(prev);
      if (next.has(level)) next.delete(level);
      else next.add(level);
      return next;
    });
  }, []);

  const extractTime = (line: string): string | null => {
    const m = line.match(/^\[\d{4}-\d{2}-\d{2}T(\d{2}:\d{2}:\d{2})/);
    return m ? m[1] : null;
  };

  const filteredIndices = useMemo(() => {
    const hasLevelFilter = hiddenLevels.size > 0;
    const hasComponentFilter = hiddenComponents.size > 0;
    const hasTimeFilter = timeFrom !== "" || timeTo !== "";
    const hasIsolation = isolatedLines !== null;
    if (!hasLevelFilter && !hasComponentFilter && !hasTimeFilter && !hasIsolation) return null;
    const result: number[] = [];
    for (let i = 0; i < lines.length; i++) {
      if (hasIsolation && !isolatedLines.has(i)) continue;
      const line = lines[i];
      if (hasLevelFilter) {
        let hidden = false;
        for (const level of hiddenLevels) {
          if (line.includes(` ${level} `)) { hidden = true; break; }
        }
        if (hidden) continue;
      }
      
      if (hasComponentFilter) {
        const comp = extractComponent(line);
        if (comp && hiddenComponents.has(comp)) continue;
      }
      
      if (hasTimeFilter) {
        const t = extractTime(line);
        if (t) {
          if (timeFrom && t < timeFrom) continue;
          if (timeTo && t > timeTo) continue;
        }
      }
      result.push(i);
    }
    return result;
  }, [lines, hiddenLevels, hiddenComponents, timeFrom, timeTo, isolatedLines]);

  const displayCount = filteredIndices ? filteredIndices.length : lines.length;

  const [searchOpen, setSearchOpen] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const [searchMatches, setSearchMatches] = useState<number[]>([]);
  const [activeMatchIdx, setActiveMatchIdx] = useState(-1);
  const [searching, setSearching] = useState(false);
  const searchInputRef = useRef<HTMLInputElement>(null);
  const searchMatchSet = useMemo(() => new Set(searchMatches), [searchMatches]);

  useEffect(() => {
    if (!filename) {
      setLines([]);
      setTotal(0);
      return;
    }
    let cancelled = false;
    (async () => {
      try {
        const page = await invoke<LogPageResult>("read_log_file_page", {
          filename,
          offset: 0,
          limit: PAGE_SIZE,
        });
        if (!cancelled) {
          setLines(page.lines);
          setTotal(page.total);
        }
      } catch {
        if (!cancelled) {
          setLines(["Failed to load log file"]);
          setTotal(1);
        }
      }
    })();
    return () => { cancelled = true; };
  }, [filename]);

  useEffect(() => {
    setSearchQuery("");
    setSearchMatches([]);
    setActiveMatchIdx(-1);
    setTimeFrom("");
    setTimeTo("");
    setIsolatedLines(null);
    setHiddenComponents(new Set());
    setShowComponentFilter(false);
  }, [filename]);

  const loadMore = useCallback(async () => {
    if (!filename || loadingMore || lines.length >= total) return;
    setLoadingMore(true);
    try {
      const page = await invoke<LogPageResult>("read_log_file_page", {
        filename,
        offset: lines.length,
        limit: PAGE_SIZE,
      });
      setLines((prev) => [...prev, ...page.lines]);
      setTotal(page.total);
    } catch { } finally {
      setLoadingMore(false);
    }
  }, [filename, lines.length, total, loadingMore]);

  const runSearch = useCallback(async (query: string) => {
    if (!filename || !query.trim()) {
      setSearchMatches([]);
      setActiveMatchIdx(-1);
      return;
    }
    setSearching(true);
    try {
      const result = await invoke<LogSearchResultType>("search_log_file", {
        filename,
        query: query.trim(),
      });
      setSearchMatches(result.matches);
      setActiveMatchIdx(result.matches.length > 0 ? 0 : -1);
      if (result.matches.length > 0) {
        virtualizer.scrollToIndex(result.matches[0], { align: "center" });
      }
    } catch {
      setSearchMatches([]);
      setActiveMatchIdx(-1);
    } finally {
      setSearching(false);
    }
  }, [filename]);

  const searchTimerRef = useRef<ReturnType<typeof setTimeout>>(undefined);
  const handleSearchChange = useCallback((value: string) => {
    setSearchQuery(value);
    clearTimeout(searchTimerRef.current);
    searchTimerRef.current = setTimeout(() => runSearch(value), 300);
  }, [runSearch]);

  const goToMatch = useCallback((direction: "next" | "prev") => {
    if (searchMatches.length === 0) return;
    const nextIdx = direction === "next"
      ? (activeMatchIdx + 1) % searchMatches.length
      : (activeMatchIdx - 1 + searchMatches.length) % searchMatches.length;
    setActiveMatchIdx(nextIdx);
    virtualizer.scrollToIndex(searchMatches[nextIdx], { align: "center" });
  }, [searchMatches, activeMatchIdx]);

  const closeSearch = useCallback(() => {
    setSearchOpen(false);
    setSearchQuery("");
    setSearchMatches([]);
    setActiveMatchIdx(-1);
  }, []);

  const handleContextMenu = useCallback((e: React.MouseEvent, originalIdx: number) => {
    e.preventDefault();
    e.stopPropagation();
    window.getSelection()?.removeAllRanges();
    if (e.shiftKey && e.ctrlKey) {
      setContextMenu((prev) => {
        const existing = prev?.lineIndices ?? new Set<number>();
        const next = new Set(existing);
        if (existing.size > 0) {
          const sorted = Array.from(existing).sort((a, b) => a - b);
          let anchor = sorted[0];
          let minDist = Math.abs(originalIdx - anchor);
          for (const idx of sorted) {
            const dist = Math.abs(originalIdx - idx);
            if (dist < minDist) {
              minDist = dist;
              anchor = idx;
            }
          }
          const lo = Math.min(anchor, originalIdx);
          const hi = Math.max(anchor, originalIdx);
          for (let i = lo; i <= hi; i++) {
            next.add(i);
          }
        } else {
          next.add(originalIdx);
        }
        return { x: e.clientX, y: e.clientY, lineIndices: next };
      });
    } else if (e.shiftKey) {
      setContextMenu((prev) => {
        const existing = prev?.lineIndices ?? new Set<number>();
        const next = new Set(existing);
        if (next.has(originalIdx)) {
          next.delete(originalIdx);
          if (next.size === 0) return null;
        } else {
          next.add(originalIdx);
        }
        return { x: e.clientX, y: e.clientY, lineIndices: next };
      });
    } else {
      setContextMenu({ x: e.clientX, y: e.clientY, lineIndices: new Set([originalIdx]) });
    }
  }, []);

  const closeContextMenu = useCallback(() => setContextMenu(null), []);

  // Clamp context menu to viewport after render
  const contextMenuRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!contextMenu || !contextMenuRef.current) return;
    const el = contextMenuRef.current;
    const rect = el.getBoundingClientRect();
    const pad = 8;
    let x = contextMenu.x;
    let y = contextMenu.y;
    if (x + rect.width > window.innerWidth - pad) x = window.innerWidth - rect.width - pad;
    if (y + rect.height > window.innerHeight - pad) y = window.innerHeight - rect.height - pad;
    if (x < pad) x = pad;
    if (y < pad) y = pad;
    el.style.left = `${x}px`;
    el.style.top = `${y}px`;
  }, [contextMenu]);

  // Close on left-click outside
  useEffect(() => {
    if (!contextMenu) return;
    const handleDown = (e: MouseEvent) => {
      if (e.button !== 0) return;
      const menu = document.getElementById("log-context-menu");
      if (menu && menu.contains(e.target as Node)) return;
      setContextMenu(null);
    };
    window.addEventListener("mousedown", handleDown);
    return () => window.removeEventListener("mousedown", handleDown);
  }, [contextMenu]);

  const handleIsolate = useCallback((indices: Set<number>) => {
    setIsolatedLines((prev) => {
      const next = new Set(prev ?? []);
      for (const idx of indices) next.add(idx);
      return next;
    });
    setContextMenu(null);
  }, []);

  const handleLoadRelevant = useCallback(async (indices: Set<number>) => {
    if (!filename) return;
    setContextMenu(null);
    setLoadingRelevant(true);
    try {
      const allMatches = new Set<number>(indices);
      for (const lineIdx of indices) {
        const line = lines[lineIdx];
        if (!line) continue;
        const result = await invoke<LogSearchResultType>("find_relevant_lines", {
          filename,
          referenceLine: line,
        });
        for (const idx of result.matches) {
          if (idx < lines.length) allMatches.add(idx);
        }
      }
      if (allMatches.size > 0) {
        setIsolatedLines((prev) => {
          const next = new Set(prev ?? []);
          for (const idx of allMatches) next.add(idx);
          return next;
        });
      }
    } catch { } finally {
      setLoadingRelevant(false);
    }
  }, [filename, lines]);

  const handleCopyLines = useCallback(async (indices: Set<number>) => {
    const sorted = Array.from(indices).sort((a, b) => a - b);
    const text = sorted
      .map((idx) => lines[idx])
      .filter((line): line is string => line !== undefined)
      .join("\n");
    if (!text) {
      setContextMenu(null);
      return;
    }
    try {
      await navigator.clipboard.writeText(text);
    } catch (err) {
      logger.error("Failed to copy log lines", err);
    }
    setContextMenu(null);
  }, [lines]);

  const clearIsolation = useCallback(() => setIsolatedLines(null), []);

  const virtualizer = useVirtualizer({
    count: displayCount,
    getScrollElement: () => parentRef.current,
    estimateSize: () => lineHeight,
    overscan: 20,
    measureElement: (el) => el.getBoundingClientRect().height,
  });

  useEffect(() => {
    virtualizer.measure();
  }, [fontSize]);

  if (loading) {
    return (
      <div className="flex flex-1 items-center justify-center">
        <Loader2 className="h-6 w-6 animate-spin text-fg/30" />
      </div>
    );
  }

  if (!filename) {
    return (
      <div className="flex flex-1 items-center justify-center text-center px-6">
        <div>
          <FileCode className="mx-auto h-8 w-8 text-fg/15" />
          <p className={cn("mt-2", typography.caption.size, "text-fg/30")}>
            Select a log file to view its contents
          </p>
        </div>
      </div>
    );
  }

  const hasMore = lines.length < total;

  const scrollToBottom = async () => {
    if (hasMore && filename) {
      setLoadingMore(true);
      try {
        const page = await invoke<LogPageResult>("read_log_file_page", {
          filename,
          offset: lines.length,
          limit: total - lines.length,
        });
        setLines((prev) => {
          const all = [...prev, ...page.lines];
          requestAnimationFrame(() => {
            virtualizer.scrollToIndex(all.length - 1, { align: "end" });
          });
          return all;
        });
        setTotal(page.total);
      } catch { } finally {
        setLoadingMore(false);
      }
    } else if (lines.length > 0) {
      virtualizer.scrollToIndex(lines.length - 1, { align: "end" });
    }
  };

  return (
    <div ref={containerRef} className="flex flex-1 flex-col min-h-0">
      {/* Toolbar */}
      <div className="flex items-center gap-2 px-4 py-1.5 border-b border-fg/10 shrink-0 flex-wrap lg:flex-nowrap">
        <span className={cn(typography.caption.size, "text-fg/40 shrink-0")}>
          {filteredIndices ? `${displayCount.toLocaleString()} / ` : ""}
          {lines.length.toLocaleString()}{hasMore ? ` of ${total.toLocaleString()}` : ""} lines
        </span>
        {hasMore && (
          <button
            onClick={loadMore}
            disabled={loadingMore}
            className={cn(
              typography.caption.size,
              "shrink-0 text-info/70 hover:text-info",
              interactive.transition.default,
              "disabled:opacity-50",
            )}
          >
            {loadingMore ? "Loading..." : "Load more"}
          </button>
        )}
        {isolatedLines && (
          <>
            <span className={cn(typography.caption.size, "shrink-0 text-accent/70")}>
              {isolatedLines.size} isolated
            </span>
            <button
              onClick={() => {
                const sorted = Array.from(isolatedLines).sort((a, b) => a - b);
                const content = sorted.map((i) => lines[i] ?? "").join("\n");
                const blob = new Blob([content], { type: "text/plain" });
                const url = URL.createObjectURL(blob);
                const a = document.createElement("a");
                a.href = url;
                a.download = `isolated-${filename?.replace(/\.log$/, "") ?? "logs"}.log`;
                a.click();
                URL.revokeObjectURL(url);
              }}
              className={cn("shrink-0 text-fg/40 hover:text-fg/70 hover:bg-fg/10 rounded p-1", interactive.transition.default)}
              title="Download isolated lines"
            >
              <Download className="h-3 w-3" />
            </button>
            <button
              onClick={clearIsolation}
              className={cn("shrink-0 text-fg/40 hover:text-fg/70 hover:bg-fg/10 rounded p-1", interactive.transition.default)}
              title="Clear isolation"
            >
              <X className="h-3 w-3" />
            </button>
          </>
        )}
        {loadingRelevant && <Loader2 className="h-3 w-3 animate-spin text-fg/30 shrink-0" />}
        <div className="flex-1" />
        {fontSize !== FONT_SIZE_BASE && (
          <button
            onClick={() => setFontIdx(DEFAULT_FONT_IDX)}
            className={cn("shrink-0 text-[10px] text-fg/30 hover:text-fg/50 tabular-nums", interactive.transition.default)}
            title="Reset zoom (Ctrl+0)"
          >
            {fontSize}px
          </button>
        )}
        <button
          onClick={() => { setSearchOpen(true); requestAnimationFrame(() => searchInputRef.current?.focus()); }}
          className={cn("shrink-0 text-fg/40 hover:text-fg/70 hover:bg-fg/10 rounded p-1", interactive.transition.default, searchOpen && "text-info")}
          title="Search"
        >
          <Search className="h-3.5 w-3.5" />
        </button>
        <button
          onClick={() => virtualizer.scrollToIndex(0, { align: "start" })}
          className={cn("shrink-0 text-fg/40 hover:text-fg/70 hover:bg-fg/10 rounded p-1", interactive.transition.default)}
          title="Jump to top"
        >
          <ArrowUpToLine className="h-3.5 w-3.5" />
        </button>
        <button
          onClick={scrollToBottom}
          disabled={loadingMore}
          className={cn("shrink-0 text-fg/40 hover:text-fg/70 hover:bg-fg/10 rounded p-1", interactive.transition.default, "disabled:opacity-50")}
          title="Jump to bottom"
        >
          <ArrowDownToLine className="h-3.5 w-3.5" />
        </button>
        <div className="w-px h-4 bg-fg/10 mx-0.5 shrink-0 hidden lg:block" />
        {/* Level filters */}
        {(["ERROR", "WARN", "INFO", "DEBUG"] as const).map((level) => {
          const active = !hiddenLevels.has(level);
          const color = {
            ERROR: active ? "text-danger border-danger/30 bg-danger/10 hover:bg-danger/20" : "text-fg/25 border-fg/10 hover:text-fg/40 hover:border-fg/20",
            WARN: active ? "text-warning border-warning/30 bg-warning/10 hover:bg-warning/20" : "text-fg/25 border-fg/10 hover:text-fg/40 hover:border-fg/20",
            INFO: active ? "text-info border-info/30 bg-info/10 hover:bg-info/20" : "text-fg/25 border-fg/10 hover:text-fg/40 hover:border-fg/20",
            DEBUG: active ? "text-fg/60 border-fg/20 bg-fg/5 hover:bg-fg/10" : "text-fg/25 border-fg/10 hover:text-fg/40 hover:border-fg/20",
          }[level];
          return (
            <button
              key={level}
              onClick={() => toggleLevel(level)}
              className={cn("shrink-0 rounded px-1.5 py-0.5 border text-[10px] font-semibold", interactive.transition.default, color)}
            >
              {level}
            </button>
          );
        })}
        {/* Component filter */}
        <div className="relative shrink-0">
          <button
            onClick={() => setShowComponentFilter((p) => !p)}
            className={cn(
              "rounded px-1.5 py-0.5 border text-[10px] font-semibold",
              interactive.transition.default,
              hiddenComponents.size > 0
                ? "text-accent border-accent/30 bg-accent/10 hover:bg-accent/20"
                : "text-fg/40 border-fg/10 hover:text-fg/60 hover:border-fg/20",
            )}
            title="Filter by component"
          >
            {hiddenComponents.size > 0
              ? `${components.length - hiddenComponents.size}/${components.length}`
              : `${components.length}`}
            {" "}src
          </button>
          {showComponentFilter && (
            <>
              <div className="fixed inset-0 z-[60]" onClick={() => setShowComponentFilter(false)} />
              <div className="fixed left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 z-[70] rounded-lg border border-fg/15 bg-surface shadow-xl py-1 w-72 max-h-80 overflow-y-auto scrollbar-thin">
                <div className="flex items-center justify-between px-3 py-1.5 border-b border-fg/10 sticky top-0 bg-surface z-10">
                  <span className={cn(typography.caption.size, "text-fg/40 font-medium")}>Components</span>
                  <div className="flex gap-2">
                    <button
                      onClick={() => setHiddenComponents(new Set())}
                      className={cn(typography.caption.size, "text-info/70 hover:text-info", interactive.transition.default)}
                    >
                      All
                    </button>
                    <button
                      onClick={() => setHiddenComponents(new Set(components))}
                      className={cn(typography.caption.size, "text-fg/40 hover:text-fg/70 hover:bg-fg/10 rounded p-1", interactive.transition.default)}
                    >
                      None
                    </button>
                  </div>
                </div>
                {components.map((comp) => {
                  const visible = !hiddenComponents.has(comp);
                  return (
                    <button
                      key={comp}
                      onClick={() => toggleComponent(comp)}
                      className={cn(
                        "w-full px-3 py-1 text-left flex items-center gap-2",
                        typography.caption.size,
                        "font-mono",
                        interactive.transition.default,
                        visible ? "text-fg/70 hover:bg-fg/5" : "text-fg/25 hover:bg-fg/5",
                      )}
                    >
                      <span className={cn(
                        "w-2 h-2 rounded-sm border shrink-0",
                        visible ? "bg-accent/60 border-accent/40" : "border-fg/20",
                      )} />
                      <span className="truncate">{comp}</span>
                    </button>
                  );
                })}
              </div>
            </>
          )}
        </div>
        <div className="w-px h-4 bg-fg/10 mx-0.5 shrink-0" />
        {/* Time range filter */}
        <div className="flex items-center gap-1 shrink-0">
          <input
            type="time"
            step="1"
            value={timeFrom}
            onChange={(e) => setTimeFrom(e.target.value)}
            className={cn(
              "bg-transparent border border-fg/10 rounded px-1.5 py-0.5 text-[10px] font-mono text-fg/60 w-22",
              "focus:outline-none focus:border-fg/25",
              !timeFrom && "text-fg/25",
            )}
            title="From time"
          />
          <span className="text-fg/20 text-[10px]">-</span>
          <input
            type="time"
            step="1"
            value={timeTo}
            onChange={(e) => setTimeTo(e.target.value)}
            className={cn(
              "bg-transparent border border-fg/10 rounded px-1.5 py-0.5 text-[10px] font-mono text-fg/60 w-22",
              "focus:outline-none focus:border-fg/25",
              !timeTo && "text-fg/25",
            )}
            title="To time"
          />
          {(timeFrom || timeTo) && (
            <button
              onClick={() => { setTimeFrom(""); setTimeTo(""); }}
              className="text-fg/30 hover:text-fg/60"
              title="Clear time filter"
            >
              <X className="h-3 w-3" />
            </button>
          )}
        </div>
      </div>

      {/* Search bar */}
      {searchOpen && (
        <div className="flex items-center gap-2 px-4 py-1.5 border-b border-fg/10 bg-fg/3 shrink-0">
          <Search className="h-3.5 w-3.5 text-fg/30 shrink-0" />
          <input
            ref={searchInputRef}
            type="text"
            value={searchQuery}
            onChange={(e) => handleSearchChange(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") goToMatch(e.shiftKey ? "prev" : "next");
              if (e.key === "Escape") closeSearch();
            }}
            placeholder="Search logs..."
            className={cn(
              typography.caption.size,
              "flex-1 bg-transparent text-fg outline-none placeholder:text-fg/30",
            )}
          />
          {searching && <Loader2 className="h-3 w-3 animate-spin text-fg/30 shrink-0" />}
          {searchMatches.length > 0 && (
            <span className={cn(typography.caption.size, "text-fg/40 shrink-0")}>
              {activeMatchIdx + 1}/{searchMatches.length}
            </span>
          )}
          {searchQuery && searchMatches.length === 0 && !searching && (
            <span className={cn(typography.caption.size, "text-danger/60 shrink-0")}>
              No matches
            </span>
          )}
          <button
            onClick={() => goToMatch("prev")}
            disabled={searchMatches.length === 0}
            className="text-fg/40 hover:text-fg/70 hover:bg-fg/10 rounded p-1 disabled:opacity-30"
          >
            <ChevronUp className="h-3.5 w-3.5" />
          </button>
          <button
            onClick={() => goToMatch("next")}
            disabled={searchMatches.length === 0}
            className="text-fg/40 hover:text-fg/70 hover:bg-fg/10 rounded p-1 disabled:opacity-30"
          >
            <ChevronDown className="h-3.5 w-3.5" />
          </button>
          <button
            onClick={closeSearch}
            className="text-fg/40 hover:text-fg/70 hover:bg-fg/10 rounded p-1"
          >
            <X className="h-3.5 w-3.5" />
          </button>
        </div>
      )}

      {/* Empty state when filters hide everything */}
      {displayCount === 0 && lines.length > 0 && (
        <div className="flex flex-1 items-center justify-center text-center px-6">
          <div>
            <Search className="mx-auto h-8 w-8 text-fg/10" />
            <p className={cn("mt-2", typography.caption.size, "text-fg/30")}>
              No lines match the current filters
            </p>
            <button
              onClick={() => { setHiddenLevels(new Set()); setHiddenComponents(new Set()); setTimeFrom(""); setTimeTo(""); setIsolatedLines(null); }}
              className={cn(
                "mt-2",
                typography.caption.size,
                "text-info/70 hover:text-info",
                interactive.transition.default,
              )}
            >
              Clear all filters
            </button>
          </div>
        </div>
      )}

      {/* Log lines */}
      <div key={`zoom-${fontIdx}`} ref={parentRef} className={cn("flex-1 overflow-y-auto min-h-0 scrollbar-thin pb-[calc(env(safe-area-inset-bottom)+4px)]", displayCount === 0 && "hidden")}>
        <div className="relative w-full" style={{ height: virtualizer.getTotalSize() }}>
          {virtualizer.getVirtualItems().map((virtualRow) => {
            const originalIdx = filteredIndices ? filteredIndices[virtualRow.index] : virtualRow.index;
            const line = lines[originalIdx] ?? "";
            const isMatch = searchMatchSet.has(originalIdx);
            const isActive = activeMatchIdx >= 0 && searchMatches[activeMatchIdx] === originalIdx;
            const isContextTarget = contextMenu !== null && contextMenu.lineIndices.has(originalIdx);
            const isError = line.includes(" ERROR ");
            const isWarn = line.includes(" WARN ");
            const isDebug = line.includes(" DEBUG ");
            const borderColor = isError
              ? "border-l-danger/50"
              : isWarn
                ? "border-l-warning/40"
                : isDebug
                  ? "border-l-fg/10"
                  : "border-l-info/20";
            return (
              <div
                key={virtualRow.index}
                data-index={virtualRow.index}
                ref={virtualizer.measureElement}
                onContextMenu={(e) => handleContextMenu(e, originalIdx)}
                className={cn(
                  "absolute left-0 right-0 pl-4 pr-4 py-1 font-mono border-l-2 select-none",
                  "cursor-default",
                  borderColor,
                  isError && "bg-danger/[0.03]",
                  isContextTarget && "bg-fg/10",
                  isActive && "bg-warning/10",
                  !isActive && isMatch && "bg-warning/5",
                  !isActive && !isMatch && !isContextTarget && !isError && virtualRow.index % 2 === 0 && "bg-fg/[0.02]",
                )}
                style={{
                  fontSize: `${fontSize}px`,
                  lineHeight: `${fontSize + 8}px`,
                  transform: `translateY(${virtualRow.start}px)`,
                }}
              >
                <ColoredLine
                  text={line}
                  searchQuery={searchQuery}
                  isSearchMatch={isMatch}
                  isActiveMatch={isActive}
                />
              </div>
            );
          })}
        </div>
      </div>

      {/* Context menu */}
      {contextMenu && (
          <div
            id="log-context-menu"
            ref={contextMenuRef}
            className="fixed z-50 rounded-lg border border-fg/15 bg-surface shadow-xl py-1 min-w-44"
            style={{ left: contextMenu.x, top: contextMenu.y }}
          >
            <button
              onClick={() => void handleCopyLines(contextMenu.lineIndices)}
              className={cn(
                "w-full px-3 py-1.5 text-left flex items-center gap-2",
                typography.caption.size,
                "text-fg/70 hover:bg-fg/10",
                interactive.transition.default,
              )}
            >
              <Copy className="w-3.5 h-3.5 text-fg/50" />
              Copy {contextMenu.lineIndices.size > 1 ? `${contextMenu.lineIndices.size} lines` : "this line"}
            </button>
            <div className="border-t border-fg/10 my-1" />
            {!isolatedLines ? (
              <button
                onClick={() => handleIsolate(contextMenu.lineIndices)}
                className={cn(
                  "w-full px-3 py-1.5 text-left flex items-center gap-2",
                  typography.caption.size,
                  "text-fg/70 hover:bg-fg/10",
                  interactive.transition.default,
                )}
              >
                <span className="w-1.5 h-1.5 rounded-full bg-accent/60" />
                Isolate {contextMenu.lineIndices.size > 1 ? `${contextMenu.lineIndices.size} lines` : "this line"}
              </button>
            ) : (
              <>
                <button
                  onClick={() => void handleLoadRelevant(contextMenu.lineIndices)}
                  className={cn(
                    "w-full px-3 py-1.5 text-left flex items-center gap-2",
                    typography.caption.size,
                    "text-fg/70 hover:bg-fg/10",
                    interactive.transition.default,
                  )}
                >
                  <span className="w-1.5 h-1.5 rounded-full bg-info/60" />
                  Load relevant{contextMenu.lineIndices.size > 1 ? ` (${contextMenu.lineIndices.size} lines)` : ""}
                </button>
                <button
                  onClick={() => {
                    setIsolatedLines((prev) => {
                      if (!prev) return prev;
                      const next = new Set(prev);
                      for (const idx of contextMenu.lineIndices) next.delete(idx);
                      return next.size === 0 ? null : next;
                    });
                    closeContextMenu();
                  }}
                  className={cn(
                    "w-full px-3 py-1.5 text-left flex items-center gap-2",
                    typography.caption.size,
                    "text-fg/70 hover:bg-fg/10",
                    interactive.transition.default,
                  )}
                >
                  <span className="w-1.5 h-1.5 rounded-full bg-warning/60" />
                  Unisolate {contextMenu.lineIndices.size > 1 ? `${contextMenu.lineIndices.size} lines` : "this line"}
                </button>
                <div className="border-t border-fg/10 my-1" />
                <button
                  onClick={() => { clearIsolation(); closeContextMenu(); }}
                  className={cn(
                    "w-full px-3 py-1.5 text-left flex items-center gap-2",
                    typography.caption.size,
                    "text-fg/50 hover:bg-fg/10",
                    interactive.transition.default,
                  )}
                >
                  <span className="w-1.5 h-1.5 rounded-full bg-fg/30" />
                  Clear all isolation
                </button>
              </>
            )}
          </div>
      )}
    </div>
  );
}

function LogsPageInner() {
  const platform = getPlatform();
  const isAndroid = platform.os === "android";
  const [logFiles, setLogFiles] = useState<string[]>([]);
  const [selectedFile, setSelectedFile] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const [logDir, setLogDir] = useState<string>("");
  const [exportDir, setExportDir] = useState<string>("");
  const [diagnosticsText, setDiagnosticsText] = useState("");
  const [generatingDiagnostics, setGeneratingDiagnostics] = useState(false);

  const formatBytes = (bytes: number | null | undefined) => {
    if (bytes == null) return "Unknown";
    if (bytes === 0) return "0 B";
    const units = ["B", "KB", "MB", "GB", "TB"];
    const index = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
    const value = bytes / 1024 ** index;
    return `${value.toFixed(value >= 10 || index === 0 ? 0 : 1)} ${units[index]}`;
  };

  const buildDiagnostics = async () => {
    setGeneratingDiagnostics(true);
    try {
      const [settings, embeddingInfo] = await Promise.allSettled([
        readSettings(),
        getEmbeddingModelInfo(),
      ]).then(([s, e]) => [
        s.status === "fulfilled" ? s.value : null,
        e.status === "fulfilled" ? e.value : { installed: false, version: null, sourceVersion: null, maxTokens: null },
      ] as const);
      const platform = getPlatform();
      let appName = "unknown";
      let appVersion = "unknown";
      try { appName = await getName(); } catch { /* ignore */ }
      try { appVersion = await invoke<string>("get_app_version"); } catch { /* ignore */ }

      let storageRoot: string | null = null;
      let dbSize: number | null = null;
      try {
        storageRoot = await invoke<string>("get_storage_root");
      } catch (err) {
        logger.warn("Failed to read storage root", err);
      }
      try {
        dbSize = await invoke<number>("storage_db_size");
      } catch (err) {
        logger.warn("Failed to read database size", err);
      }

      let embeddingTest;
      try {
        embeddingTest = await runEmbeddingTest();
      } catch (err: any) {
        embeddingTest = {
          success: false,
          message: err?.message || "Embedding test failed",
          scores: [],
          modelInfo: embeddingInfo.installed
            ? {
                version: embeddingInfo.version ?? "unknown",
                maxTokens: embeddingInfo.maxTokens ?? 0,
                embeddingDimensions: 0,
              }
            : {
                version: "not installed",
                maxTokens: 0,
                embeddingDimensions: 0,
              },
        };
      }

      const dynamicSettings = settings?.advancedSettings?.dynamicMemory;
      const dynamicEnabled = dynamicSettings?.enabled ?? false;

      const lines: string[] = [];
      lines.push("LettuceAI Diagnostics");
      lines.push(`Generated: ${new Date().toISOString()}`);
      lines.push("");
      lines.push("App");
      lines.push(`- Name: ${appName}`);
      lines.push(`- Version: ${appVersion}`);
      lines.push(`- Build: ${import.meta.env.DEV ? "debug" : "release"}`);
      lines.push("");
      lines.push("Device");
      lines.push(`- Platform: ${platform.type} (${platform.os})`);
      lines.push(`- Arch: ${platform.arch}`);
      lines.push("");
      lines.push("Storage");
      lines.push(`- App data path: ${storageRoot ?? "unknown"}`);
      lines.push(`- Database size: ${formatBytes(dbSize)}`);
      if (settings) {
        lines.push("");
        lines.push("App State");
        lines.push(
          `- Pure Mode: ${settings.appState.pureModeLevel ?? (settings.appState.pureModeEnabled ? "standard" : "off")}`,
        );
        lines.push(`- Analytics: ${settings.appState.analyticsEnabled ? "enabled" : "disabled"}`);
        lines.push("");
        lines.push("Providers");
        if (settings.providerCredentials.length > 0) {
          for (const provider of settings.providerCredentials) {
            lines.push(`- ${provider.providerId} (${provider.label})`);
          }
        } else {
          lines.push("- none");
        }
      } else {
        lines.push("");
        lines.push("Settings: UNAVAILABLE (database may be corrupted)");
      }
      lines.push("");
      lines.push("Models");
      if (settings && settings.models.length > 0) {
        for (const model of settings.models) {
          const defaults = createDefaultAdvancedModelSettings();
          const merged: AdvancedModelSettings = {
            ...defaults,
            ...(model.advancedModelSettings ?? {}),
          };
          const normalized = sanitizeAdvancedModelSettings(merged);
          lines.push(`- ${model.id}`);
          lines.push(`  name: ${model.name}`);
          lines.push(`  display: ${model.displayName}`);
          lines.push(`  provider: ${model.providerId} (${model.providerLabel})`);
          lines.push(`  inputScopes: ${(model.inputScopes ?? []).join(", ") || "none"}`);
          lines.push(`  outputScopes: ${(model.outputScopes ?? []).join(", ") || "none"}`);
          lines.push(`  advanced: ${JSON.stringify(normalized)}`);
        }
      } else {
        lines.push("- none");
      }
      lines.push("");
      lines.push("Memory System");
      lines.push(`- Dynamic memory enabled: ${dynamicEnabled ? "yes" : "no"}`);
      if (dynamicSettings) {
        lines.push(`- Summary interval: ${dynamicSettings.summaryMessageInterval}`);
        lines.push(`- Max entries: ${dynamicSettings.maxEntries}`);
        lines.push(`- Min similarity: ${dynamicSettings.minSimilarityThreshold}`);
        lines.push(`- Hot token budget: ${dynamicSettings.hotMemoryTokenBudget}`);
        lines.push(`- Decay rate: ${dynamicSettings.decayRate}`);
        lines.push(`- Cold threshold: ${dynamicSettings.coldThreshold}`);
        lines.push(
          `- Context enrichment: ${dynamicSettings.contextEnrichmentEnabled ? "enabled" : "disabled"}`,
        );
      }
      lines.push("");
      lines.push("Embedding Model");
      lines.push(`- Installed: ${embeddingInfo.installed ? "yes" : "no"}`);
      lines.push(`- Version: ${embeddingInfo.version ?? "unknown"}`);
      lines.push(`- Source version: ${embeddingInfo.sourceVersion ?? "unknown"}`);
      lines.push(`- Max tokens: ${embeddingInfo.maxTokens ?? "unknown"}`);
      lines.push("");
      lines.push("Embedding Test");
      lines.push(`- Success: ${embeddingTest.success ? "yes" : "no"}`);
      lines.push(`- Message: ${embeddingTest.message}`);
      if (embeddingTest.scores.length > 0) {
        lines.push("- Scores:");
        for (const score of embeddingTest.scores) {
          const scoreValue = Number.isFinite(score.similarityScore)
            ? score.similarityScore.toFixed(3)
            : String(score.similarityScore);
          lines.push(
            `  - ${score.pairName}: ${scoreValue} (expected ${score.expected}) ${score.passed ? "PASS" : "FAIL"}`,
          );
        }
      } else {
        lines.push("- Scores: none");
      }

      setDiagnosticsText(lines.join("\n"));
      return lines.join("\n");
    } catch (err) {
      const msg = `Diagnostics generation failed: ${err instanceof Error ? err.message : String(err)}`;
      setDiagnosticsText(msg);
      return msg;
    } finally {
      setGeneratingDiagnostics(false);
    }
  };

  const copyDiagnostics = async () => {
    try {
      const text = diagnosticsText || (await buildDiagnostics());
      await navigator.clipboard.writeText(text);
      alert("Diagnostics copied to clipboard.");
    } catch (err) {
      logger.error("Failed to copy diagnostics", err);
      alert("Failed to copy diagnostics.");
    }
  };

  const loadLogFiles = async () => {
    try {
      setRefreshing(true);
      const files = await invoke<string[]>("list_log_files");
      setLogFiles(files);
    } catch (err) {
      logger.error("Failed to load log files", err);
    } finally {
      setRefreshing(false);
    }
  };

  const loadLogDir = async () => {
    try {
      const dir = await invoke<string>("get_log_dir_path");
      setLogDir(dir);
    } catch (err) {
      logger.error("Failed to get log directory", err);
    }
  };

  const loadLogExportDir = async () => {
    if (!isAndroid) return;
    try {
      const dir = await invoke<string | null>("get_log_export_dir");
      setExportDir(dir ?? "");
    } catch (err) {
      logger.error("Failed to get Android log export directory", err);
    }
  };

  const selectFile = (filename: string) => {
    setSelectedFile(filename);
  };

  const downloadLogFile = async () => {
    if (!selectedFile) return;
    try {
      const savedPath = await invoke<string>("save_log_to_downloads", { filename: selectedFile });
      alert(`Log file saved to:\n${savedPath}`);
    } catch (err) {
      logger.error("Failed to download log file", err);
      alert(`Failed to save log file: ${err}`);
    }
  };

  const pickLogExportDir = async () => {
    try {
      const dir = await invoke<string | null>("pick_log_export_dir");
      if (dir) {
        setExportDir(dir);
        alert(`Log export folder set to:\n${dir}`);
      }
    } catch (err) {
      logger.error("Failed to pick Android log export directory", err);
      alert(`Failed to set log export folder: ${err}`);
    }
  };

  const clearLogExportDir = async () => {
    try {
      await invoke("clear_log_export_dir");
      setExportDir("");
    } catch (err) {
      logger.error("Failed to clear Android log export directory", err);
      alert(`Failed to clear log export folder: ${err}`);
    }
  };

  const deleteLogFile = async (filename: string) => {
    try {
      await invoke("delete_log_file", { filename });
      await loadLogFiles();
      if (selectedFile === filename) {
        setSelectedFile(null);
      }
    } catch (err) {
      logger.error("Failed to delete log file", err);
    }
  };

  const clearAllLogs = async () => {
    const confirmed = await confirmBottomMenu({
      title: "Delete all logs?",
      message: "Are you sure you want to delete all log files?",
      confirmLabel: "Delete",
      destructive: true,
    });
    if (!confirmed) return;
    try {
      await invoke("clear_all_logs");
      await loadLogFiles();
      setSelectedFile(null);
    } catch (err) {
      logger.error("Failed to clear all logs", err);
    }
  };

  useEffect(() => {
    loadLogFiles();
    loadLogDir();
    loadLogExportDir();
  }, []);

  useEffect(() => {
    if (logFiles.length > 0 && !selectedFile) {
      selectFile(logFiles[0]);
    }
  }, [logFiles]);


  const [showMenu, setShowMenu] = useState(false);
  const isDesktop = platform.type === "desktop";

  const fileList = (
    <div className="space-y-0.5">
      {logFiles.map((file) => (
        <button
          key={file}
          onClick={() => selectFile(file)}
          className={cn(
            "group w-full rounded-lg px-3 py-1.5 text-left",
            interactive.transition.default,
            selectedFile === file
              ? "bg-info/10 text-info"
              : "text-fg/60 hover:bg-fg/5",
          )}
        >
          <div className="flex items-center gap-2">
            <span className={cn(typography.caption.size, "flex-1 truncate font-medium")}>
              {file.replace(/^app-/, "").replace(/\.log$/, "")}
            </span>
          </div>
        </button>
      ))}
    </div>
  );

  const actionButtons = (
    <div className="flex items-center gap-1">
      <button
        onClick={downloadLogFile}
        disabled={!selectedFile}
        className={cn("shrink-0 text-fg/40 hover:text-fg/70 hover:bg-fg/10 rounded p-1", interactive.transition.default, "disabled:opacity-30")}
        title="Download log file"
      >
        <Download className="h-3.5 w-3.5" />
      </button>
      <button
        onClick={() => {
          void loadLogFiles();
          if (selectedFile) {
            const f = selectedFile;
            setSelectedFile(null);
            setTimeout(() => setSelectedFile(f), 0);
          }
        }}
        disabled={refreshing}
        className={cn("shrink-0 text-fg/40 hover:text-fg/70 hover:bg-fg/10 rounded p-1", interactive.transition.default, "disabled:opacity-50")}
        title="Refresh"
      >
        <RefreshCw className={cn("h-3.5 w-3.5", refreshing && "animate-spin")} />
      </button>
      <button
        onClick={() => setShowMenu(true)}
        className={cn("shrink-0 text-fg/40 hover:text-fg/70 hover:bg-fg/10 rounded p-1", interactive.transition.default)}
        title="More options"
      >
        <MoreVertical className="h-3.5 w-3.5" />
      </button>
    </div>
  );

  const bottomMenu = (
    <BottomMenu
      isOpen={showMenu}
      onClose={() => setShowMenu(false)}
      title="Log Options"
    >
      <div className="space-y-4">
        <div>
          <div className="flex items-center justify-between mb-2">
            <span className={cn(typography.caption.size, "font-medium text-fg/50")}>Diagnostics</span>
            <div className="flex items-center gap-2">
              <button
                onClick={buildDiagnostics}
                disabled={generatingDiagnostics}
                className={cn(typography.caption.size, "flex items-center gap-1 text-fg/50 hover:text-fg/80", interactive.transition.default, "disabled:opacity-50")}
              >
                <Sparkles className={cn("h-3.5 w-3.5", generatingDiagnostics && "animate-spin")} />
                Generate
              </button>
              <button
                onClick={copyDiagnostics}
                disabled={generatingDiagnostics}
                className={cn(typography.caption.size, "flex items-center gap-1 text-info/70 hover:text-info", interactive.transition.default, "disabled:opacity-50")}
              >
                <Clipboard className="h-3.5 w-3.5" />
                Copy
              </button>
            </div>
          </div>
          <div className="rounded-lg border border-fg/10 bg-fg/5 overflow-hidden">
            {generatingDiagnostics ? (
              <div className="flex items-center justify-center py-6">
                <Loader2 className="h-5 w-5 animate-spin text-fg/30" />
              </div>
            ) : (
              <div className="max-h-48 overflow-y-auto">
                <pre className={cn(typography.caption.size, "font-mono p-3 text-fg/60 whitespace-pre-wrap wrap-break-word leading-relaxed")}>
                  {diagnosticsText || "Click Generate to see device/app summary."}
                </pre>
              </div>
            )}
          </div>
        </div>
        {logDir && (
          <div className="flex items-center gap-2">
            <FolderOpen className="h-3.5 w-3.5 text-fg/30 shrink-0" />
            <p className={cn(typography.caption.size, "text-fg/40 font-mono truncate")}>{logDir}</p>
          </div>
        )}
        {isAndroid && (
          <div className="rounded-lg border border-fg/10 bg-fg/5 p-3">
            <div className="flex items-center justify-between gap-3">
              <div className="min-w-0">
                <p className={cn(typography.caption.size, "text-fg/50")}>Export folder</p>
                <p className={cn(typography.caption.size, "text-fg/70 break-all")}>{exportDir || "Downloads/lettuceai/logs"}</p>
              </div>
              <div className="flex items-center gap-2 shrink-0">
                <button onClick={pickLogExportDir} className={cn(typography.caption.size, "text-info/70 hover:text-info", interactive.transition.default)}>Choose</button>
                {exportDir && (
                  <button onClick={clearLogExportDir} className={cn(typography.caption.size, "text-fg/40 hover:text-fg/70", interactive.transition.default)}>Reset</button>
                )}
              </div>
            </div>
          </div>
        )}
        {logFiles.length > 0 && (
          <div>
            <span className={cn(typography.caption.size, "font-medium text-fg/50 mb-2 block")}>Delete log files</span>
            <div className="flex flex-wrap gap-1">
              {logFiles.map((file) => (
                <button
                  key={file}
                  onClick={() => deleteLogFile(file)}
                  className={cn("rounded-md px-2 py-1 border text-[10px]", interactive.transition.default, "border-fg/10 text-fg/50 hover:border-danger/30 hover:text-danger/70 hover:bg-danger/5")}
                >
                  {file.replace(/^app-/, "").replace(/\.log$/, "")}
                </button>
              ))}
            </div>
          </div>
        )}
        {logFiles.length > 0 && (
          <MenuButton
            icon={Trash2}
            title="Clear All Logs"
            description="Delete all log files permanently"
            onClick={() => { setShowMenu(false); void clearAllLogs(); }}
            color="from-danger to-danger/80"
          />
        )}
      </div>
    </BottomMenu>
  );

  if (isDesktop) {
    return (
      <div className="flex h-[calc(100dvh-72px-env(safe-area-inset-top))] overflow-hidden">
        {/* Left sidebar */}
        <div className="w-48 shrink-0 flex flex-col border-r border-fg/10">
          <div className="shrink-0 px-3 pt-3 pb-1 flex items-center justify-between">
            <span className={cn(typography.overline.size, typography.overline.weight, typography.overline.tracking, typography.overline.transform, "text-fg/35")}>
              Log Files ({logFiles.length})
            </span>
            {actionButtons}
          </div>
          <div className="flex-1 overflow-y-auto min-h-0 px-2">
            {logFiles.length === 0 ? (
              <div className="py-8 text-center">
                <FileCode className="mx-auto h-8 w-8 text-fg/15" />
                <p className={cn("mt-2", typography.caption.size, "text-fg/30")}>No logs yet</p>
              </div>
            ) : (
              fileList
            )}
          </div>
        </div>

        {/* Right: viewer */}
        <div className="flex-1 flex flex-col min-h-0 min-w-0">
          <VirtualizedLogViewer filename={selectedFile} loading={false} />
        </div>

        {/* Bottom menu */}
        {bottomMenu}
      </div>
    );
  }

  const fileTabsRef = useRef<HTMLDivElement>(null);

  return (
    <div className="flex h-[calc(100dvh-72px-env(safe-area-inset-top))] flex-col overflow-hidden">
      {/* Header bar: file tabs + action buttons */}
      <div className="flex items-center gap-1 px-3 py-1.5 border-b border-fg/10 shrink-0 min-w-0">
        <div ref={fileTabsRef} className="flex-1 flex gap-1 overflow-x-auto scrollbar-hide min-w-0">
          {logFiles.length === 0 ? (
            <span className={cn(typography.caption.size, "text-fg/30 py-1")}>No log files</span>
          ) : (
            logFiles.map((file) => (
              <button
                key={file}
                onClick={() => selectFile(file)}
                className={cn(
                  "shrink-0 rounded-md px-2.5 py-1",
                  typography.caption.size,
                  interactive.transition.default,
                  selectedFile === file
                    ? "bg-info/15 text-info font-medium"
                    : "text-fg/50",
                )}
              >
                {file.replace(/^app-/, "").replace(/\.log$/, "")}
              </button>
            ))
          )}
        </div>
        {actionButtons}
      </div>

      <VirtualizedLogViewer filename={selectedFile} loading={false} />
      {bottomMenu}
    </div>
  );
}

class LogsErrorBoundary extends Component<
  { children: React.ReactNode },
  { error: Error | null }
> {
  state: { error: Error | null } = { error: null };

  static getDerivedStateFromError(error: Error) {
    return { error };
  }

  render() {
    if (this.state.error) {
      return (
        <div className="flex h-[calc(100dvh-72px-env(safe-area-inset-bottom))] flex-col items-center justify-center gap-4 px-6 text-center">
          <div className="rounded-xl border border-danger/20 bg-danger/5 p-6 max-w-md">
            <h2 className="text-sm font-semibold text-danger mb-2">Logs page crashed</h2>
            <pre className="text-[11px] font-mono text-fg/50 whitespace-pre-wrap break-all mb-4">
              {this.state.error.message}
            </pre>
            <button
              onClick={() => this.setState({ error: null })}
              className="rounded-lg border border-fg/15 bg-fg/5 px-4 py-2 text-xs font-medium text-fg/70 hover:bg-fg/10 transition"
            >
              Try again
            </button>
          </div>
        </div>
      );
    }
    return this.props.children;
  }
}

export function LogsPage() {
  return (
    <LogsErrorBoundary>
      <LogsPageInner />
    </LogsErrorBoundary>
  );
}
