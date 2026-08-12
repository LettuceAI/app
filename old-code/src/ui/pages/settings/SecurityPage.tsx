import { useState, useEffect, useCallback } from "react";
import { Shield, Lock, Database, Power, ScrollText, Trash2, FilePlus2, FileBadge2 } from "lucide-react";
import { isAnalyticsAvailable, readSettings, setAppState } from "../../../core/storage/repo";
import {
  setAnalyticsEnabled,
  setAutoDownloadCharacterCardAvatars,
  setPureModeLevel,
} from "../../../core/storage/appState";
import type { PureModeLevel, TrustedCertificate } from "../../../core/storage/schemas";
import { invoke } from "@tauri-apps/api/core";
import { relaunch } from "@tauri-apps/plugin-process";
import { open } from "@tauri-apps/plugin-dialog";
import { readTextFile } from "@tauri-apps/plugin-fs";
import { BottomMenu, MenuButton, MenuButtonGroup } from "../../components/BottomMenu";
import { useI18n, type TranslationKey } from "../../../core/i18n/context";
import { Switch } from "../../components/Switch";

interface FilterLogEntry {
  timestamp_ms: number;
  text_snippet: string;
  score: number;
  blocked: boolean;
  matched_terms: string[];
  level: string;
}

const PURE_MODE_OPTIONS = [
  {
    value: "off",
    labelKey: "security.pureMode.off",
    descriptionKey: "security.pureMode.offDesc",
    color: "text-fg/60",
    activeColor: "text-warning",
    activeBg: "border-warning/40 bg-warning/20",
  },
  {
    value: "low",
    labelKey: "security.pureMode.low",
    descriptionKey: "security.pureMode.lowDesc",
    color: "text-fg/60",
    activeColor: "text-warning",
    activeBg: "border-warning/40 bg-warning/20",
  },
  {
    value: "standard",
    labelKey: "security.pureMode.standard",
    descriptionKey: "security.pureMode.standardDesc",
    color: "text-fg/60",
    activeColor: "text-accent",
    activeBg: "border-accent/40 bg-accent/20",
  },
  {
    value: "strict",
    labelKey: "security.pureMode.strict",
    descriptionKey: "security.pureMode.strictDesc",
    color: "text-fg/60",
    activeColor: "text-info",
    activeBg: "border-info/40 bg-info/20",
  },
] satisfies {
  value: PureModeLevel;
  labelKey: TranslationKey;
  descriptionKey: TranslationKey;
  color: string;
  activeColor: string;
  activeBg: string;
}[];
const FILTER_DEBUG_ENABLED = import.meta.env.DEV;

