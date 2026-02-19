import { invoke } from "@tauri-apps/api/core";
import { getName, getVersion } from "@tauri-apps/api/app";
import { useEffect, useState } from "react";
import {
  FileText,
  RefreshCw,
  Trash2,
  Download,
  FolderOpen,
  Loader2,
  FileCode,
  Clipboard,
  Sparkles,
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

const logger = logManager({ component: "LogsPage" });

export function LogsPage() {
  const [logFiles, setLogFiles] = useState<string[]>([]);
  const [selectedFile, setSelectedFile] = useState<string | null>(null);
  const [logContent, setLogContent] = useState<string>("");
  const [loading, setLoading] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [logDir, setLogDir] = useState<string>("");
  const [diagnosticsText, setDiagnosticsText] = useState("");
  const [generatingDiagnostics, setGeneratingDiagnostics] = useState(false);
  const [diagnosticsExpanded, setDiagnosticsExpanded] = useState(false);

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
      const [settings, embeddingInfo] = await Promise.all([
        readSettings(),
        getEmbeddingModelInfo(),
      ]);
      const platform = getPlatform();
      const [appName, appVersion] = await Promise.all([getName(), getVersion()]);

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

      const dynamicSettings = settings.advancedSettings?.dynamicMemory;
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
      lines.push("");
      lines.push("App State");
      lines.push(`- Pure Mode: ${settings.appState.pureModeLevel ?? (settings.appState.pureModeEnabled ? "standard" : "off")}`);
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
      lines.push("");
      lines.push("Models");
      if (settings.models.length > 0) {
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
      logger.info("Loaded log files", { count: files.length });
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

  const loadLogContent = async (filename: string) => {
    setLoading(true);
    try {
      const content = await invoke<string>("read_log_file", { filename });
      setLogContent(content);
      setSelectedFile(filename);
      logger.info("Loaded log file", { filename });
    } catch (err) {
      logger.error("Failed to load log file", err);
      setLogContent("Failed to load log file");
    } finally {
      setLoading(false);
    }
  };

  const downloadLogFile = async () => {
    if (!selectedFile) return;

    try {
      const savedPath = await invoke<string>("save_log_to_downloads", { filename: selectedFile });
      logger.info("Downloaded log file", { filename: selectedFile, path: savedPath });
      alert(`Log file saved to:\n${savedPath}`);
    } catch (err) {
      logger.error("Failed to download log file", err);
      alert(`Failed to save log file: ${err}`);
    }
  };

  const deleteLogFile = async (filename: string) => {
    try {
      await invoke("delete_log_file", { filename });
      await loadLogFiles();
      if (selectedFile === filename) {
        setSelectedFile(null);
        setLogContent("");
      }
      logger.info("Deleted log file", { filename });
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
      setLogContent("");
      logger.info("Cleared all logs");
    } catch (err) {
      logger.error("Failed to clear all logs", err);
    }
  };

  useEffect(() => {
    loadLogFiles();
    loadLogDir();
  }, []);

  return (
    <div className="flex h-full flex-col pb-16">
      <section className="flex-1 overflow-y-auto px-3 pt-3 space-y-4">
        {/* Diagnostics */}
        <div>
          <div className="flex items-center justify-between mb-2 px-1">
            <h2
              className={cn(
                typography.overline.size,
                typography.overline.weight,
                typography.overline.tracking,
                typography.overline.transform,
                "text-fg/35",
              )}
            >
              Diagnostics
            </h2>
            <div className="flex items-center gap-2">
              <button
                onClick={buildDiagnostics}
                disabled={generatingDiagnostics}
                className={cn(
                  typography.caption.size,
                  typography.caption.weight,
                  "text-secondary hover:text-secondary/80",
                  interactive.transition.default,
                  "disabled:opacity-50 flex items-center gap-1",
                )}
              >
                <Sparkles className={cn("h-3.5 w-3.5", generatingDiagnostics && "animate-spin")} />
                Generate
              </button>
              <button
                onClick={copyDiagnostics}
                disabled={generatingDiagnostics}
                className={cn(
                  typography.caption.size,
                  typography.caption.weight,
                  "text-info hover:text-info/80",
                  interactive.transition.default,
                  "disabled:opacity-50 flex items-center gap-1",
                )}
              >
                <Clipboard className="h-3.5 w-3.5" />
                Copy
              </button>
            </div>
          </div>

          <div className="rounded-xl border border-fg/10 bg-fg/5 overflow-hidden">
            {generatingDiagnostics ? (
              <div className="flex items-center justify-center py-12">
                <Loader2 className="h-6 w-6 animate-spin text-fg/30" />
              </div>
            ) : (
              <div className="relative">
                <div
                  className={cn(
                    "overflow-y-auto",
                    diagnosticsExpanded ? "max-h-130" : "max-h-56",
                  )}
                >
                  <pre
                    className={cn(
                      typography.caption.size,
                      "font-mono p-4 text-fg/70 whitespace-pre-wrap wrap-break-word",
                    )}
                  >
                    {diagnosticsText ||
                      "Generate diagnostics to see device/app summary and embedding test results."}
                  </pre>
                </div>
                {!diagnosticsExpanded && diagnosticsText && (
                  <div className="pointer-events-none absolute inset-x-0 bottom-0 h-12 bg-gradient-to-t from-surface-el to-transparent" />
                )}
              </div>
            )}
          </div>
          {diagnosticsText && !generatingDiagnostics && (
            <button
              onClick={() => setDiagnosticsExpanded((prev) => !prev)}
              className={cn(
                typography.caption.size,
                typography.caption.weight,
                "mt-2 text-fg/50 hover:text-fg/80",
                interactive.transition.default,
              )}
            >
              {diagnosticsExpanded ? "Show less" : "Show full diagnostics"}
            </button>
          )}
        </div>

        {/* Log Directory Info */}
        {logDir && (
          <div className="rounded-xl border border-fg/10 bg-fg/5 p-3">
            <div className="flex items-center gap-2">
              <FolderOpen className="h-4 w-4 text-fg/40" />
              <p className={cn(typography.caption.size, "text-fg/50 font-mono break-all")}>
                {logDir}
              </p>
            </div>
          </div>
        )}

        {/* Log Files List */}
        <div>
          <div className="flex items-center justify-between mb-2 px-1">
            <h2
              className={cn(
                typography.overline.size,
                typography.overline.weight,
                typography.overline.tracking,
                typography.overline.transform,
                "text-fg/35",
              )}
            >
              Log Files {logFiles.length > 0 && `(${logFiles.length})`}
            </h2>
            <button
              onClick={loadLogFiles}
              disabled={refreshing}
              className={cn(
                typography.caption.size,
                typography.caption.weight,
                "text-info hover:text-info/80",
                interactive.transition.default,
                "disabled:opacity-50",
              )}
            >
              <RefreshCw className={cn("h-3.5 w-3.5", refreshing && "animate-spin")} />
            </button>
          </div>

          {logFiles.length === 0 ? (
            <div className="rounded-xl border border-fg/10 bg-fg/5 p-8 text-center">
              <FileCode className="mx-auto h-10 w-10 text-fg/20" />
              <p className={cn("mt-3", typography.body.size, "text-fg/40")}>
                No log files found
              </p>
              <p className={cn("mt-1", typography.caption.size, "text-fg/30")}>
                Logs will appear here as you use the app
              </p>
            </div>
          ) : (
            <div className="space-y-2">
              {logFiles.map((file) => (
                <button
                  key={file}
                  onClick={() => loadLogContent(file)}
                  className={cn(
                    "w-full rounded-xl border p-3 text-left",
                    interactive.transition.default,
                    selectedFile === file
                      ? "border-info/30 bg-info/10"
                      : "border-fg/10 bg-fg/5 hover:border-fg/20 hover:bg-fg/8",
                    interactive.active.scale,
                  )}
                >
                  <div className="flex items-center justify-between gap-3">
                    <div className="flex items-center gap-3 flex-1 min-w-0">
                      <div
                        className={cn(
                          "flex h-8 w-8 items-center justify-center rounded-lg border",
                          selectedFile === file
                            ? "border-info/30 bg-info/15"
                            : "border-fg/10 bg-fg/10",
                        )}
                      >
                        <FileText className="h-4 w-4 text-fg/60" />
                      </div>
                      <div className="min-w-0 flex-1">
                        <p
                          className={cn(
                            typography.body.size,
                            typography.body.weight,
                            "text-fg truncate",
                          )}
                        >
                          {file}
                        </p>
                      </div>
                    </div>
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        deleteLogFile(file);
                      }}
                      className={cn(
                        "p-1.5 rounded-lg",
                        "text-danger/60 hover:text-danger hover:bg-danger/10",
                        interactive.transition.default,
                      )}
                    >
                      <Trash2 className="h-4 w-4" />
                    </button>
                  </div>
                </button>
              ))}
            </div>
          )}
        </div>

        {/* Log Content Viewer */}
        {selectedFile && (
          <div>
            <div className="flex items-center justify-between mb-2 px-1">
              <h2
                className={cn(
                  typography.overline.size,
                  typography.overline.weight,
                  typography.overline.tracking,
                  typography.overline.transform,
                  "text-fg/35",
                )}
              >
                {selectedFile}
              </h2>
              <button
                onClick={downloadLogFile}
                disabled={!logContent || loading}
                className={cn(
                  typography.caption.size,
                  typography.caption.weight,
                  "text-info hover:text-info/80",
                  interactive.transition.default,
                  "disabled:opacity-50 flex items-center gap-1",
                )}
              >
                <Download className="h-3.5 w-3.5" />
                Download
              </button>
            </div>

            <div className="rounded-xl border border-fg/10 bg-fg/5 overflow-hidden">
              {loading ? (
                <div className="flex items-center justify-center py-12">
                  <Loader2 className="h-6 w-6 animate-spin text-fg/30" />
                </div>
              ) : (
                <div className="max-h-100 overflow-y-auto">
                  <pre
                    className={cn(
                      typography.caption.size,
                      "font-mono p-4 text-fg/70 whitespace-pre-wrap wrap-break-word",
                    )}
                  >
                    {logContent || "Log file is empty"}
                  </pre>
                </div>
              )}
            </div>
          </div>
        )}

        {/* Clear All Logs */}
        {logFiles.length > 0 && (
          <div>
            <h2
              className={cn(
                "mb-2 px-1",
                typography.overline.size,
                typography.overline.weight,
                typography.overline.tracking,
                typography.overline.transform,
                "text-fg/35",
              )}
            >
              Danger Zone
            </h2>
            <button
              onClick={clearAllLogs}
              className={cn(
                "w-full rounded-xl border border-danger/30 bg-danger/10 p-4 text-left",
                interactive.transition.default,
                "hover:border-danger/50 hover:bg-danger/15",
                interactive.active.scale,
              )}
            >
              <div className="flex items-center gap-3">
                <div className="flex h-10 w-10 items-center justify-center rounded-full border border-danger/30 bg-danger/15">
                  <Trash2 className="h-5 w-5 text-danger/80" />
                </div>
                <div className="flex-1">
                  <p className={cn(typography.body.size, typography.body.weight, "text-fg")}>
                    Clear All Logs
                  </p>
                  <p className={cn(typography.caption.size, "text-fg/50")}>
                    Delete all log files permanently
                  </p>
                </div>
              </div>
            </button>
          </div>
        )}

        {/* Info */}
        <div className="rounded-xl border border-fg/10 bg-fg/5 p-4">
          <h3 className={cn(typography.body.size, typography.body.weight, "text-fg mb-2")}>
            About Application Logs
          </h3>
          <div className={cn(typography.caption.size, "text-fg/50 space-y-1.5")}>
            <p>• Log files are organized by date (app-YYYY-MM-DD.log)</p>
            <p>• Each entry includes timestamp, component, log level, and message</p>
            <p>• Useful for debugging issues and understanding app behavior</p>
            <p>• Sensitive information like API keys are never logged</p>
            <p>• Old log files can be safely deleted to free up space</p>
          </div>
        </div>
      </section>
    </div>
  );
}
