import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { useQueries, useQuery, useQueryClient } from "@tanstack/react-query";
import { useLocation, useNavigate, useRouter, useSearch } from "@tanstack/react-router";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useTranslation } from "react-i18next";
import {
  ArrowDownToLine,
  ChevronDown,
  Clock,
  ChevronUp,
  ClipboardList,
  Copy,
  Crosshair,
  Download,
  EyeOff,
  Filter,
  Link2,
  MoreHorizontal,
  Radio,
  RefreshCw,
  Settings2,
  SlidersHorizontal,
  Trash2,
  TriangleAlert,
  X,
} from "lucide-react";
import {
  Button,
  Checkbox,
  Chip,
  CodeBlock,
  CrispStringsProvider,
  EmptyState,
  Field,
  IconButton,
  Input,
  Kbd,
  ListGroup,
  ListRowButton,
  Menu,
  MenuContent,
  MenuItem,
  MenuLabel,
  MenuSeparator,
  MenuTrigger,
  Page,
  Popover,
  PopoverContent,
  PopoverTrigger,
  SearchField,
  Segmented,
  Select,
  Sheet,
  Skeleton,
  Text,
  ToggleGroup,
  Tooltip,
  cn,
  icon,
  toast,
} from "@lettuceai/crisp";
import { ApiFailure } from "@/api/client";
import type { ApiClient } from "@/api/client";
import type { LogLineView, LogLevel, LogViewFilter } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { useClosingValue } from "@/shared/hooks/useClosingValue";
import { FailureState } from "@/shared/ui/FailureState";
import { SelectMenuField } from "@/shared/ui/SelectMenuField";
import { useSettingsApi, useSettingsLayout } from "../data";
import {
  dayLabel,
  defaultLevels,
  filesQuery,
  levels as allLevels,
  matcherFor,
  pageSize,
  parseLine,
  rawText,
  viewQuery,
  type Line,
  type Panel,
  type SearchOption,
  type Size,
} from "./data";
import { useLogActions } from "./use-actions";