export function SecurityPage() {
  const { t } = useI18n();
  const [pureModeLevel, setPureModeLevelState] = useState<PureModeLevel>("standard");
  const [autoDownloadCharacterCardAvatars, setAutoDownloadCharacterCardAvatarsState] =
    useState(true);
  const [isAnalyticsEnabled, setIsAnalyticsEnabled] = useState(true);
  const [isAnalyticsAvailableState, setIsAnalyticsAvailableState] = useState(true);
  const [showRestartMenu, setShowRestartMenu] = useState(false);
  const [isLoading, setIsLoading] = useState(true);
  const [filterLog, setFilterLog] = useState<FilterLogEntry[]>([]);
  const [trustedCertificates, setTrustedCertificates] = useState<TrustedCertificate[]>([]);
  const [certificateError, setCertificateError] = useState<string | null>(null);
  const [isImportingCertificate, setIsImportingCertificate] = useState(false);

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
        setTrustedCertificates(settings.appState.trustedCertificates ?? []);
        if (!available) {
          setIsAnalyticsEnabled(false);
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

  const persistTrustedCertificates = useCallback(async (next: TrustedCertificate[]) => {
    const settings = await readSettings();
    await setAppState({
      ...settings.appState,
      trustedCertificates: next,
    });
    setTrustedCertificates(next);
  }, []);

  const handleImportCertificate = useCallback(async () => {
    setCertificateError(null);
    setIsImportingCertificate(true);
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: "Certificates", extensions: ["pem", "crt", "cer"] }],
      });
      if (!selected || typeof selected !== "string") {
        return;
      }

      const pem = (await readTextFile(selected)).trim();
      if (!pem.includes("BEGIN CERTIFICATE") || !pem.includes("END CERTIFICATE")) {
        setCertificateError(t("security.certificates.errorNotPem"));
        return;
      }

      if (trustedCertificates.some((certificate) => certificate.pem.trim() === pem)) {
        setCertificateError(t("security.certificates.errorAlreadyImported"));
        return;
      }

      const filename = selected.split("/").pop() || "certificate.pem";
      const nextEntry: TrustedCertificate = {
        id: crypto.randomUUID(),
        name: filename,
        pem,
        importedAt: Date.now(),
      };
      await persistTrustedCertificates([...trustedCertificates, nextEntry]);
    } catch (error) {
      console.error("Failed to import certificate:", error);
      setCertificateError(String(error));
    } finally {
      setIsImportingCertificate(false);
    }
  }, [persistTrustedCertificates, trustedCertificates, t]);

  const handleDeleteCertificate = useCallback(
    async (id: string) => {
      setCertificateError(null);
      try {
        await persistTrustedCertificates(
          trustedCertificates.filter((certificate) => certificate.id !== id),
        );
      } catch (error) {
        console.error("Failed to delete certificate:", error);
        setCertificateError(String(error));
      }
    },
    [persistTrustedCertificates, trustedCertificates],
  );

  if (isLoading) {
    return null;
  }

  const isEnabled = pureModeLevel !== "off";
  const activeOption = PURE_MODE_OPTIONS.find((o) => o.value === pureModeLevel)!;

  return (
    <div className="flex h-full flex-col">
      <section className="flex-1 overflow-y-auto px-3 pt-3 pb-6 space-y-6">
        {/* Section: Content Filtering */}
        <div>
          <h2 className="mb-2 px-1 text-[10px] font-semibold uppercase tracking-[0.25em] text-fg/35">
            {t("security.contentFiltering.sectionTitle")}
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
                  <span className="text-sm font-medium text-fg">
                    {t("security.contentFiltering.pureModeTitle")}
                  </span>
                  <span
                    className={`rounded-md border px-1.5 py-0.5 text-[10px] font-medium leading-none uppercase tracking-[0.25em] transition-all duration-300 ${activeOption.activeBg} ${activeOption.activeColor}`}
                  >
                    {t(activeOption.labelKey)}
                  </span>
                </div>
                <div className="mt-0.5 text-[11px] text-fg/50">{t(activeOption.descriptionKey)}</div>

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
                        {t(option.labelKey)}
                      </button>
                    );
                  })}
                </div>

                <div className="mt-2 text-[11px] text-fg/45 leading-relaxed">
                  {t("security.contentFiltering.pureModeFooter")}
                </div>
              </div>
            </div>
          </div>
        </div>

        {/* Section: Data Protection */}
        <div>
          <h2 className="mb-2 px-1 text-[10px] font-semibold uppercase tracking-[0.25em] text-fg/35">
            {t("security.dataProtection.sectionTitle")}
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
                          {t("security.dataProtection.remoteAvatarTitle")}
                        </span>
                        <span
                          className={`rounded-md border px-1.5 py-0.5 text-[10px] font-medium leading-none uppercase tracking-[0.25em] ${
                            autoDownloadCharacterCardAvatars
                              ? "border-accent/50 bg-accent/25 text-accent"
                              : "border-fg/10 bg-fg/10 text-fg/60"
                          }`}
                        >
                          {autoDownloadCharacterCardAvatars ? t("common.labels.on") : t("common.labels.off")}
                        </span>
                      </div>
                      <div className="mt-0.5 text-[11px] text-fg/50">
                        {t("security.dataProtection.remoteAvatarDesc")}
                      </div>
                    </div>
                    <Switch
                      id="remote-avatar-download"
                      checked={autoDownloadCharacterCardAvatars}
                      onChange={() => void handleAutoDownloadCharacterCardAvatarsToggle()}
                    />
                  </div>
                  <div className="mt-2 text-[11px] text-fg/45 leading-relaxed">
                    {t("security.dataProtection.remoteAvatarFooter")}
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
                        <span className="text-sm font-medium text-fg">
                          {t("security.dataProtection.analyticsTitle")}
                        </span>
                        <span
                          className={`rounded-md border px-1.5 py-0.5 text-[10px] font-medium leading-none uppercase tracking-[0.25em] ${
                            isAnalyticsEnabled
                              ? "border-accent/50 bg-accent/25 text-accent"
                              : "border-fg/10 bg-fg/10 text-fg/60"
                          }`}
                        >
                          {!isAnalyticsAvailableState
                            ? t("security.dataProtection.unavailable")
                            : isAnalyticsEnabled
                              ? t("common.labels.on")
                              : t("common.labels.off")}
                        </span>
                      </div>
                      <div className="mt-0.5 text-[11px] text-fg/50">
                        {isAnalyticsAvailableState
                          ? t("security.dataProtection.analyticsDescAvailable")
                          : t("security.dataProtection.analyticsDescUnavailable")}
                      </div>
                    </div>
                    <Switch
                      id="analytics-tracking"
                      checked={isAnalyticsEnabled}
                      disabled={!isAnalyticsAvailableState}
                      onChange={() => void handleAnalyticsToggle()}
                    />
                  </div>
                  <div className="mt-2 text-[11px] text-fg/45 leading-relaxed">
                    {isAnalyticsAvailableState
                      ? t("security.dataProtection.analyticsFooterAvailable")
                      : t("security.dataProtection.analyticsFooterUnavailable")}
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
                    <span className="text-sm font-medium text-fg">
                      {t("security.dataProtection.aptabaseTitle")}
                    </span>
                    <span className="rounded-md border border-fg/10 bg-fg/10 px-1.5 py-0.5 text-[10px] font-medium leading-none text-fg/70">
                      {t("security.dataProtection.aptabaseBadge")}
                    </span>
                  </div>
                  <div className="mt-0.5 text-[11px] text-fg/45 leading-relaxed">
                    {t("security.dataProtection.aptabaseDesc")}
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>

        <div>
          <h2 className="mb-2 px-1 text-[10px] font-semibold uppercase tracking-[0.25em] text-fg/35">
            {t("security.certificates.sectionTitle")}
          </h2>
          <div className="space-y-2">
            <div className="rounded-xl border border-fg/10 bg-fg/5 px-4 py-3">
              <div className="flex items-start gap-3">
                <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-fg/10 bg-fg/10">
                  <Lock className="h-4 w-4 text-fg/70" />
                </div>
                <div className="min-w-0 flex-1">
                  <div className="flex items-start justify-between gap-3">
                    <div className="flex-1">
                      <div className="flex items-center gap-2">
                        <span className="text-sm font-medium text-fg">
                          {t("security.certificates.customRootCasTitle")}
                        </span>
                        <span className="rounded-md border border-fg/10 bg-fg/10 px-1.5 py-0.5 text-[10px] font-medium leading-none text-fg/70">
                          {trustedCertificates.length}
                        </span>
                      </div>
                      <div className="mt-0.5 text-[11px] text-fg/50">
                        {t("security.certificates.customRootCasDesc")}
                      </div>
                    </div>
                    <button
                      onClick={() => void handleImportCertificate()}
                      disabled={isImportingCertificate}
                      className="inline-flex items-center gap-2 rounded-lg border border-accent/35 bg-accent/15 px-3 py-2 text-[11px] font-medium text-accent transition hover:bg-accent/20 disabled:cursor-not-allowed disabled:opacity-60"
                    >
                      <FilePlus2 className="h-3.5 w-3.5" />
                      {isImportingCertificate ? t("common.buttons.importing") : t("common.buttons.import")}
                    </button>
                  </div>
                  <div className="mt-2 text-[11px] text-fg/45 leading-relaxed">
                    {t("security.certificates.footer")}
                  </div>
                  {certificateError && (
                    <div className="mt-3 rounded-lg border border-danger/30 bg-danger/10 px-3 py-2 text-[11px] text-danger">
                      {certificateError}
                    </div>
                  )}
                </div>
              </div>
            </div>

            {trustedCertificates.length === 0 ? (
              <div className="rounded-xl border border-fg/10 bg-fg/5 px-4 py-3 text-[11px] text-fg/45">
                {t("security.certificates.empty")}
              </div>
            ) : (
              trustedCertificates.map((certificate) => (
                <div
                  key={certificate.id}
                  className="rounded-xl border border-fg/10 bg-fg/5 px-4 py-3"
                >
                  <div className="flex items-start gap-3">
                    <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-fg/10 bg-fg/10">
                      <FileBadge2 className="h-4 w-4 text-fg/70" />
                    </div>
                    <div className="min-w-0 flex-1">
                      <div className="flex items-center justify-between gap-3">
                        <div className="min-w-0 flex-1">
                          <div className="truncate text-sm font-medium text-fg">
                            {certificate.name}
                          </div>
                          <div className="mt-0.5 text-[11px] text-fg/45">
                            {t("security.certificates.importedAt", {
                              date: new Date(certificate.importedAt).toLocaleString(),
                            })}
                          </div>
                        </div>
                        <button
                          onClick={() => void handleDeleteCertificate(certificate.id)}
                          className="rounded-lg border border-danger/25 bg-danger/10 px-2.5 py-2 text-[11px] font-medium text-danger transition hover:bg-danger/15"
                        >
                          {t("common.buttons.remove")}
                        </button>
                      </div>
                    </div>
                  </div>
                </div>
              ))
            )}
          </div>
        </div>
        {FILTER_DEBUG_ENABLED && (
          <div>
            <div className="mb-2 flex items-center justify-between px-1">
              <h2 className="text-[10px] font-semibold uppercase tracking-[0.25em] text-fg/35">
                {t("security.filterLog.sectionTitle")}
              </h2>
              {filterLog.length > 0 && (
                <button
                  onClick={() => void clearFilterLog()}
                  className="flex items-center gap-1 rounded-md px-1.5 py-0.5 text-[10px] text-fg/40 transition-colors hover:bg-fg/10 hover:text-fg/60"
                >
                  <Trash2 className="h-3 w-3" />
                  {t("security.filterLog.clear")}
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
                    {t("security.filterLog.empty")}
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
                              {entry.blocked ? t("security.filterLog.blocked") : t("security.filterLog.hit")}
                            </span>
                            <span className="text-[10px] text-fg/30">{entry.level}</span>
                            <span className="text-[10px] text-fg/30">
                              {t("security.filterLog.score")}{" "}
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

      </section>
      <BottomMenu
        isOpen={showRestartMenu}
        onClose={() => setShowRestartMenu(false)}
        title={t("security.restart.title")}
      >
        <div className="text-sm text-fg/70">{t("security.restart.body")}</div>
        <MenuButtonGroup>
          <MenuButton
            icon={Power}
            title={t("security.restart.restartNow")}
            description={t("security.restart.restartNowDesc")}
            color="from-accent to-accent/80"
            onClick={async () => {
              setShowRestartMenu(false);
              await relaunch();
            }}
          />
          <MenuButton
            icon={Lock}
            title={t("security.restart.later")}
            description={t("security.restart.laterDesc")}
            color="from-info to-info/80"
            onClick={() => setShowRestartMenu(false)}
          />
        </MenuButtonGroup>
      </BottomMenu>
    </div>
  );
}
