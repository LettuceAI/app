import { useState, useEffect } from "react";
import { motion } from "framer-motion";
import { Download, X, CheckCircle, XCircle, Loader2, Zap } from "lucide-react";
import { storageBridge } from "../../../core/storage/files";
import { updateAdvancedSetting } from "../../../core/storage/advanced";
import { useNavigate, useSearchParams } from "react-router-dom";
import { listen } from "@tauri-apps/api/event";

type Capacity = 1024 | 2048 | 4096;

const CAPACITY_OPTIONS: { value: Capacity; label: string; description: string }[] = [
  { value: 1024, label: "1K tokens", description: "Best for quick responses" },
  { value: 2048, label: "2K tokens", description: "Balanced performance" },
  { value: 4096, label: "4K tokens", description: "Maximum context" },
];

export function EmbeddingDownloadPage() {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const returnTo = searchParams.get("returnTo");
  const isUpgrade = searchParams.get("upgrade") === "true";
  const requestedVersion = searchParams.get("version");
  const downloadVersion = requestedVersion === "v2" ? "v2" : "v3";
  const downloadVersionLabel = downloadVersion.toUpperCase();
  const downloadApproxSize = downloadVersion === "v2" ? "~132 MB" : "~86 MB";

  // Capacity selection state
  const [showCapacitySelection, setShowCapacitySelection] = useState(true);
  const [selectedCapacity, setSelectedCapacity] = useState<Capacity>(2048);

  const [progress, setProgress] = useState<{
    downloaded: number;
    total: number;
    status: string;
    currentFileIndex: number;
    totalFiles: number;
    currentFileName: string;
  }>({
    downloaded: 0,
    total: 0,
    status: "idle",
    currentFileIndex: 0,
    totalFiles: 0,
    currentFileName: "",
  });
  const [error, setError] = useState<string | null>(null);
  const [isDownloading, setIsDownloading] = useState(false);
  const [preStepStatus, setPreStepStatus] = useState<"idle" | "preparing" | "ready">("idle");
  const [testStatus, setTestStatus] = useState<"idle" | "testing" | "passed" | "failed">("idle");
  const [testResults, setTestResults] = useState<{
    success: boolean;
    message: string;
    scores: Array<{
      pairName: string;
      textA: string;
      textB: string;
      similarityScore: number;
      expected: string;
    }>;
  } | null>(null);
  const [countdown, setCountdown] = useState<number>(5);
  const [testProgress, setTestProgress] = useState<{ current: number; total: number } | null>(null);

  // Setup progress listener on mount
  useEffect(() => {
    let mounted = true;
    let unsubscribe: (() => void) | null = null;

    const setupListener = async () => {
      try {
        const currentProgress = await storageBridge.getEmbeddingDownloadProgress();

        // Setup listener for progress updates
        unsubscribe = await storageBridge.listenToEmbeddingDownloadProgress((progressData) => {
          console.log("Progress update:", progressData);
          if (mounted) {
            setProgress(progressData);

            if (progressData.status === "completed") {
              setPreStepStatus("ready");
              setIsDownloading(false);
              void (async () => {
                try {
                  await storageBridge.initializeEmbeddingModel();
                  runTest();
                } catch (err) {
                  setTestStatus("failed");
                  setError(err instanceof Error ? err.message : String(err));
                }
              })();
            } else if (progressData.status === "failed" || progressData.status === "cancelled") {
              setIsDownloading(false);
              setPreStepStatus("idle");
            } else if (progressData.status === "downloading") {
              setPreStepStatus("ready");
            }
          }
        });

        // If already downloading, attach to it and skip capacity selection
        if (currentProgress.status === "downloading") {
          console.log("Download already in progress, attaching...");
          setShowCapacitySelection(false);
          setIsDownloading(true);
          setPreStepStatus("ready");
          setProgress(currentProgress);
        }
      } catch (err) {
        console.error("Failed to setup listener:", err);
      }
    };

    setupListener();

    return () => {
      mounted = false;
      if (unsubscribe) {
        unsubscribe();
      }
    };
  }, [navigate, returnTo]);

  // Start download with selected capacity
  const startDownload = async () => {
    setShowCapacitySelection(false);
    setIsDownloading(true);
    setError(null);
    setPreStepStatus("preparing");

    try {
      await new Promise((resolve) => setTimeout(resolve, 300));
      await storageBridge.startEmbeddingDownload(downloadVersion);

      // Save the selected capacity to settings
      await updateAdvancedSetting("embeddingMaxTokens", selectedCapacity);
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      setPreStepStatus("idle");

      if (errorMessage.includes("Download already in progress")) {
        console.log("Race condition caught, attaching to existing download...");
        setIsDownloading(true);
        setPreStepStatus("ready");
      } else {
        setError(errorMessage);
        setIsDownloading(false);
      }
    }
  };

  // Run embedding test
  const runTest = async () => {
    setTestStatus("testing");
    setError(null);
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
          minSimilarityThreshold: 0.35,
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
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    const setupListener = async () => {
      unlisten = await listen<{ current: number; total: number; stage?: string }>(
        "embedding_test_progress",
        (event) => {
          setTestProgress({ current: event.payload.current, total: event.payload.total });
        },
      );
    };
    setupListener();
    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, []);

  // Countdown and redirect after successful test
  useEffect(() => {
    if (testStatus === "passed") {
      setCountdown(5);
      const timer = setInterval(() => {
        setCountdown((prev) => {
          if (prev <= 1) {
            clearInterval(timer);
            if (returnTo) {
              navigate(returnTo, { replace: true });
            } else {
              navigate("/settings/advanced", { replace: true });
            }
            return 0;
          }
          return prev - 1;
        });
      }, 1000);

      return () => clearInterval(timer);
    }
  }, [testStatus, navigate]);

  const handleCancel = async () => {
    try {
      await storageBridge.cancelEmbeddingDownload();
      await updateAdvancedSetting("dynamicMemory", {
        enabled: false,
        summaryMessageInterval: 20,
        maxEntries: 50,
        minSimilarityThreshold: 0.35,
        retrievalLimit: 5,
        retrievalStrategy: "smart",
        hotMemoryTokenBudget: 2000,
        decayRate: 0.08,
        coldThreshold: 0.3,
      });
      if (returnTo) {
        navigate(returnTo);
      } else {
        navigate("/settings/advanced");
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const formatBytes = (bytes: number): string => {
    if (bytes === 0) return "0 Bytes";
    const k = 1024;
    const sizes = ["Bytes", "KB", "MB", "GB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return Math.round((bytes / Math.pow(k, i)) * 100) / 100 + " " + sizes[i];
  };

  const progressPercent =
    testStatus !== "idle"
      ? 100
      : progress.total > 0
        ? (progress.downloaded / progress.total) * 100
        : 0;

  const headerDescription = showCapacitySelection
    ? "Choose your preferred memory context capacity"
    : testStatus === "idle"
      ? preStepStatus === "preparing"
        ? "Preparing download..."
        : "Dynamic Memory requires a local embedding model to function"
      : testStatus === "testing"
        ? "Verifying model functionality..."
        : testStatus === "passed"
          ? `Redirecting in ${countdown} seconds...`
          : testStatus === "failed"
            ? "Model verification failed. Please try again."
            : "";

  return (
    <div className="flex min-h-screen flex-col">
      <main className="flex-1 px-4 pb-24 pt-6">
        <motion.div
          initial={{ opacity: 0, y: 16 }}
          animate={{ opacity: 1, y: 0 }}
          className="mx-auto w-full max-w-2xl space-y-6"
        >
          {/* Header */}
          <div className="text-center">
            <h1 className="text-2xl font-bold text-fg">
              {showCapacitySelection &&
                (isUpgrade
                  ? `Upgrade to ${downloadVersionLabel} Embedding Model`
                  : `Setup ${downloadVersionLabel} Embedding Model`)}
              {!showCapacitySelection &&
                testStatus === "idle" &&
                (preStepStatus === "preparing"
                  ? "Preparing Download"
                  : `Downloading ${downloadVersionLabel} Embedding Model`)}
              {!showCapacitySelection && testStatus === "testing" && "Testing Model"}
              {!showCapacitySelection && testStatus === "passed" && "Test Passed!"}
              {!showCapacitySelection && testStatus === "failed" && "Test Failed"}
            </h1>
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

          {/* Capacity Selection */}
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
                      className={`
                                                flex items-center justify-between p-4 rounded-xl border transition-all
                                                ${
                                                  selectedCapacity === option.value
                                                    ? "border-info bg-info/20"
                                                    : "border-fg/10 bg-fg/5 hover:border-fg/20"
                                                }
                                            `}
                    >
                      <div className="flex items-center gap-3">
                        <div
                          className={`
                                                    w-5 h-5 rounded-full border-2 flex items-center justify-center
                                                    ${
                                                      selectedCapacity === option.value
                                                        ? "border-info bg-info"
                                                        : "border-fg/30"
                                                    }
                                                `}
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
                  onClick={() => {
                    if (returnTo) {
                      navigate(returnTo);
                    } else {
                      navigate("/settings/advanced");
                    }
                  }}
                  className="w-full py-2 text-sm text-fg/50 hover:text-fg/70 transition-colors"
                >
                  Cancel
                </button>
              </div>
            </div>
          )}

          {/* Download/Test Progress Card */}
          {!showCapacitySelection && (
            <div className="rounded-xl border border-fg/10 bg-fg/5 p-6">
              {/* Icon */}
              <div className="mb-4 flex justify-center">
                {testStatus === "idle" && (
                  <div className="flex h-16 w-16 items-center justify-center rounded-full border border-info/20 bg-info/10">
                    <Download className="h-8 w-8 text-info" />
                  </div>
                )}
                {testStatus === "testing" && (
                  <div className="flex h-16 w-16 items-center justify-center rounded-full border border-warning/20 bg-warning/10">
                    <Loader2 className="h-8 w-8 text-warning animate-spin" />
                  </div>
                )}
                {testStatus === "passed" && (
                  <div className="flex h-16 w-16 items-center justify-center rounded-full border border-accent/20 bg-accent/10">
                    <CheckCircle className="h-8 w-8 text-accent" />
                  </div>
                )}
                {testStatus === "failed" && (
                  <div className="flex h-16 w-16 items-center justify-center rounded-full border border-danger/20 bg-danger/10">
                    <XCircle className="h-8 w-8 text-danger/80" />
                  </div>
                )}
              </div>

              <div className="space-y-6">
                {/* File Info - Above Progress Bar */}
                {progress.status.toLowerCase().includes("downloading") && (
                  <div className="space-y-2">
                    <div className="text-center">
                      <div className="text-sm font-medium text-fg/80">
                        File {progress.currentFileIndex} of {progress.totalFiles}
                      </div>
                      {progress.currentFileName && (
                        <div className="mt-1 text-xs text-fg/50 font-mono">
                          {progress.currentFileName}
                        </div>
                      )}
                    </div>
                  </div>
                )}

                {/* Progress Bar */}
                <div className="space-y-2">
                  <div className="flex justify-between text-sm">
                    <span className="text-fg/70">Progress</span>
                    <span className="font-medium text-fg">{Math.round(progressPercent)}%</span>
                  </div>
                  <div className="h-2 w-full overflow-hidden rounded-full bg-fg/10">
                    <motion.div
                      className="h-full bg-accent"
                      initial={{ width: 0 }}
                      animate={{
                        width:
                          progress.total > 0
                            ? `${(progress.downloaded / progress.total) * 100}%`
                            : "0%",
                      }}
                      transition={{ duration: 0.3 }}
                    />
                  </div>
                  <div className="flex justify-between text-xs text-fg/50">
                    <span>{formatBytes(progress.downloaded)}</span>
                    <span>{formatBytes(progress.total)}</span>
                  </div>
                </div>

                {/* Status */}
                <div className="text-center text-sm text-fg/60">
                  {testStatus === "idle" &&
                    preStepStatus === "preparing" &&
                    "Preparing download..."}
                  {testStatus === "idle" &&
                    preStepStatus !== "preparing" &&
                    progress.status === "downloading" &&
                    `Downloading ${downloadVersionLabel} files...`}
                  {testStatus === "idle" &&
                    progress.status === "completed" &&
                    "Download completed!"}
                  {testStatus === "idle" && progress.status === "failed" && "Download failed"}
                  {testStatus === "idle" && progress.status === "cancelled" && "Download cancelled"}
                  {testStatus === "testing" && "Running verification tests..."}
                  {testStatus === "passed" && "All tests passed successfully"}
                  {testStatus === "failed" && "Verification failed"}
                </div>

                {/* Test Results */}
                {testResults && (testStatus === "passed" || testStatus === "failed") && (
                  <div className="mt-6 space-y-4">
                    <div className="text-center">
                      <div
                        className={`text-sm font-medium ${
                          testResults.success ? "text-accent" : "text-danger/80"
                        }`}
                      >
                        {testResults.message}
                      </div>
                    </div>

                    {/* Similarity Scores */}
                    <div className="space-y-3">
                      {testResults.scores.map((score, idx) => (
                        <div key={idx} className="rounded-lg border border-fg/10 bg-fg/5 p-3">
                          <div className="flex items-center justify-between">
                            <span className="text-xs font-medium text-fg/70">
                              {score.pairName}
                            </span>
                            <span
                              className={`text-sm font-bold ${
                                score.similarityScore > 0.6 ? "text-accent" : "text-warning"
                              }`}
                            >
                              {score.similarityScore.toFixed(4)}
                            </span>
                          </div>
                          <div className="mt-1 text-xs text-fg/50">
                            Expected: {score.expected}
                          </div>
                        </div>
                      ))}
                    </div>
                  </div>
                )}
              </div>

              {/* Error Message */}
              {error && (
                <div className="mt-4 rounded-lg border border-danger/20 bg-danger/10 p-3">
                  <p className="text-sm text-danger/80">{error}</p>
                </div>
              )}
            </div>
          )}

          {/* Cancel Button */}
          {isDownloading && (
            <button
              onClick={handleCancel}
              className="flex w-full items-center justify-center gap-2 rounded-xl border border-danger/20 bg-danger/10 px-6 py-3 text-sm font-medium text-danger/80 transition hover:border-danger/30 hover:bg-danger/15"
            >
              <X className="h-4 w-4" />
              Cancel Download
            </button>
          )}

          {/* Retry Button */}
          {!isDownloading && progress.status === "failed" && (
            <div className="space-y-3">
              <button
                onClick={() => {
                  setError(null);
                  startDownload();
                }}
                className="flex w-full items-center justify-center gap-2 rounded-xl bg-info px-6 py-3 text-sm font-medium text-fg transition hover:bg-info/80"
              >
                <Download className="h-4 w-4" />
                Retry Download
              </button>
              <button
                onClick={() => {
                  if (returnTo) {
                    navigate(returnTo);
                  } else {
                    navigate("/settings/advanced");
                  }
                }}
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