const LEVEL_TEXT: Record<LogLevel, string> = {
  error: "text-danger",
  warn: "text-warning",
  info: "text-info",
  debug: "text-fg-4",
};
const LEVEL_DOT: Record<LogLevel, string> = {
  error: "bg-danger",
  warn: "bg-warning",
  info: "bg-info",
  debug: "bg-fg-4",
};
const SIZE_CLASS: Record<Size, string> = {
  sm: "text-2xs leading-5",
  md: "text-xs leading-5",
  lg: "text-sm leading-6",
};
const uuidOf = (s: string) =>
  /[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/i.exec(s)?.[0] ?? null;

export function LogsPage() {
  const { t, i18n } = useTranslation("logs");
  const api = useSettingsApi();
  const { phone, back } = useSettingsLayout();
  const search = useSearch({ from: "/settings/$section" });
  const navigate = useNavigate();
  const files = useQuery(filesQuery(api));
  const actions = useLogActions(api);
  const filename =
    search.logFile && files.data?.files.includes(search.logFile)
      ? search.logFile
      : files.data?.files[0];
  const report = () =>
    void navigate({
      to: "/settings/$section",
      params: { section: "logs" },
      search: { logPanel: "report" },
      state: { logPanelPushed: true },
      resetScroll: false,
    });
  return (
    <CrispStringsProvider
      locale={i18n.language}
      strings={{
        sheetClose: t("Close"),
        searchFieldClear: t("Clear search"),
        copyButtonCopy: t("Copy"),
        copyButtonCopied: t("Copied"),
      }}
    >
      {filename ? (
        <LogViewer
          key={filename}
          api={api}
          day={filename}
          days={files.data?.files ?? []}
          phone={phone}
          back={back}
          actions={actions}
        />
      ) : (
        <Page
          title={t("Logs")}
          {...back}
          actions={
            <IconButton label={t("Diagnostics report")} onClick={report}>
              <ClipboardList size={icon.lg} />
            </IconButton>
          }
        >
          {files.isPending ? (
            <div role="status" aria-label={t("Loading logs")} className="space-y-3">
              {Array.from({ length: 8 }, (_, index) => (
                <Skeleton key={index} className="h-6 w-full" />
              ))}
            </div>
          ) : files.isError ? (
            <FailureState error={files.error} onRetry={() => void files.refetch()} />
          ) : (
            <EmptyState
              icon={<ClipboardList size={icon.xl} />}
              title={t("No logs yet")}
              description={t("The app writes one file per day while it runs.")}
            />
          )}
          <Diagnostics api={api} />
        </Page>
      )}
    </CrispStringsProvider>
  );
}

function LogViewer({
  api,
  day,
  days,
  phone,
  back,
  actions,
}: {
  api: ApiClient;
  day: string;
  days: string[];
  phone: boolean;
  back: ReturnType<typeof useSettingsLayout>["back"];
  actions: ReturnType<typeof useLogActions>;
}) {
  "use no memo";
  const { t, i18n } = useTranslation("logs");
  const LEVELS = allLevels;
  const DEFAULT_LEVELS = defaultLevels;
  const LEVEL_LABEL: Record<LogLevel, string> = {
    error: t("Error"),
    warn: t("Warn"),
    info: t("Info"),
    debug: t("Debug"),
  };
  const plural = (count: number, one: string) =>
    t(one === "debug line" ? "debugLines" : "lines", { count });
  const client = useQueryClient();
  const search = useSearch({ from: "/settings/$section" });
  const navigate = useNavigate();
  const router = useRouter();
  const routeKey = useLocation({ select: (location) => location.state.__TSR_key });
  const pushed = useLocation({ select: (location) => location.state.logPanelPushed });
  const panel = search.logPanel;
  const [levels, setLevels] = useState<LogLevel[]>([...defaultLevels]);
  const [hidden, setHidden] = useState<string[]>([]);
  const [from, setFrom] = useState("");
  const [to, setTo] = useState("");
  const [size, setSize] = useState<Size>("md");
  const [wrap, setWrap] = useState(true);
  const [showAt, setShowAt] = useState(false);
  const [query, setQuery] = useState("");
  const [settledQuery, setSettledQuery] = useState("");
  const [opts, setOpts] = useState<SearchOption[]>([]);
  const [onlyMatches, setOnlyMatches] = useState(false);
  const [active, setActive] = useState(0);
  const [selected, setSelected] = useState<number[]>([]);
  const selectedRows = useRef(new Map<number, LogLineView>());
  const [focus, setFocus] = useState<number | null>(null);
  const [isolated, setIsolated] = useState<{ label: string; lines: number[] } | null>(null);
  const scrollRef = useRef<HTMLElement>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const viewport = useRef<HTMLDivElement>(null);
  const [scrollMargin, setScrollMargin] = useState(0);
  const [filtersOpen, moreOpen] = [panel === "filters", panel === "more"];
  const close = () => {
    if (router.state.location.state.__TSR_key !== routeKey) return;
    if (pushed) router.history.back();
    else
      void navigate({
        to: "/settings/$section",
        params: { section: "logs" },
        search: { logFile: day },
        replace: true,
        resetScroll: false,
      });
  };
  const open = (next: Panel, line?: number) =>
    void navigate({
      to: "/settings/$section",
      params: { section: "logs" },
      search: { logFile: day, logPanel: next, ...(line === undefined ? {} : { logLine: line }) },
      state: { logPanelPushed: panel === undefined ? true : (pushed ?? false) },
      replace: panel !== undefined,
      resetScroll: false,
    });
  const setDay = (name: string) => {
    void navigate({
      to: "/settings/$section",
      params: { section: "logs" },
      search: { logFile: name },
      replace: true,
      resetScroll: true,
    });
  };
  const setFiltersOpen = (on: boolean) => (on ? open("filters") : close());
  const setMoreOpen = (on: boolean) => (on ? open("more") : close());
  const setReportOpen = (on: boolean) => (on ? open("report") : close());
  const confirm = panel === "delete-all" ? "all" : panel === "delete-day" ? "day" : null;
  const shownConfirm = useClosingValue(confirm).shown;
  const setConfirm = (value: "all" | "day" | null) =>
    value ? open(value === "all" ? "delete-all" : "delete-day") : close();
  useEffect(() => {
    const timer = setTimeout(() => setSettledQuery(query), 180);
    return () => clearTimeout(timer);
  }, [query]);
  const filter: LogViewFilter = useMemo(
    () => ({
      levels,
      hidden_components: hidden,
      from: from || null,
      to: to || null,
      query: settledQuery,
      case_sensitive: opts.includes("case"),
      whole_word: opts.includes("word"),
      regex: opts.includes("regex"),
      only_matches: onlyMatches,
      isolation: isolated ? isolated.lines.map((n) => n - 1) : null,
    }),
    [levels, hidden, from, to, settledQuery, opts, onlyMatches, isolated],
  );
  const selectionKey = JSON.stringify(filter);
  const selectionSession = useRef({});
  const [selectionFilter, setSelectionFilter] = useState(selectionKey);
  if (selectionFilter !== selectionKey) {
    setSelectionFilter(selectionKey);
    setSelected([]);
  }
  useLayoutEffect(() => {
    selectionSession.current = {};
    selectedRows.current.clear();
    return () => {
      selectionSession.current = {};
    };
  }, [selectionKey]);
  const head = useQuery(viewQuery(api, day, filter));
  const metadata = useClosingValue(head.data ?? null).shown;
  const totalVisible = head.data?.filtered_total ?? 0;
  const COMPONENTS = metadata?.components.map((entry) => entry.component) ?? [];
  const componentCounts = new Map(
    metadata?.components.map((entry) => [entry.component, entry.count]) ?? [],
  );
  const COUNTS = metadata?.counts ?? { error: 0, warn: 0, info: 0, debug: 0 };
  const matches = head.data?.matches.map((entry) => entry.index + 1) ?? [];
  const visibleProblems = head.data?.problems.map((entry) => entry.index + 1) ?? [];
  const regexError =
    head.isError &&
    head.error instanceof ApiFailure &&
    head.error.code === "invalid_input" &&
    head.error.details?.type === "invalid_field" &&
    head.error.details.field === "query";
  const matcher = useMemo(() => matcherFor(query, opts), [query, opts]);
  const activeLine = matches.length ? matches[Math.min(active, matches.length - 1)] : null;
  const narrowed =
    Boolean(isolated) ||
    hidden.length > 0 ||
    levels.length < allLevels.length ||
    Boolean(from || to) ||
    (onlyMatches && Boolean(settledQuery));
  useLayoutEffect(() => {
    const element = viewport.current,
      scroller = scrollRef.current;
    if (!element || !scroller) return;
    const measure = () =>
      setScrollMargin(
        element.getBoundingClientRect().top -
          scroller.getBoundingClientRect().top +
          scroller.scrollTop,
      );
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    observer.observe(scroller);
    return () => observer.disconnect();
  }, [totalVisible]);
  const virtualizer = useVirtualizer({
    count: totalVisible,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => (phone ? 56 : size === "lg" ? 28 : 24),
    overscan: 12,
    scrollMargin,
    initialRect: { width: 1000, height: 600 },
  });
  useLayoutEffect(() => {
    virtualizer.measure();
  }, [virtualizer, size, wrap, phone, showAt]);
  const virtualRows = virtualizer.getVirtualItems();
  const offsets = [
    ...new Set(virtualRows.map((row) => Math.floor(row.index / pageSize) * pageSize)),
  ].filter((offset) => offset !== 0);
  const pages = useQueries({
    queries: offsets.map((offset) => viewQuery(api, day, filter, offset)),
  });
  const available = [head, ...pages];
  const rowByPosition = new Map<number, Line>();
  for (const page of available)
    for (const line of page.data?.lines ?? []) rowByPosition.set(line.position, parseLine(line));
  const lineKey = [...queryKeys.logs.all, "line", day, search.logLine];
  const lineQuery = useQuery({
    queryKey: lineKey,
    enabled: panel === "line" && search.logLine !== undefined,
    queryFn: async () => {
      const result = await api.call("logReadPage", {
        name: day,
        offset: search.logLine ?? 0,
        limit: 1,
      });
      if (!result.lines.length)
        throw new ApiFailure("not_found", "log line no longer exists", null);
      return parseLine({ index: search.logLine ?? 0, position: 0, raw: result.lines[0]! });
    },
  });
  const lineSheet = panel === "line" ? (lineQuery.data ?? null) : null;
  const shownLine = useClosingValue(lineSheet).shown;
  const setLineSheet = (line: Line | null) => {
    if (line) {
      client.setQueryData([...queryKeys.logs.all, "line", day, line.index], line);
      open("line", line.index);
    } else close();
  };
  const follow = actions.follow;
  const setFollow = (fn: (value: boolean) => boolean) => {
    actions.setFollow(fn);
    if (!follow) void actions.refresh();
  };
  const newest = useCallback(() => {
    if (totalVisible) virtualizer.scrollToIndex(totalVisible - 1, { align: "end" });
  }, [totalVisible, virtualizer]);
  const activePosition = head.data?.matches.find(
    (entry) => entry.index === (activeLine ?? 0) - 1,
  )?.position;
  const focusPosition =
    !head.data || focus === null
      ? undefined
      : (head.data.matches.find((entry) => entry.index === focus - 1)?.position ??
        head.data.problems.find((entry) => entry.index === focus - 1)?.position ??
        (!narrowed ? focus - 1 : undefined));
  useEffect(() => {
    if (activePosition !== undefined)
      virtualizer.scrollToIndex(activePosition, { align: "center" });
  }, [activePosition, virtualizer]);
  useEffect(() => {
    if (focusPosition !== undefined) virtualizer.scrollToIndex(focusPosition, { align: "center" });
  }, [focusPosition, virtualizer]);
  useEffect(() => {
    if (follow) newest();
  }, [follow, head.dataUpdatedAt, newest]);
  const stepMatch = (delta: 1 | -1) => {
    if (!matches.length) return;
    actions.setFollow(false);
    const next = (Math.min(active, matches.length - 1) + delta + matches.length) % matches.length;
    setActive(next);
    const target = head.data?.matches[next];
    if (target) virtualizer.scrollToIndex(target.position, { align: "center" });
  };
  const stepProblem = (delta: 1 | -1) => {
    if (!visibleProblems.length) return;
    actions.setFollow(false);
    const current = focus ?? (delta === 1 ? 0 : Infinity);
    const next =
      delta === 1
        ? (visibleProblems.find((n) => n > current) ?? visibleProblems[0]!)
        : ([...visibleProblems].reverse().find((n) => n < current) ?? visibleProblems.at(-1)!);
    setFocus(next);
    setSelected([next]);
    const target = head.data?.problems.find((entry) => entry.index === next - 1);
    if (target) virtualizer.scrollToIndex(target.position, { align: "center" });
  };
  const copyRows = async (numbers: number[]) => {
    const rows: LogLineView[] = [];
    for (const n of numbers) {
      const cached =
        selectedRows.current.get(n) ??
        [...rowByPosition.values()].find((line) => line.index === n - 1);
      if (cached) rows.push(cached);
      else {
        const result = await api.call("logReadPage", { name: day, offset: n - 1, limit: 1 });
        if (!result.lines.length)
          throw new ApiFailure("not_found", "selected log line is gone", null);
        rows.push({ index: n - 1, position: 0, raw: result.lines[0]! });
      }
    }
    await navigator.clipboard.writeText(rawText(rows));
    toast.success(t("Copied"));
  };
  const copySelected = () => void actions.run(() => copyRows(selected));
  const saveSelected = () =>
    void actions.save(day, { type: "indices", indices: selected.map((n) => n - 1) });
  const saveVisible = () => void actions.save(day, { type: "filter", filter });
  const isolate = (iso: { label: string; lines: number[] }) => {
    setIsolated(iso);
    setSelected([]);
    setFocus(null);
    if (panel) close();
  };
  const related = (line: Line) =>
    void actions.run(async () => {
      const result = await api.call("logRelevantLines", { name: day, reference_line: line.raw });
      isolate({
        label: t("Related to line {{number}}", { number: line.index + 1 }),
        lines: result.matches.map((index) => index + 1),
      });
    });
  const resetAll = () => {
    setIsolated(null);
    setLevels([...allLevels]);
    setHidden([]);
    setFrom("");
    setTo("");
    setOnlyMatches(false);
  };
  const inContext = (n: number) => {
    setSelectionFilter(
      JSON.stringify({
        ...filter,
        levels: [...allLevels],
        hidden_components: [],
        from: null,
        to: null,
        only_matches: false,
        isolation: null,
      }),
    );
    resetAll();
    setSelected([n]);
    setFocus(n);
    if (panel) close();
  };
  const onlyComponent = (component: string) => {
    setIsolated(null);
    setHidden(COMPONENTS.filter((entry) => entry !== component));
    setSelected([]);
    if (panel) close();
  };
  const hideComponent = (component: string) => {
    setIsolated(null);
    setHidden((value) => [...new Set([...value, component])]);
    setSelected([]);
    if (panel) close();
    toast.info(t("Hiding {{component}}", { component }), undefined, {
      action: {
        label: t("Undo"),
        onClick: () => setHidden((value) => value.filter((entry) => entry !== component)),
      },
    });
  };
  const highlight = (text: string): ReactNode => {
    if (!matcher) return text;
    const parts: ReactNode[] = [];
    let last = 0;
    matcher.lastIndex = 0;
    for (const match of text.matchAll(matcher)) {
      if (match.index === undefined || !match[0]) break;
      parts.push(
        text.slice(last, match.index),
        <mark key={match.index} className="rounded-sm bg-warning/35 px-px text-fg">
          {match[0]}
        </mark>,
      );
      last = match.index + match[0].length;
    }
    parts.push(text.slice(last));
    return parts;
  };
  const click = (line: Line, event: React.MouseEvent) => {
    if (window.getSelection()?.toString()) return;
    if (phone) {
      setLineSheet(line);
      return;
    }
    setFocus(null);
    selectedRows.current.set(line.index + 1, line);
    if (event.shiftKey && selected.length) {
      const anchor =
        selectedRows.current.get(selected.at(-1)!) ??
        [...rowByPosition.values()].find((row) => row.index === selected.at(-1)! - 1);
      if (!anchor) return;
      const start = Math.min(anchor.position, line.position),
        end = Math.max(anchor.position, line.position);
      const session = selectionSession.current;
      void actions.run(async () => {
        const rows: LogLineView[] = [];
        try {
          for (let offset = start; offset <= end; offset += pageSize) {
            if (session !== selectionSession.current) return;
            const result = await api.call("logQuery", {
              name: day,
              filter,
              offset,
              limit: Math.min(pageSize, end - offset + 1),
            });
            if (session !== selectionSession.current) return;
            rows.push(...result.lines);
            if (!result.lines.length) break;
          }
        } catch (failure) {
          if (session === selectionSession.current) throw failure;
          return;
        }
        for (const row of rows) selectedRows.current.set(row.index + 1, row);
        setSelected((previous) => [...new Set([...previous, ...rows.map((row) => row.index + 1)])]);
      });
    } else if (event.metaKey || event.ctrlKey)
      setSelected((value) =>
        value.includes(line.index + 1)
          ? value.filter((n) => n !== line.index + 1)
          : [...value, line.index + 1],
      );
    else
      setSelected((value) =>
        value.length === 1 && value[0] === line.index + 1 ? [] : [line.index + 1],
      );
  };
  useEffect(() => {
    if (phone || panel) return;
    const onKey = (event: KeyboardEvent) => {
      const typing =
        event.target instanceof HTMLElement &&
        (event.target.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(event.target.tagName));
      if (
        (event.key === "f" && (event.ctrlKey || event.metaKey)) ||
        (event.key === "/" && !typing)
      ) {
        event.preventDefault();
        searchRef.current?.focus();
        return;
      }
      if (typing) return;
      if (event.key === "e") stepProblem(1);
      if (event.key === "E") stepProblem(-1);
      if (event.key === "n") stepMatch(1);
      if (event.key === "N") stepMatch(-1);
      if (event.key === "Escape") {
        setSelected([]);
        setFocus(null);
      }
      if (
        event.key === "c" &&
        (event.ctrlKey || event.metaKey) &&
        selected.length &&
        !window.getSelection()?.toString()
      ) {
        event.preventDefault();
        copySelected();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });
  const lineActions = (line: Line) => (
    <div className="flex flex-wrap items-center gap-1 font-sans">
      <Button
        size="sm"
        variant="ghost"
        leading={<Copy size={icon.sm} />}
        onClick={() => void actions.run(() => copyRows([line.index + 1]))}
      >
        {t("Copy")}
      </Button>
      <Button
        size="sm"
        variant="ghost"
        leading={<Link2 size={icon.sm} />}
        onClick={() => related(line)}
      >
        {t("Related lines")}
      </Button>
      {narrowed && (
        <Button
          size="sm"
          variant="ghost"
          leading={<Crosshair size={icon.sm} />}
          onClick={() => inContext(line.index + 1)}
        >
          {t("Show in full log")}
        </Button>
      )}
      <Button
        size="sm"
        variant="ghost"
        leading={<Filter size={icon.sm} />}
        disabled={!line.component}
        onClick={() => onlyComponent(line.component)}
      >
        {t("Only {{component}}", { component: line.component })}
      </Button>
      <Button
        size="sm"
        variant="ghost"
        leading={<EyeOff size={icon.sm} />}
        disabled={!line.component}
        onClick={() => hideComponent(line.component)}
      >
        {t("Hide {{component}}", { component: line.component })}
      </Button>
    </div>
  );
  const row = (l: Line) => {
    const on = selected.includes(l.n);
    const open = !phone && selected.length === 1 && selected[0] === l.n;
    const isMatch = matches.includes(l.n);
    return (
      <div key={l.n}>
        <Button
          variant="ghost"
          aria-pressed={on}
          onClick={(e) => click(l, e)}
          onContextMenu={(e) => {
            e.preventDefault();
            setLineSheet(l);
          }}
          className={cn(
            "flex h-auto min-h-0 w-full cursor-default select-text justify-start whitespace-normal rounded-none border-l-2 py-0 text-left font-mono font-normal [&>span]:w-full [&>span]:min-w-0",
            phone
              ? "gap-0.5 px-4 py-1.5 [&>span]:flex-col [&>span]:items-start"
              : "gap-3 pl-4 pr-6",
            SIZE_CLASS[size],
            l.level === "error"
              ? "border-danger bg-danger/5"
              : l.level === "warn"
                ? "border-warning/70"
                : "border-transparent",
            on ? "bg-accent/10" : "hover:bg-fill",
            isMatch && activeLine !== l.n && !on && "bg-warning/10",
            activeLine === l.n && "ring-1 ring-inset ring-warning/70",
          )}
        >
          {phone ? (
            <>
              <span className="flex w-full min-w-0 items-center gap-2 text-fg-3">
                <span className="shrink-0 tabular-nums">{l.time.slice(0, 8)}</span>
                <span className={cn("shrink-0 font-semibold uppercase", LEVEL_TEXT[l.level ?? "debug"])}>
                  {l.level}
                </span>
                <span className="min-w-0 truncate text-accent">{highlight(l.component)}</span>
              </span>
              <span
                className={cn(
                  "w-full min-w-0 wrap-anywhere",
                  l.level === "error" ? "text-fg" : "text-fg-2",
                  !wrap && "truncate",
                )}
              >
                {highlight(l.message)}
              </span>
            </>
          ) : (
            <>
              <span className="w-8 shrink-0 select-none text-right tabular-nums text-fg-4">
                {l.n}
              </span>
              <span className="shrink-0 tabular-nums text-fg-3">{l.time}</span>
              <span
                className={cn(
                  "w-10 shrink-0 font-semibold uppercase",
                  LEVEL_TEXT[l.level ?? "debug"],
                )}
              >
                {l.level}
              </span>
              <span className="w-28 shrink-0 truncate text-accent" title={l.component}>
                {highlight(l.component)}
              </span>
              <span
                className={cn(
                  "min-w-0 flex-1",
                  wrap ? "wrap-anywhere" : "truncate",
                  l.level === "error" ? "text-fg" : "text-fg-2",
                )}
              >
                {showAt && <span className="text-fg-4">{l.at} </span>}
                {highlight(l.message)}
              </span>
            </>
          )}
        </Button>
        {open && (
          <div className="border-b border-l-2 border-line border-l-accent bg-fill/60 py-2 pl-16 pr-6">
            {!wrap && (
              <p className={cn("mb-1 wrap-anywhere font-mono text-fg", SIZE_CLASS[size])}>
                {l.message}
              </p>
            )}
            {!showAt && (
              <p className="mb-1 break-all font-mono text-2xs text-fg-4">
                {t("at {{location}}", { location: l.at })}
              </p>
            )}
            {lineActions(l)}
          </div>
        )}
      </div>
    );
  };

  const searchOptions = (
    <ToggleGroup
      size="sm"
      aria-label={t("Search options")}
      value={opts}
      onChange={(v) => setOpts(v.includes("regex") ? v.filter((x) => x !== "word") : v)}
      options={[
        {
          value: "case",
          label: (
            <Tooltip content={t("Match case")}>
              <span>{t("Aa")}</span>
            </Tooltip>
          ),
        },
        {
          value: "word",
          label: (
            <Tooltip content={t("Whole word")}>
              <span>{t("ab")}</span>
            </Tooltip>
          ),
          disabled: opts.includes("regex"),
        },
        {
          value: "regex",
          label: (
            <Tooltip content={t("Regular expression")}>
              <span>{t(".*")}</span>
            </Tooltip>
          ),
        },
      ]}
    />
  );

  const matchNav = query.trim() !== "" && (
    <span className="flex shrink-0 items-center gap-0.5">
      <span
        className={cn(
          "min-w-16 text-center text-sm tabular-nums",
          matches.length ? "text-fg-2" : "text-danger",
        )}
      >
        {regexError
          ? t("Bad regex")
          : matches.length
            ? t("matchCount", {
                current: Math.min(active, matches.length - 1) + 1,
                count: matches.length,
              })
            : t("No match")}
      </span>
      <IconButton
        label={t("Previous match")}
        size="sm"
        disabled={!matches.length}
        onClick={() => stepMatch(-1)}
      >
        <ChevronUp size={icon.md} />
      </IconButton>
      <IconButton
        label={t("Next match")}
        size="sm"
        disabled={!matches.length}
        onClick={() => stepMatch(1)}
      >
        <ChevronDown size={icon.md} />
      </IconButton>
    </span>
  );

  const searchField = (
    <SearchField
      ref={searchRef}
      size="sm"
      value={query}
      {...(phone ? {} : { shortcut: "/" })}
      onChange={(v) => {
        setQuery(v);
        setActive(0);
      }}
      onKeyDown={(e) => {
        if (e.key === "Enter") {
          e.preventDefault();
          stepMatch(e.shiftKey ? -1 : 1);
        }
      }}
      placeholder={t(opts.includes("regex") ? "Regular expression" : "Search the whole day")}
      aria-label={t("Search logs")}
      aria-invalid={regexError || undefined}
    />
  );

  const levelToggles = (
    <ToggleGroup
      size="sm"
      aria-label={t("Levels")}
      value={levels}
      onChange={(v) => {
        setLevels(v as LogLevel[]);
        setIsolated(null);
      }}
      options={LEVELS.map((lv) => ({
        value: lv,
        label: (
          <span className="flex items-center gap-1.5">
            <span className={cn("h-1.5 w-1.5 rounded-full", LEVEL_DOT[lv])} />
            {LEVEL_LABEL[lv]}
            <span className="tabular-nums text-fg-3">{COUNTS[lv]}</span>
          </span>
        ),
      }))}
    />
  );

  const componentPicker = (
    <Popover>
      <PopoverTrigger>
        <Button size="sm" variant="secondary" trailing={<ChevronDown size={icon.sm} />}>
          {hidden.length
            ? t("Parts count", {
                shown: COMPONENTS.length - hidden.length,
                total: COMPONENTS.length,
              })
            : t("All parts")}
        </Button>
      </PopoverTrigger>
      <PopoverContent className="w-64 p-2">
        <div className="flex items-center justify-between px-2 pb-1">
          <Text variant="label">{t("Parts of the app")}</Text>
          <Button
            size="sm"
            variant="ghost"
            onClick={() => setHidden(hidden.length ? [] : [...COMPONENTS])}
          >
            {t(hidden.length ? "All" : "None")}
          </Button>
        </div>
        <div className="space-y-0.5">
          {COMPONENTS.map((c) => (
            <div key={c} className="flex items-center gap-2 rounded-lg px-2 py-1 hover:bg-fill">
              <Checkbox
                className="min-w-0 flex-1"
                checked={!hidden.includes(c)}
                onChange={(on) => setHidden((h) => (on ? h.filter((x) => x !== c) : [...h, c]))}
                label={
                  <span className="flex items-center gap-2">
                    <span className="font-mono text-sm">{c}</span>
                    <span className="text-xs tabular-nums text-fg-4">
                      {componentCounts.get(c) ?? 0}
                    </span>
                  </span>
                }
              />
              <button
                type="button"
                className="text-xs text-fg-3 hover:text-accent"
                onClick={() => onlyComponent(c)}
              >
                {t("Only")}
              </button>
            </div>
          ))}
        </div>
      </PopoverContent>
    </Popover>
  );

  const timeRange = (
    <Popover>
      <PopoverTrigger>
        <Button
          size="sm"
          variant="secondary"
          leading={<Clock size={icon.sm} />}
          trailing={<ChevronDown size={icon.sm} />}
        >
          {from || to
            ? t("timeRange", { from: from || t("Start"), to: to || t("end") })
            : t("Any time")}
        </Button>
      </PopoverTrigger>
      <PopoverContent className="w-72 space-y-3 p-3">
        <div className="grid grid-cols-2 gap-2">
          <Field label={t("From")}>
            <Input
              type="time"
              step={1}
              value={from}
              onChange={(e) => setFrom(e.target.value)}
              className="tabular-nums"
            />
          </Field>
          <Field label={t("To")}>
            <Input
              type="time"
              step={1}
              value={to}
              onChange={(e) => setTo(e.target.value)}
              className="tabular-nums"
            />
          </Field>
        </div>
        {selected.length > 0 && (
          <Button
            size="sm"
            variant="ghost"
            onClick={() => {
              const t = (
                selectedRows.current.get(selected[0]!)
                  ? parseLine(selectedRows.current.get(selected[0]!)!).time
                  : ""
              ).slice(0, 8);
              setFrom(t);
            }}
          >
            {t("From the selected line")}
          </Button>
        )}
        <div className="flex justify-end">
          <Button
            size="sm"
            variant="ghost"
            disabled={!from && !to}
            onClick={() => {
              setFrom("");
              setTo("");
            }}
          >
            {t("Clear")}
          </Button>
        </div>
      </PopoverContent>
    </Popover>
  );

  const viewMenu = (
    <Menu>
      <MenuTrigger>
        <IconButton label={t("View")} size="sm">
          <Settings2 size={icon.md} />
        </IconButton>
      </MenuTrigger>
      <MenuContent align="end" className="w-64">
        <MenuItem selected={wrap} onSelect={() => setWrap((w) => !w)}>
          {t("Wrap long lines")}
        </MenuItem>
        <MenuItem selected={showAt} onSelect={() => setShowAt((s) => !s)}>
          {t("Show code location")}
        </MenuItem>
        <MenuSeparator />
        <MenuLabel>{t("Text size")}</MenuLabel>
        {(["sm", "md", "lg"] as const).map((s) => (
          <MenuItem key={s} selected={size === s} onSelect={() => setSize(s)}>
            {t(s === "sm" ? "Small" : s === "md" ? "Medium" : "Large")}
          </MenuItem>
        ))}
        <MenuSeparator />
        <MenuLabel>{t("Keys")}</MenuLabel>
        <div className="grid grid-cols-[auto_1fr] items-center gap-x-3 gap-y-1.5 px-3 pb-2 pt-1 text-xs text-fg-3">
          <span className="flex gap-1">
            <Kbd>{t("/")}</Kbd>
          </span>
          {t("Search")}
          <span className="flex gap-1">
            <Kbd>{t("n")}</Kbd>
            <Kbd>{t("N")}</Kbd>
          </span>
          {t("Next, previous match")}
          <span className="flex gap-1">
            <Kbd>{t("e")}</Kbd>
            <Kbd>{t("E")}</Kbd>
          </span>
          {t("Next, previous problem")}
          <span className="flex gap-1">
            <Kbd>{t("Shift")}</Kbd>
          </span>
          {t("Click to select a range")}
          <span className="flex gap-1">
            <Kbd>{t("Esc")}</Kbd>
          </span>
          {t("Clear selection")}
        </div>
      </MenuContent>
    </Menu>
  );

  const problemNav = (
    <span className="flex shrink-0 items-center gap-0.5">
      <Tooltip content={t("Previous error or warning (E)")}>
        <IconButton
          label={t("Previous problem")}
          size="sm"
          disabled={!visibleProblems.length}
          onClick={() => stepProblem(-1)}
        >
          <ChevronUp size={icon.md} />
        </IconButton>
      </Tooltip>
      <span className="flex items-center gap-1 px-1 text-sm tabular-nums text-fg-2">
        <TriangleAlert size={icon.sm} className="text-danger" />
        {visibleProblems.length}
      </span>
      <Tooltip content={t("Next error or warning (e)")}>
        <IconButton
          label={t("Next problem")}
          size="sm"
          disabled={!visibleProblems.length}
          onClick={() => stepProblem(1)}
        >
          <ChevronDown size={icon.md} />
        </IconButton>
      </Tooltip>
    </span>
  );

  const activeChips = (
    <>
      {isolated && (
        <Chip selected onRemove={() => setIsolated(null)} icon={<Link2 size={icon.sm} />}>
          {t("isolationCount", { label: isolated.label, count: isolated.lines.length })}
        </Chip>
      )}
      {onlyMatches && Boolean(settledQuery) && (
        <Chip selected onRemove={() => setOnlyMatches(false)}>
          {t("Only lines matching {{query}}", { query })}
        </Chip>
      )}
      {hidden.length > 0 && (
        <Chip selected onRemove={() => setHidden([])}>
          {hidden.length > COMPONENTS.length / 2
            ? t("Only {{component}}", {
                component: COMPONENTS.filter((c) => !hidden.includes(c)).join(", "),
              })
            : t("Hiding {{component}}", { component: hidden.join(", ") })}
        </Chip>
      )}
      {(from || to) && (
        <Chip
          selected
          onRemove={() => {
            setFrom("");
            setTo("");
          }}
        >
          {t("timeRange", { from: from || t("start"), to: to || t("end") })}
        </Chip>
      )}
    </>
  );
  const hasChips =
    Boolean(isolated) ||
    (onlyMatches && Boolean(matcher)) ||
    hidden.length > 0 ||
    Boolean(from || to);

  const desktopToolbar = (
    <div className="space-y-2">
      <div className="flex items-center gap-2">
        <SelectMenuField
          className="w-52 shrink-0"
          aria-label={t("Day")}
          value={day}
          onChange={(v) => v && setDay(v)}
          options={days.map((d) => ({
            value: d,
            label: dayLabel(d, i18n.language, new Date(), {
              today: t("Today"),
              yesterday: t("Yesterday"),
            }),
          }))}
        />
        <div className="min-w-0 flex-1">{searchField}</div>
        {searchOptions}
        <Tooltip content={t("Hide lines that don't match")}>
          <Button
            size="sm"
            variant={onlyMatches ? "primary" : "secondary"}
            leading={<Filter size={icon.sm} />}
            disabled={!settledQuery || query !== settledQuery || regexError || head.isPending}
            onClick={() => setOnlyMatches((o) => !o)}
          >
            {t("Only matches")}
          </Button>
        </Tooltip>
        {matchNav}
      </div>
      <div className="flex items-center gap-2">
        {levelToggles}
        {componentPicker}
        {timeRange}
        <span className="flex-1" />
        {problemNav}
        {viewMenu}
      </div>
      {hasChips && <div className="flex flex-wrap items-center gap-2">{activeChips}</div>}
    </div>
  );

  const levelsChanged =
    levels.length !== DEFAULT_LEVELS.length || DEFAULT_LEVELS.some((lv) => !levels.includes(lv));
  const phoneFilterCount = (levelsChanged ? 1 : 0) + (hidden.length ? 1 : 0) + (from || to ? 1 : 0);

  const phoneToolbar = (
    <div className="space-y-2">
      <div className="flex items-center gap-2">
        <div className="min-w-0 flex-1">
          <Select
            aria-label={t("Day")}
            value={day}
            onChange={(e) => setDay(e.target.value)}
            options={days.map((d) => ({
              value: d,
              label: dayLabel(d, i18n.language, new Date(), {
                today: t("Today"),
                yesterday: t("Yesterday"),
              }),
            }))}
          />
        </div>
        <Button
          variant="secondary"
          leading={<SlidersHorizontal size={icon.sm} />}
          onClick={() => setFiltersOpen(true)}
        >
          {phoneFilterCount ? t("Filters count", { count: phoneFilterCount }) : t("Filters")}
        </Button>
      </div>
      {searchField}
      {query.trim() && (
        <div className="flex items-center gap-2">
          {searchOptions}
          <Button
            size="sm"
            variant={onlyMatches ? "primary" : "secondary"}
            disabled={!settledQuery || query !== settledQuery || regexError || head.isPending}
            onClick={() => setOnlyMatches((o) => !o)}
          >
            {t("Only matches")}
          </Button>
          <span className="flex-1" />
          {matchNav}
        </div>
      )}
      {hasChips && <div className="flex flex-wrap gap-2">{activeChips}</div>}
    </div>
  );

  const statusBar = (
    <div
      className={cn(
        "sticky bottom-0 z-20 mt-auto flex items-center gap-2 border-t border-line bg-surface/95 backdrop-blur-md",
        phone ? "px-4 py-2 pb-[calc(0.5rem+var(--safe-bottom,0px))]" : "px-6 py-1.5",
      )}
    >
      {selected.length > 1 && !phone ? (
        <>
          <span className="text-sm tabular-nums text-fg-2">
            {t("selectedLines", { count: selected.length })}
          </span>
          <Button
            size="sm"
            variant="ghost"
            leading={<Copy size={icon.sm} />}
            onClick={() => copySelected()}
          >
            {t("Copy")}
          </Button>
          <Button
            size="sm"
            variant="ghost"
            leading={<Download size={icon.sm} />}
            onClick={() => saveSelected()}
          >
            {t("Save")}
          </Button>
          <Button
            size="sm"
            variant="ghost"
            leading={<Filter size={icon.sm} />}
            onClick={() =>
              isolate({ label: t("Selected lines"), lines: [...selected].sort((a, b) => a - b) })
            }
          >
            {t("Show only these")}
          </Button>
          <span className="flex-1" />
          <IconButton label={t("Clear selection")} size="sm" onClick={() => setSelected([])}>
            <X size={icon.sm} />
          </IconButton>
        </>
      ) : (
        <>
          <span className="min-w-0 flex-1 truncate text-xs tabular-nums text-fg-3">
            {totalVisible === (head.data?.total ?? 0)
              ? plural(head.data?.total ?? 0, "line")
              : t("shownLines", { shown: totalVisible, count: head.data?.total ?? 0 })}
            {!phone &&
              !levels.includes("debug") &&
              !isolated &&
              t("hiddenDebug", { count: COUNTS.debug })}
            {follow && t("followingSuffix")}
          </span>
          {phone ? (
            problemNav
          ) : (
            <Button
              size="sm"
              variant="ghost"
              leading={<ArrowDownToLine size={icon.sm} />}
              onClick={() => newest()}
            >
              {t("Newest")}
            </Button>
          )}
        </>
      )}
    </div>
  );

  return (
    <>
      <Page
        title={t("Logs")}
        meta={phone ? undefined : days.length ? day : undefined}
        mainRef={scrollRef}
        padding="none"
        footer={statusBar}
        {...back}
        filters={days.length ? (phone ? phoneToolbar : desktopToolbar) : undefined}
        actions={
          days.length ? (
            <>
              <Tooltip
                content={
                  follow
                    ? t("New log lines update as they arrive")
                    : t("Keep the newest lines in view")
                }
              >
                <Button
                  size="sm"
                  variant={follow ? "primary" : "ghost"}
                  leading={<Radio size={icon.sm} />}
                  onClick={() => {
                    setFollow((f) => !f);
                    if (!follow) newest();
                  }}
                >
                  {t(follow ? "Following" : "Follow")}
                </Button>
              </Tooltip>
              <IconButton label={t("Reload")} onClick={() => void actions.refresh()}>
                <RefreshCw size={icon.lg} />
              </IconButton>
              {phone ? (
                <IconButton label={t("More")} onClick={() => setMoreOpen(true)}>
                  <MoreHorizontal size={icon.lg} />
                </IconButton>
              ) : (
                <Menu>
                  <MenuTrigger>
                    <IconButton label={t("More")}>
                      <MoreHorizontal size={icon.lg} />
                    </IconButton>
                  </MenuTrigger>
                  <MenuContent align="end" className="w-64">
                    <MenuItem
                      icon={<ClipboardList size={icon.md} />}
                      onSelect={() => setReportOpen(true)}
                    >
                      {t("Diagnostics report")}
                    </MenuItem>
                    <MenuItem
                      icon={<Download size={icon.md} />}
                      onSelect={() => void actions.save(day)}
                    >
                      {t("Save this day's file")}
                    </MenuItem>
                    <MenuItem
                      icon={<Download size={icon.md} />}
                      disabled={!narrowed}
                      onSelect={() => saveVisible()}
                    >
                      {t("Save shown lines", { count: totalVisible })}
                    </MenuItem>
                    <MenuSeparator />
                    <MenuItem
                      icon={<Trash2 size={icon.md} />}
                      danger
                      onSelect={() => setConfirm("day")}
                    >
                      {t("Delete this day")}
                    </MenuItem>
                    <MenuItem
                      icon={<Trash2 size={icon.md} />}
                      danger
                      onSelect={() => setConfirm("all")}
                    >
                      {t("Delete all days", { count: days.length })}
                    </MenuItem>
                  </MenuContent>
                </Menu>
              )}
            </>
          ) : undefined
        }
      >
        <div ref={viewport} className="relative py-1">
          {(head.isPending || query !== settledQuery) && (
            <div role="status" aria-label={t("Loading logs")} className="space-y-2 px-4">
              {Array.from({ length: 8 }, (_, index) => (
                <Skeleton key={index} className="h-5 w-full" />
              ))}
            </div>
          )}
          {head.isError && (
            <div className="p-4">
              <FailureState error={head.error} onRetry={() => void head.refetch()} />
            </div>
          )}
          {actions.error !== null && (
            <div className="p-4">
              <FailureState error={actions.error} onRetry={actions.retry} />
            </div>
          )}
          {actions.subscriptionError !== null && (
            <div className="p-4">
              <FailureState error={actions.subscriptionError} onRetry={actions.retrySubscription} />
            </div>
          )}
          {!head.isPending &&
            !head.isError &&
            query === settledQuery &&
            (totalVisible ? (
              <div
                style={{ height: virtualizer.getTotalSize(), position: "relative", width: "100%" }}
              >
                {virtualRows.map((item) => {
                  const line = rowByPosition.get(item.index);
                  return (
                    <div
                      key={item.key}
                      ref={virtualizer.measureElement}
                      data-index={item.index}
                      style={{
                        position: "absolute",
                        top: 0,
                        left: 0,
                        width: "100%",
                        transform: `translateY(${item.start - scrollMargin}px)`,
                      }}
                    >
                      {line ? row(line) : <Skeleton className="h-5 w-full" />}
                    </div>
                  );
                })}
              </div>
            ) : (
              <div className="px-6 pt-10">
                <EmptyState
                  size="sm"
                  title={t("No lines match")}
                  description={t("Turn a level back on or loosen the filters.")}
                  action={
                    <Button size="sm" variant="secondary" onClick={resetAll}>
                      {t("Show everything")}
                    </Button>
                  }
                />
              </div>
            ))}
          {pages.some((page) => page.isError) && (
            <div className="p-4">
              <FailureState
                error={pages.find((page) => page.isError)?.error}
                onRetry={() => void actions.refresh()}
              />
            </div>
          )}
        </div>
      </Page>
      <Sheet
        open={panel === "line"}
        onClose={() => setLineSheet(null)}
        title={shownLine ? t("Line {{number}}", { number: shownLine.n }) : undefined}
        description={
          shownLine
            ? `${shownLine.time} · ${(shownLine.level ?? "").toUpperCase()} · ${shownLine.component}`
            : undefined
        }
      >
        {lineQuery.isPending && panel === "line" && <Skeleton className="h-32 w-full" />}
        {lineQuery.isError && (
          <FailureState error={lineQuery.error} onRetry={() => void lineQuery.refetch()} />
        )}
        {actions.error !== null && <FailureState error={actions.error} onRetry={actions.retry} />}
        {shownLine && (
          <div className="space-y-4 pb-2">
            <div className="select-text rounded-xl border border-line bg-fill p-3 font-mono text-xs leading-5 text-fg wrap-anywhere">
              {shownLine.message}
            </div>
            <Text variant="hint" className="break-all font-mono">
              {t("at {{location}}", { location: shownLine.at })}
            </Text>
            <ListGroup>
              <ListRowButton
                icon={<Copy size={icon.lg} />}
                title={t("Copy line")}
                chevron={false}
                onClick={() => {
                  setLineSheet(null);
                  void actions.run(() => copyRows([shownLine.n]));
                }}
              />
              <ListRowButton
                icon={<Link2 size={icon.lg} />}
                title={t("Related lines")}
                description={
                  uuidOf(shownLine.message)
                    ? t("relatedIdBody", {
                        id: uuidOf(shownLine.message)!.split("-")[0],
                        component: shownLine.component,
                      })
                    : t("relatedBody", { component: shownLine.component })
                }
                onClick={() => related(shownLine)}
              />
              {narrowed && (
                <ListRowButton
                  icon={<Crosshair size={icon.lg} />}
                  title={t("Show in full log")}
                  description={t("Clears the filters and keeps this line in view")}
                  chevron={false}
                  onClick={() => inContext(shownLine.n)}
                />
              )}
              <ListRowButton
                icon={<Filter size={icon.lg} />}
                title={t("Only {{component}}", { component: shownLine.component })}
                chevron={false}
                onClick={() => onlyComponent(shownLine.component)}
              />
              <ListRowButton
                icon={<EyeOff size={icon.lg} />}
                title={t("Hide {{component}}", { component: shownLine.component })}
                chevron={false}
                onClick={() => hideComponent(shownLine.component)}
              />
            </ListGroup>
          </div>
        )}
      </Sheet>

      <Sheet open={filtersOpen} onClose={() => setFiltersOpen(false)} title={t("Filters")}>
        <div className="space-y-5 pb-2">
          <section className="space-y-2">
            <Text variant="label" className="px-1">
              {t("Levels")}
            </Text>
            {levelToggles}
          </section>
          <section className="space-y-2">
            <div className="flex items-center justify-between px-1">
              <Text variant="label">{t("Parts of the app")}</Text>
              <Button
                size="sm"
                variant="ghost"
                onClick={() => setHidden(hidden.length ? [] : [...COMPONENTS])}
              >
                {t(hidden.length ? "Show all" : "Hide all")}
              </Button>
            </div>
            <div className="grid gap-2 rounded-xl border border-line p-3">
              {COMPONENTS.map((c) => (
                <Checkbox
                  key={c}
                  checked={!hidden.includes(c)}
                  onChange={(on) => setHidden((h) => (on ? h.filter((x) => x !== c) : [...h, c]))}
                  label={
                    <span className="flex items-center gap-2">
                      <span className="font-mono text-sm">{c}</span>
                      <span className="text-xs tabular-nums text-fg-3">
                        {componentCounts.get(c) ?? 0}
                      </span>
                    </span>
                  }
                />
              ))}
            </div>
          </section>
          <section className="space-y-2">
            <Text variant="label" className="px-1">
              {t("Time")}
            </Text>
            <div className="grid grid-cols-2 gap-3">
              <Field label={t("From")}>
                <Input
                  type="time"
                  step={1}
                  value={from}
                  onChange={(e) => setFrom(e.target.value)}
                />
              </Field>
              <Field label={t("To")}>
                <Input type="time" step={1} value={to} onChange={(e) => setTo(e.target.value)} />
              </Field>
            </div>
          </section>
          <section className="space-y-2">
            <Text variant="label" className="px-1">
              {t("Display")}
            </Text>
            <Segmented
              block
              size="sm"
              aria-label={t("Text size")}
              value={size}
              onChange={setSize}
              options={[
                { value: "sm", label: t("Small") },
                { value: "md", label: t("Medium") },
                { value: "lg", label: t("Large") },
              ]}
            />
            <Checkbox checked={wrap} onChange={setWrap} label={t("Wrap long lines")} />
          </section>
          <div className="flex justify-end gap-2">
            <Button variant="ghost" onClick={resetAll}>
              {t("Reset")}
            </Button>
            <Button onClick={() => setFiltersOpen(false)}>
              {t("Show lines", { count: totalVisible })}
            </Button>
          </div>
        </div>
      </Sheet>

      <Sheet open={moreOpen} onClose={() => setMoreOpen(false)} title={t("Logs")}>
        <div className="space-y-4 pb-2">
          <ListGroup>
            <ListRowButton
              icon={<ClipboardList size={icon.lg} />}
              title={t("Diagnostics report")}
              chevron={false}
              onClick={() => {
                setReportOpen(true);
              }}
            />
            <ListRowButton
              icon={<Download size={icon.lg} />}
              title={t("Save this day's file")}
              chevron={false}
              onClick={() => {
                setMoreOpen(false);
                void actions.save(day);
              }}
            />
            {narrowed && (
              <ListRowButton
                icon={<Download size={icon.lg} />}
                title={t("Save shown lines", { count: totalVisible })}
                chevron={false}
                onClick={() => {
                  setMoreOpen(false);
                  saveVisible();
                }}
              />
            )}
          </ListGroup>
          <ListGroup>
            <ListRowButton
              icon={<Trash2 size={icon.lg} />}
              tone="danger"
              title={t("Delete this day")}
              chevron={false}
              onClick={() => {
                setConfirm("day");
              }}
            />
            <ListRowButton
              icon={<Trash2 size={icon.lg} />}
              tone="danger"
              title={t("Delete all days", { count: days.length })}
              chevron={false}
              onClick={() => {
                setConfirm("all");
              }}
            />
          </ListGroup>
        </div>
      </Sheet>

      <Sheet
        open={confirm !== null}
        onClose={() => setConfirm(null)}
        size="sm"
        title={shownConfirm === "all" ? t("Delete all logs?") : t("Delete {{day}}?", { day })}
      >
        <div className="space-y-4 pb-2">
          <Text variant="secondary">
            {shownConfirm === "all"
              ? t("deleteAllBody", { count: days.length })
              : t("This file is removed. This can't be undone.")}
          </Text>
          <div className="flex justify-end gap-2">
            <Button variant="ghost" onClick={() => setConfirm(null)}>
              {t("Cancel")}
            </Button>
            {actions.error !== null && (
              <FailureState error={actions.error} onRetry={actions.retry} />
            )}
            <Button
              variant="danger"
              loading={actions.busy}
              onClick={() => {
                void actions.run(async () => {
                  if (shownConfirm === "all") await api.call("logsClear");
                  else await api.call("logDelete", { name: day });
                  close();
                  await actions.refresh();
                  toast.success(t("Logs deleted"));
                });
              }}
            >
              {t("Delete")}
            </Button>
          </div>
        </div>
      </Sheet>

      <Diagnostics api={api} />
    </>
  );
}

function Diagnostics({ api }: { api: ApiClient }) {
  const { t } = useTranslation("logs");
  const search = useSearch({ from: "/settings/$section" });
  const navigate = useNavigate();
  const router = useRouter();
  const routeKey = useLocation({ select: (location) => location.state.__TSR_key });
  const pushed = useLocation({ select: (location) => location.state.logPanelPushed });
  const open = search.logPanel === "report";
  const report = useQuery({
    queryKey: [...queryKeys.logs.all, "diagnostics"],
    queryFn: () => api.call("logsDiagnosticsReport"),
    enabled: open,
    retry: false,
  });
  const [copyError, setCopyError] = useState<unknown>(null);
  const close = () => {
    if (router.state.location.state.__TSR_key !== routeKey) return;
    if (pushed) router.history.back();
    else
      void navigate({
        to: "/settings/$section",
        params: { section: "logs" },
        search: search.logFile ? { logFile: search.logFile } : {},
        replace: true,
        resetScroll: false,
      });
  };
  const copy = async () => {
    if (!report.data) return;
    setCopyError(null);
    try {
      await navigator.clipboard.writeText(report.data);
      close();
      toast.success(t("Diagnostics copied"));
    } catch (failure) {
      setCopyError(failure);
    }
  };
  return (
    <Sheet
      open={open}
      onClose={close}
      placement="auto"
      size="md"
      title={t("Diagnostics report")}
      description={t(
        "Paste this into a bug report. Home folder names are replaced with ~, and keys are never included.",
      )}
    >
      <div className="space-y-4 pb-2">
        {report.isPending ? (
          <Skeleton className="h-64 w-full" />
        ) : report.isError ? (
          <FailureState error={report.error} onRetry={() => void report.refetch()} />
        ) : (
          <CodeBlock code={report.data} label={t("Diagnostics")} wrap />
        )}
        {copyError !== null && <FailureState error={copyError} onRetry={() => void copy()} />}
        <div className="flex justify-end gap-2">
          <Button variant="ghost" loading={report.isFetching} onClick={() => void report.refetch()}>
            {t("Generate again")}
          </Button>
          <Button
            leading={<Copy size={icon.sm} />}
            disabled={!report.data}
            onClick={() => void copy()}
          >
            {t("Copy")}
          </Button>
        </div>
      </div>
    </Sheet>
  );
}
