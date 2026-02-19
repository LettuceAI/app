import { useState, useEffect, useCallback } from "react";
import { Shield, Lock, Database, Power, Search, ScrollText, Trash2 } from "lucide-react";
import { isAnalyticsAvailable, readSettings } from "../../../core/storage/repo";
import {
  setAnalyticsEnabled,
  setAutoDownloadCharacterCardAvatars,
  setPureModeLevel,
} from "../../../core/storage/appState";
import type { PureModeLevel } from "../../../core/storage/schemas";
import { invoke } from "@tauri-apps/api/core";
import { relaunch } from "@tauri-apps/plugin-process";
import { BottomMenu, MenuButton, MenuButtonGroup } from "../../components/BottomMenu";

interface FilterLogEntry {
  timestamp_ms: number;
  text_snippet: string;
  score: number;
  blocked: boolean;
  matched_terms: string[];
  level: string;
}

const PURE_MODE_OPTIONS: {
  value: PureModeLevel;
  label: string;
  description: string;
  color: string;
  activeColor: string;
  activeBg: string;
}[] = [
  {
    value: "off",
    label: "Off",
    description: "All content allowed",
    color: "text-fg/60",
    activeColor: "text-warning",
    activeBg: "border-warning/40 bg-warning/20",
  },
  {
    value: "low",
    label: "Low",
    description: "Blocks explicit sexual content + slurs",
    color: "text-fg/60",
    activeColor: "text-warning",
    activeBg: "border-warning/40 bg-warning/20",
  },
  {
    value: "standard",
    label: "Standard",
    description: "Blocks NSFW + graphic violence",
    color: "text-fg/60",
    activeColor: "text-accent",
    activeBg: "border-accent/40 bg-accent/20",
  },
  {
    value: "strict",
    label: "Strict",
    description: "Maximum filtering + no suggestive tone",
    color: "text-fg/60",
    activeColor: "text-info",
    activeBg: "border-info/40 bg-info/20",
  },
];
const FILTER_DEBUG_ENABLED = import.meta.env.DEV;

