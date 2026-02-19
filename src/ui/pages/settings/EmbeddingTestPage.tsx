import { useState, useEffect } from "react";
import { motion, AnimatePresence } from "framer-motion";
import {
  CheckCircle,
  XCircle,
  Loader2,
  RefreshCw,
  Sparkles,
  AlertTriangle,
  ChevronDown,
  Search,
  Cpu,
  Zap,
} from "lucide-react";
import { storageBridge } from "../../../core/storage/files";
import { cn, interactive } from "../../design-tokens";
import { listen } from "@tauri-apps/api/event";

type TestStatus = "idle" | "testing" | "passed" | "failed";

interface ScoreResult {
  pairName: string;
  textA: string;
  textB: string;
  similarityScore: number;
  expected: string;
  passed: boolean;
  category: string;
}

interface ModelInfo {
  version: string;
  maxTokens: number;
  embeddingDimensions: number;
}

interface TestResults {
  success: boolean;
  message: string;
  scores: ScoreResult[];
  modelInfo: ModelInfo;
}

interface DevBenchmarkResults {
  maxTokensUsed: number;
  v2: {
    version: string;
    sampleCount: number;
    averageMs: number;
    p95Ms: number;
    minMs: number;
    maxMs: number;
  };
  v3: {
    version: string;
    sampleCount: number;
    averageMs: number;
    p95Ms: number;
    minMs: number;
    maxMs: number;
  };
  pairDeltas: Array<{
    pairName: string;
    v2Similarity: number;
    v3Similarity: number;
    delta: number;
  }>;
  averageSpeedupV3VsV2: number;
}

const CATEGORY_INFO: Record<string, { label: string; color: string; icon: React.ReactNode }> = {
  semantic: {
    label: "Semantic Similarity",
    color: "blue",
    icon: <Sparkles className="h-3.5 w-3.5" />,
  },
  dissimilar: {
    label: "Dissimilarity Check",
    color: "orange",
    icon: <AlertTriangle className="h-3.5 w-3.5" />,
  },
  roleplay: {
    label: "Roleplay Context",
    color: "purple",
    icon: <Zap className="h-3.5 w-3.5" />,
  },
};

function getCategoryStyles(category: string, type: "badge" | "border") {
  const info = CATEGORY_INFO[category] || CATEGORY_INFO.semantic;
  if (type === "badge") {
    switch (info.color) {
      case "blue":
        return "border-info/30 bg-info/10 text-info/80";
      case "orange":
        return "border-warning/30 bg-warning/10 text-warning/80";
      case "purple":
        return "border-secondary/30 bg-secondary/10 text-secondary/80";
      default:
        return "border-fg/20 bg-fg/10 text-fg/70";
    }
  }
  return "";
}

function SimilarityBar({ score, passed }: { score: number; passed: boolean }) {
  const percentage = Math.round(score * 100);
  return (
    <div className="flex items-center gap-3">
      <div className="flex-1 h-2 bg-fg/10 rounded-full overflow-hidden">
        <motion.div
          initial={{ width: 0 }}
          animate={{ width: `${percentage}%` }}
          transition={{ duration: 0.5, ease: "easeOut" }}
          className={cn("h-full rounded-full", passed ? "bg-accent" : "bg-danger")}
        />
      </div>
      <span
        className={cn(
          "text-sm font-bold tabular-nums w-14 text-right",
          passed ? "text-accent/80" : "text-danger/80",
        )}
      >
        {percentage}%
      </span>
    </div>
  );
}

