import { useEffect, useState } from "react";
import { motion } from "framer-motion";
import { Download, X, Zap } from "lucide-react";
import { storageBridge } from "../../../core/storage/files";
import { updateAdvancedSetting } from "../../../core/storage/advanced";
import { useNavigate, useSearchParams } from "react-router-dom";
import { listen } from "@tauri-apps/api/event";
import { useI18n } from "../../../core/i18n/context";
import {
  ModelDownloadProgress,
  type ModelDownloadPhase,
} from "./components/ModelDownloadProgress";
import { useModelDownload } from "./hooks/useModelDownload";

type Capacity = 1024 | 2048 | 4096;

const CAPACITY_OPTIONS: { value: Capacity; label: string; description: string }[] = [
  { value: 1024, label: "1K tokens", description: "Best for quick responses" },
  { value: 2048, label: "2K tokens", description: "Balanced performance" },
  { value: 4096, label: "4K tokens", description: "Maximum context" },
];

export function EmbeddingDownloadPage() {
  const { t } = useI18n();
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const returnTo = searchParams.get("returnTo");
  const isUpgrade = searchParams.get("upgrade") === "true";
  // v3 is no longer offered as a download option; the page always installs v4.
  // The `?version=` query param is ignored aside from logging legacy callers.
  const downloadVersion = "v4" as const;
  const downloadVersionLabel = "V4";
  const downloadApproxSize = "~138 MB";

  const [showCapacitySelection, setShowCapacitySelection] = useState(true);
  const [selectedCapacity, setSelectedCapacity] = useState<Capacity>(2048);
  const [preStepStatus, setPreStepStatus] = useState<"idle" | "preparing" | "ready">("idle");
  const [testStatus, setTestStatus] = useState<"idle" | "testing" | "passed" | "failed">("idle");
  const [testResults, setTestResults] = useState<{
    success: boolean;
    message: string;
    health: { identityCosine: number; passed: boolean };
    retrieval: { top1Rate: number; top3Rate: number; mrr: number; passed: boolean };
    separation: { relatedAvg: number; unrelatedAvg: number; margin: number; passed: boolean };
  } | null>(null);
  const [countdown, setCountdown] = useState<number>(5);
  const [testProgress, setTestProgress] = useState<{ current: number; total: number } | null>(null);

  const runTest = async () => {
    setTestStatus("testing");
    download.setError(null);
    setTestProgress({ current: 0, total: 0 });

    try {
      const results = await storageBridge.runEmbeddingTest();
      setTestResults(results);

      if (results.success) {
        setTestStatus("passed");
        await updateAdvancedSetting("dynamicMemory", {
          enabled: true,
          summaryMessageInterval: 20,
          maxEntries: 50,
          minSimilarityThreshold: 0.32,
          retrievalLimit: 5,
          retrievalStrategy: "smart",
          hotMemoryTokenBudget: 2000,
          decayRate: 0.08,
          coldThreshold: 0.3,
        });
      } else {
        setTestStatus("failed");
      }
    } catch (err) {
      setTestStatus("failed");
      download.setError(err instanceof Error ? err.message : String(err));
    }
  };

  const download = useModelDownload({
    onComplete: async () => {
      setPreStepStatus("ready");
      try {
        await storageBridge.initializeEmbeddingModel();
        runTest();
      } catch (err) {
        setTestStatus("failed");
        download.setError(err instanceof Error ? err.message : String(err));
      }
    },
    onFailure: () => {
      setPreStepStatus("idle");
    },
  });

  // If a download is already in progress when we mount, skip capacity selection.
  useEffect(() => {
    if (download.attached) {
      setShowCapacitySelection(false);
      setPreStepStatus("ready");
    }
  }, [download.attached]);

  const startDownload = async () => {
    setShowCapacitySelection(false);
    setPreStepStatus("preparing");
    try {
      await new Promise((resolve) => setTimeout(resolve, 300));
      await download.start(() => storageBridge.startEmbeddingDownload(downloadVersion));
      await updateAdvancedSetting("embeddingMaxTokens", selectedCapacity);
    } catch {
      setPreStepStatus("idle");
    }
  };

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    (async () => {
      unlisten = await listen<{ current: number; total: number; stage?: string }>(
        "embedding_test_progress",
        (event) => {
          setTestProgress({ current: event.payload.current, total: event.payload.total });
        },
      );
    })();
    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  useEffect(() => {
    if (testStatus === "passed") {
      setCountdown(5);
      const timer = setInterval(() => {
        setCountdown((prev) => {
          if (prev <= 1) {
            clearInterval(timer);
            navigate(returnTo ?? "/settings/advanced", { replace: true });
            return 0;
          }
          return prev - 1;
        });
      }, 1000);
      return () => clearInterval(timer);
    }
  }, [testStatus, navigate, returnTo]);

  const handleCancel = async () => {
    try {
      await download.cancel();
      await updateAdvancedSetting("dynamicMemory", {
        enabled: false,
        summaryMessageInterval: 20,
        maxEntries: 50,
        minSimilarityThreshold: 0.32,
        retrievalLimit: 5,
        retrievalStrategy: "smart",
        hotMemoryTokenBudget: 2000,
        decayRate: 0.08,
        coldThreshold: 0.3,
      });
      navigate(returnTo ?? "/settings/advanced");
    } catch {
      // error surfaced via download.error
    }
  };

  const phase: ModelDownloadPhase =
    testStatus === "testing"
      ? "verifying"
      : testStatus === "passed"
        ? "passed"
        : testStatus === "failed" || download.progress.status === "failed"
          ? "failed"
          : download.isDownloading || download.progress.status === "downloading"
            ? "downloading"
            : "idle";

  const headerTitle = showCapacitySelection
    ? isUpgrade
      ? `Upgrade to ${downloadVersionLabel} Embedding Model`
      : `Setup ${downloadVersionLabel} Embedding Model`
    : testStatus === "testing"
      ? "Testing Model"
      : testStatus === "passed"
        ? "Test Passed!"
        : testStatus === "failed"
          ? "Test Failed"
          : preStepStatus === "preparing"
            ? "Preparing Download"
            : `Downloading ${downloadVersionLabel} Embedding Model`;

  const headerDescription = showCapacitySelection
    ? "Choose your preferred memory context capacity"
    : testStatus === "idle"
      ? preStepStatus === "preparing"
        ? "Preparing download..."
        : "Dynamic Memory installs the local embedding model"
      : testStatus === "testing"
        ? "Verifying model functionality..."
        : testStatus === "passed"
          ? `Redirecting in ${countdown} seconds...`
          : "Model verification failed. Please try again.";

  const statusText =
    testStatus === "idle" && preStepStatus === "preparing"
      ? "Preparing download..."
      : testStatus === "idle" && download.progress.status === "downloading"
        ? `Downloading ${downloadVersionLabel} files...`
        : testStatus === "idle" && download.progress.status === "completed"
          ? "Download completed!"
          : testStatus === "idle" && download.progress.status === "failed"
            ? "Download failed"
            : testStatus === "idle" && download.progress.status === "cancelled"
              ? "Download cancelled"
              : testStatus === "testing"
                ? "Running verification tests..."
                : testStatus === "passed"
                  ? "All tests passed successfully"
                  : testStatus === "failed"
                    ? "Verification failed"
                    : "";

  return (
    <div className="flex min-h-screen flex-col">
      <main className="flex-1 px-4 pb-24 pt-6">
        <motion.div
          initial={{ opacity: 0, y: 16 }}
          animate={{ opacity: 1, y: 0 }}
          className="mx-auto w-full max-w-2xl space-y-6"
        >
          <div className="text-center">
            <h1 className="text-2xl font-bold text-fg">{headerTitle}</h1>
            <p className="mt-2 text-sm text-fg/60">{headerDescription}</p>
            {!showCapacitySelection &&
              testStatus === "testing" &&
              testProgress &&
              testProgress.total > 0 && (
                <p className="mt-2 text-xs text-fg/50">
                  Testing {testProgress.current}/{testProgress.total}
                </p>
              )}
          </div>

          {showCapacitySelection && (
            <div className="rounded-xl border border-fg/10 bg-fg/5 p-6">
              <div className="space-y-4">
                <div className="text-center mb-6">
                  <div className="flex h-16 w-16 mx-auto items-center justify-center rounded-full border border-info/20 bg-info/10">
                    <Zap className="h-8 w-8 text-info" />
                  </div>
                  <p className="mt-4 text-sm text-fg/70">
                    Higher capacity = better memory for longer conversations, but uses more
                    processing power per embedding.
                  </p>
                </div>

                <div className="grid gap-3">
                  {CAPACITY_OPTIONS.map((option) => (
                    <button
                      key={option.value}
                      onClick={() => setSelectedCapacity(option.value)}
                      className={`flex items-center justify-between p-4 rounded-xl border transition-all ${
                        selectedCapacity === option.value
                          ? "border-info bg-info/20"
                          : "border-fg/10 bg-fg/5 hover:border-fg/20"
                      }`}
                    >
                      <div className="flex items-center gap-3">
                        <div
                          className={`w-5 h-5 rounded-full border-2 flex items-center justify-center ${
                            selectedCapacity === option.value
                              ? "border-info bg-info"
                              : "border-fg/30"
                          }`}
                        >
                          {selectedCapacity === option.value && (
                            <div className="w-2 h-2 rounded-full bg-fg" />
                          )}
                        </div>
                        <div className="text-left">
                          <div className="font-medium text-fg">{option.label}</div>
                          <div className="text-xs text-fg/50">{option.description}</div>
                        </div>
                      </div>
                      {option.value === 2048 && (
                        <span className="text-[10px] font-medium uppercase tracking-wider text-accent bg-accent/20 px-2 py-0.5 rounded">
                          Recommended
                        </span>
                      )}
                    </button>
                  ))}
                </div>

                <button
                  onClick={startDownload}
                  className="w-full mt-4 py-3 px-4 rounded-xl bg-info hover:bg-info/80 text-fg font-medium transition-colors flex items-center justify-center gap-2"
                >
                  <Download className="h-5 w-5" />
                  Download {downloadVersionLabel} Model ({downloadApproxSize})
                </button>

                <button
                  onClick={() => navigate(returnTo ?? "/settings/advanced")}
                  className="w-full py-2 text-sm text-fg/50 hover:text-fg/70 transition-colors"
                >
                  {t("common.buttons.cancel")}
                </button>
              </div>
            </div>
          )}

          {!showCapacitySelection && (
            <>
              <ModelDownloadProgress
                progress={download.progress}
                phase={phase}
                statusText={statusText}
              />

              {testResults && (testStatus === "passed" || testStatus === "failed") && (
                <div className="rounded-xl border border-fg/10 bg-fg/5 p-4 space-y-3">
                  <div className="text-center">
                    <div
                      className={`text-sm font-medium ${
                        testResults.success ? "text-accent" : "text-danger/80"
                      }`}
                    >
                      {testResults.message}
                    </div>
                  </div>
                  <div className="space-y-2">
                    {[
                      {
                        label: "Health",
                        passed: testResults.health.passed,
                        detail: `identity ${testResults.health.identityCosine.toFixed(4)}`,
                      },
                      {
                        label: "Retrieval",
                        passed: testResults.retrieval.passed,
                        detail: `top-1 ${Math.round(testResults.retrieval.top1Rate * 100)}% · mrr ${testResults.retrieval.mrr.toFixed(2)}`,
                      },
                      {
                        label: "Separation",
                        passed: testResults.separation.passed,
                        detail: `margin ${testResults.separation.margin.toFixed(3)}`,
                      },
                    ].map((row) => (
                      <div
                        key={row.label}
                        className="rounded-lg border border-fg/10 bg-fg/5 p-3 flex items-center justify-between gap-3"
                      >
                        <div className="flex items-center gap-2">
                          <span
                            className={`inline-flex h-1.5 w-1.5 rounded-full ${
                              row.passed ? "bg-accent" : "bg-danger/80"
                            }`}
                          />
                          <span className="text-xs font-medium text-fg/70">{row.label}</span>
                        </div>
                        <span className="text-xs text-fg/50 tabular-nums">{row.detail}</span>
                      </div>
                    ))}
                  </div>
                </div>
              )}

              {download.error && (
                <div className="rounded-lg border border-danger/20 bg-danger/10 p-3">
                  <p className="text-sm text-danger/80">{download.error}</p>
                </div>
              )}
            </>
          )}

          {download.isDownloading && (
            <button
              onClick={handleCancel}
              className="flex w-full items-center justify-center gap-2 rounded-xl border border-danger/20 bg-danger/10 px-6 py-3 text-sm font-medium text-danger/80 transition hover:border-danger/30 hover:bg-danger/15"
            >
              <X className="h-4 w-4" />
              {t("common.buttons.cancel")}
            </button>
          )}

          {!download.isDownloading && download.progress.status === "failed" && (
            <div className="space-y-3">
              <button
                onClick={() => {
                  download.setError(null);
                  startDownload();
                }}
                className="flex w-full items-center justify-center gap-2 rounded-xl bg-info px-6 py-3 text-sm font-medium text-fg transition hover:bg-info/80"
              >
                <Download className="h-4 w-4" />
                Retry Download
              </button>
              <button
                onClick={() => navigate(returnTo ?? "/settings/advanced")}
                className="flex w-full items-center justify-center gap-2 rounded-xl border border-fg/10 bg-fg/5 px-6 py-3 text-sm font-medium text-fg/60 transition hover:bg-fg/10"
              >
                Go Back
              </button>
            </div>
          )}
        </motion.div>
      </main>
    </div>
  );
}