export function SecurityPage() {
  const [pureModeLevel, setPureModeLevelState] = useState<PureModeLevel>("standard");
  const [isGlitchEnabled, setIsGlitchEnabled] = useState(true);
  const [autoDownloadCharacterCardAvatars, setAutoDownloadCharacterCardAvatarsState] =
    useState(true);
  const [isAnalyticsEnabled, setIsAnalyticsEnabled] = useState(true);
  const [isAnalyticsAvailableState, setIsAnalyticsAvailableState] = useState(true);
  const [showRestartMenu, setShowRestartMenu] = useState(false);
  const [isLoading, setIsLoading] = useState(true);
  const [debugInput, setDebugInput] = useState("");
  const [debugResult, setDebugResult] = useState<Record<string, unknown> | null>(null);
  const [filterLog, setFilterLog] = useState<FilterLogEntry[]>([]);

  const handleDebugFilter = useCallback(async (text: string) => {
    if (!text.trim()) {
      setDebugResult(null);
      return;
    }
    try {
      const result = await invoke<Record<string, unknown>>("debug_content_filter", { text });
      setDebugResult(result);
    } catch (err) {
      console.error("debug_content_filter failed:", err);
    }
  }, []);

  const refreshFilterLog = useCallback(async () => {
    try {
      const log = await invoke<FilterLogEntry[]>("get_filter_log");
      setFilterLog(log);
    } catch (err) {
      console.error("get_filter_log failed:", err);
    }
  }, []);

  const clearFilterLog = useCallback(async () => {
    try {
      await invoke("clear_filter_log");
      setFilterLog([]);
    } catch (err) {
      console.error("clear_filter_log failed:", err);
    }
  }, []);

  // Load filter log on mount + poll every 5s
  useEffect(() => {
    if (!FILTER_DEBUG_ENABLED) {
      return;
    }
    void refreshFilterLog();
    const interval = setInterval(() => void refreshFilterLog(), 5000);
    return () => clearInterval(interval);
  }, [refreshFilterLog]);

  // Load settings on mount
  useEffect(() => {
    const load = async () => {
      try {
        const [settings, available] = await Promise.all([readSettings(), isAnalyticsAvailable()]);
        // Read pureModeLevel with fallback to pureModeEnabled boolean
        const level =
          settings.appState.pureModeLevel ??
          (settings.appState.pureModeEnabled ? "standard" : "off");
        const legacyState = settings.appState as unknown as Record<string, unknown>;
        const autoDownloadAvatars =
          settings.appState.autoDownloadCharacterCardAvatars ??
          (typeof legacyState.autoDownloadDiscoveryAvatars === "boolean"
            ? legacyState.autoDownloadDiscoveryAvatars
            : true);
        setPureModeLevelState(level);
        setAutoDownloadCharacterCardAvatarsState(autoDownloadAvatars);
        setIsAnalyticsEnabled(settings.appState.analyticsEnabled ?? true);
        setIsAnalyticsAvailableState(available);
        if (!available) {
          setIsAnalyticsEnabled(false);
        }
        try {
          const stored = localStorage.getItem("lettuce.easterEggs.glitch");
          if (stored !== null) {
            setIsGlitchEnabled(stored === "true");
          }
        } catch (err) {
          console.error("Failed to read glitch setting:", err);
          setIsGlitchEnabled(true);
        }
      } catch (err) {
        console.error("Failed to load settings:", err);
      } finally {
        setIsLoading(false);
      }
    };
    void load();
  }, []);

  const handleLevelChange = async (level: PureModeLevel) => {
    const prev = pureModeLevel;
    setPureModeLevelState(level);
    try {
      await setPureModeLevel(level);
    } catch (err) {
      console.error("Failed to save pure mode level:", err);
      setPureModeLevelState(prev);
    }
  };

  const handleGlitchToggle = () => {
    const newValue = !isGlitchEnabled;
    setIsGlitchEnabled(newValue);
    try {
      localStorage.setItem("lettuce.easterEggs.glitch", String(newValue));
      window.dispatchEvent(new CustomEvent("lettuce:easterEggs:glitch", { detail: newValue }));
    } catch (err) {
      console.error("Failed to save glitch setting:", err);
      setIsGlitchEnabled(!newValue);
    }
  };

  const handleAnalyticsToggle = async () => {
    if (!isAnalyticsAvailableState) {
      return;
    }
    const newValue = !isAnalyticsEnabled;
    setIsAnalyticsEnabled(newValue);
    try {
      await setAnalyticsEnabled(newValue);
      setShowRestartMenu(true);
    } catch (err) {
      console.error("Failed to save analytics setting:", err);
      setIsAnalyticsEnabled(!newValue);
    }
  };

  const handleAutoDownloadCharacterCardAvatarsToggle = async () => {
    const newValue = !autoDownloadCharacterCardAvatars;
    setAutoDownloadCharacterCardAvatarsState(newValue);
    try {
      await setAutoDownloadCharacterCardAvatars(newValue);
    } catch (err) {
      console.error("Failed to save character avatar auto-download setting:", err);
      setAutoDownloadCharacterCardAvatarsState(!newValue);
    }
  };

  if (isLoading) {
    return null;
  }

  const isEnabled = pureModeLevel !== "off";
  const activeOption = PURE_MODE_OPTIONS.find((o) => o.value === pureModeLevel)!;

  return (
    <div className="flex h-full flex-col pb-16">
      <section className="flex-1 overflow-y-auto px-3 pt-3 space-y-6">
        {/* Section: Content Filtering */}
        <div>
          <h2 className="mb-2 px-1 text-[10px] font-semibold uppercase tracking-[0.25em] text-fg/35">
            Content Filtering
          </h2>
          <div
            className={`relative overflow-hidden rounded-xl border px-4 py-3 transition-all duration-300 ${
              isEnabled
                ? "border-accent/20 bg-linear-to-br from-accent/10 via-fg/5 to-fg/5 shadow-[0_0_20px_rgba(16,185,129,0.15)]"
                : "border-fg/10 bg-fg/5"
            }`}
          >
            {isEnabled && (
              <div
                className="pointer-events-none absolute inset-0 opacity-60"
                style={{
                  background:
                    "radial-gradient(circle at 20% 20%, rgba(16,185,129,0.08) 0%, transparent 50%)",
                }}
              />
            )}

            <div className="relative flex items-start gap-3">
              <div
                className={`flex h-8 w-8 shrink-0 items-center justify-center rounded-full border transition-all duration-300 ${
                  isEnabled
                    ? "border-accent/40 bg-accent/15 shadow-lg shadow-accent/25"
                    : "border-fg/10 bg-fg/10"
                }`}
              >
                <Shield
                  className={`h-4 w-4 transition-colors duration-300 ${
                    isEnabled ? "text-accent" : "text-fg/70"
                  }`}
                />
              </div>
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2">
                  <span className="text-sm font-medium text-fg">Pure Mode</span>
                  <span
                    className={`rounded-md border px-1.5 py-0.5 text-[10px] font-medium leading-none uppercase tracking-[0.25em] transition-all duration-300 ${activeOption.activeBg} ${activeOption.activeColor}`}
                  >
                    {activeOption.label}
                  </span>
                </div>
                <div className="mt-0.5 text-[11px] text-fg/50">{activeOption.description}</div>

                {/* Level selector */}
                <div className="mt-3 flex gap-1.5">
                  {PURE_MODE_OPTIONS.map((option) => {
                    const isActive = pureModeLevel === option.value;
                    return (
                      <button
                        key={option.value}
                        onClick={() => handleLevelChange(option.value)}
                        className={`flex-1 rounded-lg border px-2 py-1.5 text-center text-[11px] font-medium transition-all duration-200 ${
                          isActive
                            ? `${option.activeBg} ${option.activeColor} shadow-sm`
                            : "border-fg/10 bg-fg/5 text-fg/50 hover:bg-fg/10"
                        }`}
                      >
                        {option.label}
                      </button>
                    );
                  })}
                </div>

                <div className="mt-2 text-[11px] text-fg/45 leading-relaxed">
                  Restrict adult content in AI responses
                </div>
              </div>
            </div>
          </div>
        </div>

        {/* Section: App Integrity */}
        <div>
          <h2 className="mb-2 px-1 text-[10px] font-semibold uppercase tracking-[0.25em] text-fg/35">
            App Integrity
          </h2>
          <div className="rounded-xl border border-fg/10 bg-fg/5 px-4 py-3">
            <div className="flex items-start gap-3">
              <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-fg/10 bg-fg/10">
                <Shield className="h-4 w-4 text-fg/70" />
              </div>
              <div className="min-w-0 flex-1">
                <div className="flex items-center justify-between gap-3">
                  <div className="flex-1">
                    <div className="flex items-center gap-2">
                      <span className="text-sm font-medium text-fg">Glitch Effects</span>
                      <span
                        className={`rounded-md border px-1.5 py-0.5 text-[10px] font-medium leading-none uppercase tracking-[0.25em] ${
                          isGlitchEnabled
                            ? "border-info/40 bg-info/15 text-info"
                            : "border-fg/10 bg-fg/10 text-fg/60"
                        }`}
                      >
                        {isGlitchEnabled ? "On" : "Off"}
                      </span>
                    </div>
                    <div className="mt-0.5 text-[11px] text-fg/50">
                      Disable the shake-triggered visuals
                    </div>
                  </div>
                  <div className="flex items-center">
                    <input
                      id="glitch-effects"
                      type="checkbox"
                      checked={isGlitchEnabled}
                      onChange={handleGlitchToggle}
                      className="peer sr-only"
                    />
                    <label
                      htmlFor="glitch-effects"
                      className={`relative inline-flex h-6 w-11 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-all duration-200 ease-in-out focus:outline-none focus:ring-2 focus:ring-info/40 ${
                        isGlitchEnabled ? "bg-info shadow-lg shadow-info/30" : "bg-fg/20"
                      }`}
                    >
                      <span
                        className={`inline-block h-5 w-5 transform rounded-full bg-fg shadow ring-0 transition duration-200 ease-in-out ${
                          isGlitchEnabled ? "translate-x-5" : "translate-x-0"
                        }`}
                      />
                    </label>
                  </div>
                </div>
                <div className="mt-2 text-[11px] text-fg/45 leading-relaxed">
                  Keeps the app stable on shake
                </div>
              </div>
            </div>
          </div>
        </div>

        {/* Section: Data Protection */}
        <div>
          <h2 className="mb-2 px-1 text-[10px] font-semibold uppercase tracking-[0.25em] text-fg/35">
            Data Protection
          </h2>
          <div className="space-y-2">
            <div className="rounded-xl border border-fg/10 bg-fg/5 px-4 py-3">
              <div className="flex items-start gap-3">
                <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-fg/10 bg-fg/10">
                  <Shield className="h-4 w-4 text-fg/70" />
                </div>
                <div className="min-w-0 flex-1">
                  <div className="flex items-center justify-between gap-3">
                    <div className="flex-1">
                      <div className="flex items-center gap-2">
                        <span className="text-sm font-medium text-fg">
                          Remote Avatar Download
                        </span>
                        <span
                          className={`rounded-md border px-1.5 py-0.5 text-[10px] font-medium leading-none uppercase tracking-[0.25em] ${
                            autoDownloadCharacterCardAvatars
                              ? "border-accent/50 bg-accent/25 text-accent"
                              : "border-fg/10 bg-fg/10 text-fg/60"
                          }`}
                        >
                          {autoDownloadCharacterCardAvatars ? "On" : "Off"}
                        </span>
                      </div>
                      <div className="mt-0.5 text-[11px] text-fg/50">
                        Auto-download avatar images from HTTPS URLs during character card import
                      </div>
                    </div>
                    <div className="flex items-center">
                      <input
                        id="remote-avatar-download"
                        type="checkbox"
                        checked={autoDownloadCharacterCardAvatars}
                        onChange={handleAutoDownloadCharacterCardAvatarsToggle}
                        className="peer sr-only"
                      />
                      <label
                        htmlFor="remote-avatar-download"
                        className={`relative inline-flex h-6 w-11 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-all duration-200 ease-in-out focus:outline-none focus:ring-2 focus:ring-accent/40 ${
                          autoDownloadCharacterCardAvatars
                            ? "bg-accent shadow-lg shadow-accent/30"
                            : "bg-fg/20"
                        }`}
                      >
                        <span
                          className={`inline-block h-5 w-5 transform rounded-full bg-fg shadow ring-0 transition duration-200 ease-in-out ${
                            autoDownloadCharacterCardAvatars ? "translate-x-5" : "translate-x-0"
                          }`}
                        />
                      </label>
                    </div>
                  </div>
                  <div className="mt-2 text-[11px] text-fg/45 leading-relaxed">
                    Disable this to prevent network avatar fetches when importing character cards
                  </div>
                </div>
              </div>
            </div>

            <div className="rounded-xl border border-fg/10 bg-fg/5 px-4 py-3">
              <div className="flex items-start gap-3">
                <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-fg/10 bg-fg/10">
                  <Database className="h-4 w-4 text-fg/70" />
                </div>
                <div className="min-w-0 flex-1">
                  <div className="flex items-center justify-between gap-3">
                    <div className="flex-1">
                      <div className="flex items-center gap-2">
                        <span className="text-sm font-medium text-fg">Analytics</span>
                        <span
                          className={`rounded-md border px-1.5 py-0.5 text-[10px] font-medium leading-none uppercase tracking-[0.25em] ${
                            isAnalyticsEnabled
                              ? "border-accent/50 bg-accent/25 text-accent"
                              : "border-fg/10 bg-fg/10 text-fg/60"
                          }`}
                        >
                          {!isAnalyticsAvailableState
                            ? "Unavailable"
                            : isAnalyticsEnabled
                              ? "On"
                              : "Off"}
                        </span>
                      </div>
                      <div className="mt-0.5 text-[11px] text-fg/50">
                        {isAnalyticsAvailableState
                          ? "Help improve the app with anonymous usage events"
                          : "Requires an analytics API key"}
                      </div>
                    </div>
                    <div className="flex items-center">
                      <input
                        id="analytics-tracking"
                        type="checkbox"
                        checked={isAnalyticsEnabled}
                        onChange={handleAnalyticsToggle}
                        disabled={!isAnalyticsAvailableState}
                        className="peer sr-only"
                      />
                      <label
                        htmlFor="analytics-tracking"
                        className={`relative inline-flex h-6 w-11 shrink-0 rounded-full border-2 border-transparent transition-all duration-200 ease-in-out focus:outline-none focus:ring-2 focus:ring-accent/40 ${
                          isAnalyticsAvailableState
                            ? "cursor-pointer"
                            : "cursor-not-allowed opacity-60"
                        } ${
                          isAnalyticsEnabled
                            ? "bg-accent shadow-lg shadow-accent/30"
                            : "bg-fg/20"
                        }`}
                      >
                        <span
                          className={`inline-block h-5 w-5 transform rounded-full bg-fg shadow ring-0 transition duration-200 ease-in-out ${
                            isAnalyticsEnabled ? "translate-x-5" : "translate-x-0"
                          }`}
                        />
                      </label>
                    </div>
                  </div>
                  <div className="mt-2 text-[11px] text-fg/45 leading-relaxed">
                    {isAnalyticsAvailableState
                      ? "Restart required to apply changes"
                      : "Set APTABASE_KEY to enable analytics"}
                  </div>
                </div>
              </div>
            </div>

            <div className="rounded-xl border border-fg/10 bg-fg/5 px-4 py-3">
              <div className="flex items-start gap-3">
                <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-fg/10 bg-fg/10">
                  <Database className="h-4 w-4 text-fg/70" />
                </div>
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="text-sm font-medium text-fg">Aptabase Analytics</span>
                    <span className="rounded-md border border-fg/10 bg-fg/10 px-1.5 py-0.5 text-[10px] font-medium leading-none text-fg/70">
                      Anonymous
                    </span>
                  </div>
                  <div className="mt-0.5 text-[11px] text-fg/45 leading-relaxed">
                    Events are anonymous and contain only the event name and not-identifying
                    properties we define. We do not send message content or personal identifiers.
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
        {FILTER_DEBUG_ENABLED && (
          <div>
            <div className="mb-2 flex items-center justify-between px-1">
              <h2 className="text-[10px] font-semibold uppercase tracking-[0.25em] text-fg/35">
                Filter Log
              </h2>
              {filterLog.length > 0 && (
                <button
                  onClick={() => void clearFilterLog()}
                  className="flex items-center gap-1 rounded-md px-1.5 py-0.5 text-[10px] text-fg/40 transition-colors hover:bg-fg/10 hover:text-fg/60"
                >
                  <Trash2 className="h-3 w-3" />
                  Clear
                </button>
              )}
            </div>
            <div className="rounded-xl border border-fg/10 bg-fg/5 px-4 py-3">
              {filterLog.length === 0 ? (
                <div className="flex items-center gap-3">
                  <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-fg/10 bg-fg/10">
                    <ScrollText className="h-4 w-4 text-fg/70" />
                  </div>
                  <div className="text-[11px] text-fg/40">
                    No filter hits recorded yet. Matches will appear here as you chat.
                  </div>
                </div>
              ) : (
                <div className="space-y-2 max-h-80 overflow-y-auto">
                  {[...filterLog].reverse().map((entry, i) => {
                    const time = new Date(entry.timestamp_ms);
                    const timeStr = time.toLocaleTimeString([], {
                      hour: "2-digit",
                      minute: "2-digit",
                      second: "2-digit",
                    });
                    return (
                      <div
                        key={`${entry.timestamp_ms}-${i}`}
                        className={`rounded-lg border px-3 py-2 ${
                          entry.blocked
                            ? "border-danger/30 bg-danger/10"
                            : "border-warning/20 bg-warning/5"
                        }`}
                      >
                        <div className="flex items-center justify-between gap-2 mb-1">
                          <div className="flex items-center gap-2">
                            <span
                              className={`rounded px-1 py-0.5 text-[9px] font-semibold uppercase tracking-wider ${
                                entry.blocked
                                  ? "bg-danger/20 text-danger/80"
                                  : "bg-warning/20 text-warning/80"
                              }`}
                            >
                              {entry.blocked ? "Blocked" : "Hit"}
                            </span>
                            <span className="text-[10px] text-fg/30">{entry.level}</span>
                            <span className="text-[10px] text-fg/30">
                              score:{" "}
                              <span className={entry.blocked ? "text-danger/80" : "text-warning"}>
                                {entry.score.toFixed(2)}
                              </span>
                            </span>
                          </div>
                          <span className="text-[10px] text-fg/25">{timeStr}</span>
                        </div>
                        <div className="text-[11px] text-fg/60 line-clamp-2 break-all font-mono">
                          {entry.text_snippet}
                        </div>
                        <div className="mt-1 flex flex-wrap gap-1">
                          {entry.matched_terms.map((term, j) => (
                            <span
                              key={j}
                              className={`rounded-md border px-1.5 py-0.5 text-[10px] ${
                                entry.blocked
                                  ? "border-danger/30 bg-danger/15 text-danger"
                                  : "border-warning/20 bg-warning/10 text-warning"
                              }`}
                            >
                              {term}
                            </span>
                          ))}
                        </div>
                      </div>
                    );
                  })}
                </div>
              )}
            </div>
          </div>
        )}

        {/* Section: Filter Debug (TEMP) */}
        {FILTER_DEBUG_ENABLED && (
          <div>
            <h2 className="mb-2 px-1 text-[10px] font-semibold uppercase tracking-[0.25em] text-fg/35">
              Filter Pipeline Debug
            </h2>
            <div className="rounded-xl border border-warning/20 bg-warning/5 px-4 py-3 space-y-3">
              <div className="flex items-center gap-2">
                <Search className="h-4 w-4 text-warning/70" />
                <span className="text-[10px] font-medium uppercase tracking-widest text-warning/60">
                  Temp — tokenization inspector
                </span>
              </div>
              <input
                type="text"
                value={debugInput}
                onChange={(e) => {
                  setDebugInput(e.target.value);
                  void handleDebugFilter(e.target.value);
                }}
                placeholder="Type a sentence to see how it gets processed..."
                className="w-full rounded-lg border border-fg/10 bg-surface-el/30 px-3 py-2 text-sm text-fg placeholder-fg/30 outline-none focus:border-warning/40"
              />
              {debugResult && (
                <div className="space-y-2 text-[11px] font-mono">
                  {(() => {
                    const pipeline = debugResult.pipeline as Record<string, unknown>;
                    const result = debugResult.result as Record<string, unknown>;
                    const steps: [string, string][] = [
                      ["stripped", String(pipeline.stripped)],
                      ["lowercase", String(pipeline.lowercased)],
                      ["unicode norm", String(pipeline.unicode_normalized)],
                      ["leet norm", String(pipeline.leet_normalized)],
                      ["tokens", (pipeline.tokens as string[]).join(" | ")],
                      ["collapsed", String(pipeline.collapsed)],
                      ["collapsed tokens", (pipeline.collapsed_tokens as string[]).join(" | ")],
                    ];
                    // Hide steps that are identical to previous
                    const visible = steps.filter((s, i) => i === 0 || s[1] !== steps[i - 1][1]);
                    return (
                      <>
                        {visible.map(([label, value]) => (
                          <div key={label} className="flex gap-2">
                            <span className="shrink-0 w-28 text-right text-fg/30">{label}</span>
                            <span className="text-fg/80 break-all">{value}</span>
                          </div>
                        ))}
                        <div className="mt-1 border-t border-fg/10 pt-2 flex flex-wrap gap-x-4 gap-y-1">
                          <span className="text-fg/40">
                            level:{" "}
                            <span className="text-fg/70">{String(debugResult.level)}</span>
                          </span>
                          <span className="text-fg/40">
                            context:{" "}
                            <span className="text-fg/70">
                              {debugResult.context_allowlist_hit ? "yes" : "no"}
                            </span>
                          </span>
                          <span className="text-fg/40">
                            score:{" "}
                            <span
                              className={
                                (result.score as number) > 0 ? "text-danger/80" : "text-accent/80"
                              }
                            >
                              {(result.score as number).toFixed(2)}
                            </span>
                          </span>
                          <span className="text-fg/40">
                            blocked:{" "}
                            <span className={result.blocked ? "text-danger/80" : "text-accent/80"}>
                              {result.blocked ? "yes" : "no"}
                            </span>
                          </span>
                        </div>
                        {(result.matched_terms as string[]).length > 0 && (
                          <div className="flex flex-wrap gap-1.5 mt-1">
                            {(result.matched_terms as string[]).map((term, i) => (
                              <span
                                key={i}
                                className="rounded-md border border-danger/30 bg-danger/15 px-1.5 py-0.5 text-[10px] text-danger"
                              >
                                {term}
                              </span>
                            ))}
                          </div>
                        )}
                      </>
                    );
                  })()}
                </div>
              )}
            </div>
          </div>
        )}
      </section>
      <BottomMenu
        isOpen={showRestartMenu}
        onClose={() => setShowRestartMenu(false)}
        title="Restart required"
      >
        <div className="text-sm text-fg/70">Analytics changes apply after a restart.</div>
        <MenuButtonGroup>
          <MenuButton
            icon={Power}
            title="Restart now"
            description="Apply analytics changes"
            color="from-accent to-accent/80"
            onClick={async () => {
              setShowRestartMenu(false);
              await relaunch();
            }}
          />
          <MenuButton
            icon={Lock}
            title="Later"
            description="Keep current session"
            color="from-info to-info/80"
            onClick={() => setShowRestartMenu(false)}
          />
        </MenuButtonGroup>
      </BottomMenu>
    </div>
  );
}