function TestResultCard({
  result,
  isExpanded,
  onToggle,
}: {
  result: ScoreResult;
  isExpanded: boolean;
  onToggle: () => void;
}) {
  const categoryInfo = CATEGORY_INFO[result.category] || CATEGORY_INFO.semantic;

  return (
    <div
      className={cn(
        "rounded-xl border overflow-hidden transition-all",
        result.passed ? "border-fg/10 bg-fg/5" : "border-danger/30 bg-danger/5",
      )}
    >
      <button
        onClick={onToggle}
        className="w-full flex items-center gap-3 px-4 py-3 text-left hover:bg-fg/5 transition-colors"
      >
        <div
          className={cn(
            "flex h-6 w-6 shrink-0 items-center justify-center rounded-full",
            result.passed ? "bg-accent/20" : "bg-danger/20",
          )}
        >
          {result.passed ? (
            <CheckCircle className="h-4 w-4 text-accent/80" />
          ) : (
            <XCircle className="h-4 w-4 text-danger/80" />
          )}
        </div>

        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 flex-wrap">
            <span className="text-sm font-medium text-fg truncate">{result.pairName}</span>
            <span
              className={cn(
                "inline-flex items-center gap-1 px-1.5 py-0.5 rounded border text-[10px] font-medium",
                getCategoryStyles(result.category, "badge"),
              )}
            >
              {categoryInfo.icon}
              {categoryInfo.label}
            </span>
          </div>
        </div>

        <div className="flex items-center gap-2 shrink-0">
          <span
            className={cn(
              "text-lg font-bold tabular-nums",
              result.passed ? "text-accent/80" : "text-danger/80",
            )}
          >
            {Math.round(result.similarityScore * 100)}%
          </span>
          <ChevronDown
            className={cn("h-4 w-4 text-fg/40 transition-transform", isExpanded && "rotate-180")}
          />
        </div>
      </button>

      <AnimatePresence>
        {isExpanded && (
          <motion.div
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: "auto", opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={{ duration: 0.2 }}
            className="overflow-hidden"
          >
            <div className="px-4 pb-4 space-y-3 border-t border-fg/5 pt-3">
              <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
                <div className="bg-surface-el/30 rounded-lg p-3">
                  <span className="text-[10px] uppercase tracking-wider text-fg/40 block mb-1.5">
                    Text A
                  </span>
                  <p className="text-xs text-fg/70 leading-relaxed">"{result.textA}"</p>
                </div>
                <div className="bg-surface-el/30 rounded-lg p-3">
                  <span className="text-[10px] uppercase tracking-wider text-fg/40 block mb-1.5">
                    Text B
                  </span>
                  <p className="text-xs text-fg/70 leading-relaxed">"{result.textB}"</p>
                </div>
              </div>

              <SimilarityBar score={result.similarityScore} passed={result.passed} />

              <div className="text-xs text-fg/50">
                <span className="text-fg/30">Expected:</span> {result.expected}
              </div>
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

export function EmbeddingTestPage() {
  const [testStatus, setTestStatus] = useState<TestStatus>("idle");
  const [testResults, setTestResults] = useState<TestResults | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [expandedCards, setExpandedCards] = useState<Set<number>>(new Set());
  const [showAllResults, setShowAllResults] = useState(false);
  const [testProgress, setTestProgress] = useState<{ current: number; total: number } | null>(null);

  // Custom comparison state
  const [customTextA, setCustomTextA] = useState("");
  const [customTextB, setCustomTextB] = useState("");
  const [customScore, setCustomScore] = useState<number | null>(null);
  const [comparingCustom, setComparingCustom] = useState(false);
  const [customError, setCustomError] = useState<string | null>(null);
  const [benchmarkStatus, setBenchmarkStatus] = useState<"idle" | "running" | "done" | "failed">(
    "idle",
  );
  const [benchmarkResults, setBenchmarkResults] = useState<DevBenchmarkResults | null>(null);
  const [benchmarkError, setBenchmarkError] = useState<string | null>(null);
  const isDevBuild = import.meta.env.DEV;

  const runTest = async () => {
    setTestStatus("testing");
    setError(null);
    setTestResults(null);
    setExpandedCards(new Set());
    setTestProgress({ current: 0, total: 0 });

    try {
      await storageBridge.initializeEmbeddingModel();
      const results = await storageBridge.runEmbeddingTest();
      setTestResults(results);
      setTestStatus(results.success ? "passed" : "failed");

      // Auto-expand failed tests
      const failedIndices = new Set<number>();
      results.scores.forEach((score, idx) => {
        if (!score.passed) failedIndices.add(idx);
      });
      setExpandedCards(failedIndices);
    } catch (err) {
      setTestStatus("failed");
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const compareCustom = async () => {
    if (!customTextA.trim() || !customTextB.trim()) {
      setCustomError("Please enter both texts to compare");
      return;
    }

    setComparingCustom(true);
    setCustomError(null);
    setCustomScore(null);

    try {
      const score = await storageBridge.compareCustomTexts(customTextA, customTextB);
      setCustomScore(score);
    } catch (err) {
      setCustomError(err instanceof Error ? err.message : String(err));
    } finally {
      setComparingCustom(false);
    }
  };

  const runDevBenchmark = async () => {
    setBenchmarkStatus("running");
    setBenchmarkError(null);
    setBenchmarkResults(null);
    try {
      const results = await storageBridge.runEmbeddingDevBenchmark();
      setBenchmarkResults(results);
      setBenchmarkStatus("done");
    } catch (err) {
      setBenchmarkError(err instanceof Error ? err.message : String(err));
      setBenchmarkStatus("failed");
    }
  };

  const toggleCard = (idx: number) => {
    setExpandedCards((prev) => {
      const next = new Set(prev);
      if (next.has(idx)) {
        next.delete(idx);
      } else {
        next.add(idx);
      }
      return next;
    });
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

  // Run test automatically on mount
  useEffect(() => {
    runTest();
  }, []);

  // Categorize results
  const categorizedResults = testResults?.scores.reduce(
    (acc, score, idx) => {
      const cat = score.category || "semantic";
      if (!acc[cat]) acc[cat] = [];
      acc[cat].push({ ...score, originalIndex: idx });
      return acc;
    },
    {} as Record<string, (ScoreResult & { originalIndex: number })[]>,
  );

  const passedCount = testResults?.scores.filter((s) => s.passed).length ?? 0;
  const totalCount = testResults?.scores.length ?? 0;
  const failedCount = totalCount - passedCount;

  return (
    <div className="flex min-h-screen flex-col">
      <main className="flex-1 px-4 pb-24 pt-4">
        <div className="mx-auto w-full max-w-2xl space-y-6">
          {/* Test Status Card */}
          <motion.div
            initial={{ opacity: 0, y: 16 }}
            animate={{ opacity: 1, y: 0 }}
            className="rounded-xl border border-fg/10 bg-fg/5 p-6"
          >
            <div className="flex flex-col items-center text-center">
              <div className="mb-4">
                {testStatus === "testing" && (
                  <div className="flex h-16 w-16 items-center justify-center rounded-full border border-info/30 bg-info/10">
                    <Loader2 className="h-8 w-8 text-info animate-spin" />
                  </div>
                )}
                {testStatus === "passed" && (
                  <div className="flex h-16 w-16 items-center justify-center rounded-full border border-accent/30 bg-accent/10">
                    <CheckCircle className="h-8 w-8 text-accent/80" />
                  </div>
                )}
                {testStatus === "failed" && (
                  <div className="flex h-16 w-16 items-center justify-center rounded-full border border-danger/30 bg-danger/10">
                    <XCircle className="h-8 w-8 text-danger/80" />
                  </div>
                )}
              </div>

              <h2 className="text-lg font-semibold text-fg mb-1">
                {testStatus === "testing" && "Running Tests..."}
                {testStatus === "passed" && "All Tests Passed"}
                {testStatus === "failed" && (error ? "Test Error" : "Some Tests Failed")}
              </h2>

              {testResults && (
                <div className="flex items-center gap-4 mt-2 mb-3">
                  <div className="flex items-center gap-1.5">
                    <CheckCircle className="h-4 w-4 text-accent/80" />
                    <span className="text-sm text-accent/80 font-medium">
                      {passedCount} passed
                    </span>
                  </div>
                  {failedCount > 0 && (
                    <div className="flex items-center gap-1.5">
                      <XCircle className="h-4 w-4 text-danger/80" />
                      <span className="text-sm text-danger/80 font-medium">{failedCount} failed</span>
                    </div>
                  )}
                </div>
              )}

              <p className="text-sm text-fg/60 max-w-md">
                {testStatus === "testing" &&
                  "Computing embeddings and comparing similarity scores..."}
                {testStatus === "passed" &&
                  "The embedding model is correctly identifying semantic relationships."}
                {testStatus === "failed" &&
                  (error ||
                    "Some tests produced unexpected results. Consider reinstalling the model.")}
              </p>
              {testStatus === "testing" && testProgress && testProgress.total > 0 && (
                <div className="mt-3 text-xs text-fg/50">
                  Testing {testProgress.current}/{testProgress.total}
                </div>
              )}

              {testStatus === "failed" && failedCount >= 4 && !error && (
                <div className="mt-4 rounded-lg border border-danger/30 bg-danger/10 p-3">
                  <div className="flex items-start gap-2">
                    <AlertTriangle className="h-5 w-5 text-danger/80 shrink-0 mt-0.5" />
                    <div className="min-w-0">
                      <p className="text-sm font-medium text-danger/80">Model May Be Corrupted</p>
                      <p className="text-xs text-danger/80/70 mt-1">
                        {failedCount} or more tests failed. The embedding model may be corrupted or
                        incompatible. We recommend reinstalling it.
                      </p>
                    </div>
                  </div>
                </div>
              )}

              {testStatus !== "testing" && (
                <button
                  onClick={runTest}
                  className={cn(
                    "mt-4 inline-flex items-center gap-2 rounded-lg px-4 py-2 text-sm font-medium",
                    "border border-fg/10 bg-fg/5 text-fg",
                    interactive.transition.fast,
                    "hover:bg-fg/10",
                  )}
                >
                  <RefreshCw className="h-4 w-4" />
                  Run Tests Again
                </button>
              )}
            </div>
          </motion.div>

          {/* Model Info */}
          {testResults?.modelInfo && (
            <motion.div
              initial={{ opacity: 0, y: 10 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ delay: 0.1 }}
              className="rounded-xl border border-fg/10 bg-fg/5 p-4"
            >
              <div className="flex items-center gap-3 mb-3">
                <div className="flex h-8 w-8 items-center justify-center rounded-full border border-fg/10 bg-fg/10">
                  <Cpu className="h-4 w-4 text-fg/70" />
                </div>
                <span className="text-sm font-medium text-fg">Model Information</span>
              </div>
              <div className="grid grid-cols-3 gap-3">
                <div className="bg-surface-el/30 rounded-lg p-3 text-center">
                  <span className="text-[10px] uppercase tracking-wider text-fg/40 block mb-1">
                    Version
                  </span>
                  <span className="text-sm font-semibold text-fg">
                    {testResults.modelInfo.version}
                  </span>
                </div>
                <div className="bg-surface-el/30 rounded-lg p-3 text-center">
                  <span className="text-[10px] uppercase tracking-wider text-fg/40 block mb-1">
                    Max Tokens
                  </span>
                  <span className="text-sm font-semibold text-fg">
                    {testResults.modelInfo.maxTokens.toLocaleString()}
                  </span>
                </div>
                <div className="bg-surface-el/30 rounded-lg p-3 text-center">
                  <span className="text-[10px] uppercase tracking-wider text-fg/40 block mb-1">
                    Dimensions
                  </span>
                  <span className="text-sm font-semibold text-fg">
                    {testResults.modelInfo.embeddingDimensions}
                  </span>
                </div>
              </div>
            </motion.div>
          )}

          {/* Detailed Results */}
          {testResults && categorizedResults && (
            <motion.div
              initial={{ opacity: 0, y: 10 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ delay: 0.2 }}
              className="space-y-4"
            >
              <div className="flex items-center justify-between">
                <h3 className="text-[10px] font-semibold uppercase tracking-[0.25em] text-fg/35 px-1">
                  Test Results
                </h3>
                <button
                  onClick={() => setShowAllResults(!showAllResults)}
                  className="text-xs text-fg/50 hover:text-fg/70 transition-colors"
                >
                  {showAllResults ? "Collapse All" : "Expand All"}
                </button>
              </div>

              {Object.entries(categorizedResults).map(([category, results]) => {
                const categoryInfo = CATEGORY_INFO[category] || CATEGORY_INFO.semantic;
                const categoryPassed = results.every((r) => r.passed);

                return (
                  <div key={category} className="space-y-2">
                    <div className="flex items-center gap-2 px-1">
                      <span
                        className={cn(
                          "inline-flex items-center gap-1.5 px-2 py-1 rounded-md border text-xs font-medium",
                          getCategoryStyles(category, "badge"),
                        )}
                      >
                        {categoryInfo.icon}
                        {categoryInfo.label}
                      </span>
                      <span className="text-[10px] text-fg/40">
                        {results.filter((r) => r.passed).length}/{results.length} passed
                      </span>
                      {categoryPassed && (
                        <CheckCircle className="h-3.5 w-3.5 text-accent/80 ml-auto" />
                      )}
                    </div>

                    <div className="space-y-2">
                      {results.map((result) => (
                        <TestResultCard
                          key={result.originalIndex}
                          result={result}
                          isExpanded={showAllResults || expandedCards.has(result.originalIndex)}
                          onToggle={() => toggleCard(result.originalIndex)}
                        />
                      ))}
                    </div>
                  </div>
                );
              })}
            </motion.div>
          )}

          {/* Developer Benchmark */}
          {isDevBuild && (
            <motion.div
              initial={{ opacity: 0, y: 10 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ delay: 0.24 }}
              className="rounded-xl border border-info/25 bg-info/5 p-4"
            >
              <div className="flex items-start justify-between gap-3">
                <div>
                  <h3 className="text-sm font-semibold text-info">Developer Benchmark</h3>
                  <p className="mt-1 text-xs text-info/70">
                    Compare local v2 and v3 models for speed and similarity outputs.
                  </p>
                </div>
                <button
                  onClick={runDevBenchmark}
                  disabled={benchmarkStatus === "running"}
                  className={cn(
                    "inline-flex items-center gap-2 rounded-lg border px-3 py-1.5 text-xs font-medium transition-colors",
                    benchmarkStatus === "running"
                      ? "cursor-not-allowed border-info/20 bg-info/10 text-info/60"
                      : "border-info/40 bg-info/20 text-info hover:bg-info/30",
                  )}
                >
                  {benchmarkStatus === "running" ? (
                    <>
                      <Loader2 className="h-3.5 w-3.5 animate-spin" />
                      Running...
                    </>
                  ) : (
                    <>
                      <Cpu className="h-3.5 w-3.5" />
                      Run v2 vs v3
                    </>
                  )}
                </button>
              </div>

              {benchmarkError && (
                <div className="mt-3 rounded-lg border border-danger/35 bg-danger/10 p-3 text-xs text-danger/80/80">
                  {benchmarkError}
                </div>
              )}

              {benchmarkResults && (
                <div className="mt-4 space-y-3">
                  <div className="grid grid-cols-2 gap-3">
                    <div className="rounded-lg border border-fg/10 bg-surface-el/30 p-3">
                      <div className="text-[10px] uppercase tracking-wider text-fg/40">v2</div>
                      <div className="mt-1 text-xs text-fg/80">
                        avg {benchmarkResults.v2.averageMs.toFixed(2)} ms
                      </div>
                      <div className="text-xs text-fg/60">
                        p95 {benchmarkResults.v2.p95Ms.toFixed(2)} ms
                      </div>
                    </div>
                    <div className="rounded-lg border border-fg/10 bg-surface-el/30 p-3">
                      <div className="text-[10px] uppercase tracking-wider text-fg/40">v3</div>
                      <div className="mt-1 text-xs text-fg/80">
                        avg {benchmarkResults.v3.averageMs.toFixed(2)} ms
                      </div>
                      <div className="text-xs text-fg/60">
                        p95 {benchmarkResults.v3.p95Ms.toFixed(2)} ms
                      </div>
                    </div>
                  </div>

                  <div className="rounded-lg border border-fg/10 bg-surface-el/20 p-3 text-xs text-fg/70">
                    Speedup (v3 vs v2):{" "}
                    <span className="font-semibold text-fg">
                      {benchmarkResults.averageSpeedupV3VsV2.toFixed(2)}x
                    </span>{" "}
                    at {benchmarkResults.maxTokensUsed} tokens max
                  </div>

                  <div className="space-y-2">
                    {benchmarkResults.pairDeltas.map((pair) => (
                      <div
                        key={pair.pairName}
                        className="rounded-lg border border-fg/10 bg-surface-el/25 p-3"
                      >
                        <div className="text-xs font-medium text-fg/85">{pair.pairName}</div>
                        <div className="mt-1 text-[11px] text-fg/60">
                          v2 {pair.v2Similarity.toFixed(4)} | v3 {pair.v3Similarity.toFixed(4)} |
                          delta {pair.delta >= 0 ? "+" : ""}
                          {pair.delta.toFixed(4)}
                        </div>
                      </div>
                    ))}
                  </div>
                </div>
              )}
            </motion.div>
          )}

          {/* Custom Comparison */}
          <motion.div
            initial={{ opacity: 0, y: 10 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ delay: 0.15 }}
            className="rounded-xl border border-fg/10 bg-fg/5 p-4"
          >
            <div className="flex items-center gap-3 mb-4">
              <div className="flex h-8 w-8 items-center justify-center rounded-full border border-info/30 bg-info/10">
                <Search className="h-4 w-4 text-info" />
              </div>
              <div>
                <span className="text-sm font-medium text-fg block">Custom Comparison</span>
                <span className="text-[11px] text-fg/45">
                  Test similarity between any two texts
                </span>
              </div>
            </div>

            <div className="space-y-3">
              <div>
                <label className="text-[10px] uppercase tracking-wider text-fg/40 block mb-1.5">
                  First Text
                </label>
                <textarea
                  value={customTextA}
                  onChange={(e) => setCustomTextA(e.target.value)}
                  placeholder="Enter the first text to compare..."
                  rows={2}
                  className={cn(
                    "w-full rounded-lg border border-fg/10 bg-surface-el/30",
                    "px-3 py-2 text-sm text-fg placeholder-fg/30",
                    "focus:border-fg/20 focus:outline-none resize-none",
                  )}
                />
              </div>
              <div>
                <label className="text-[10px] uppercase tracking-wider text-fg/40 block mb-1.5">
                  Second Text
                </label>
                <textarea
                  value={customTextB}
                  onChange={(e) => setCustomTextB(e.target.value)}
                  placeholder="Enter the second text to compare..."
                  rows={2}
                  className={cn(
                    "w-full rounded-lg border border-fg/10 bg-surface-el/30",
                    "px-3 py-2 text-sm text-fg placeholder-fg/30",
                    "focus:border-fg/20 focus:outline-none resize-none",
                  )}
                />
              </div>

              <button
                onClick={compareCustom}
                disabled={comparingCustom || !customTextA.trim() || !customTextB.trim()}
                className={cn(
                  "w-full flex items-center justify-center gap-2 rounded-lg py-2.5 text-sm font-medium",
                  "border border-info/30 bg-info/10 text-info",
                  interactive.transition.fast,
                  "hover:bg-info/20 disabled:opacity-50 disabled:cursor-not-allowed",
                )}
              >
                {comparingCustom ? (
                  <>
                    <Loader2 className="h-4 w-4 animate-spin" />
                    Comparing...
                  </>
                ) : (
                  <>
                    <Search className="h-4 w-4" />
                    Compare Texts
                  </>
                )}
              </button>

              {customError && (
                <div className="rounded-lg border border-danger/20 bg-danger/10 px-3 py-2 text-sm text-danger/80">
                  {customError}
                </div>
              )}

              {customScore !== null && (
                <motion.div
                  initial={{ opacity: 0, scale: 0.95 }}
                  animate={{ opacity: 1, scale: 1 }}
                  className="rounded-lg border border-fg/10 bg-surface-el/30 p-4"
                >
                  <div className="text-center mb-3">
                    <span className="text-3xl font-bold text-fg">
                      {Math.round(customScore * 100)}%
                    </span>
                    <span className="text-sm text-fg/50 ml-1">similarity</span>
                  </div>
                  <SimilarityBar score={customScore} passed={customScore > 0.5} />
                  <p className="text-xs text-fg/40 text-center mt-3">
                    {customScore > 0.7
                      ? "Very high similarity - texts are semantically very similar"
                      : customScore > 0.5
                        ? "Moderate similarity - texts share some common meaning"
                        : customScore > 0.3
                          ? "Low similarity - texts have limited overlap"
                          : "Very low similarity - texts appear unrelated"}
                  </p>
                </motion.div>
              )}
            </div>
          </motion.div>
        </div>
      </main>
    </div>
  );
}
