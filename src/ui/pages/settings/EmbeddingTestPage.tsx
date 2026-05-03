import { useState, useEffect, useMemo } from "react";
import { motion, AnimatePresence } from "framer-motion";
import {
  CheckCircle,
  XCircle,
  Loader2,
  RefreshCw,
  AlertTriangle,
  Search,
  Cpu,
  Activity,
  Target,
  Sparkles,
  Beaker,
  Eye,
  EyeOff,
} from "lucide-react";
import { storageBridge } from "../../../core/storage/files";
import { cn, interactive } from "../../design-tokens";
import { listen } from "@tauri-apps/api/event";
import { useI18n } from "../../../core/i18n/context";

type TestStatus = "idle" | "testing" | "passed" | "failed";

interface ModelInfo {
  version: string;
  maxTokens: number;
  embeddingDimensions: number;
}

interface HealthCheck {
  loadOk: boolean;
  identityCosine: number;
  passed: boolean;
}

interface RetrievalCaseResult {
  name: string;
  query: string;
  expectedId: string;
  expectedText: string;
  rank: number;
  correct: boolean;
  topId: string;
  topText: string;
  topScore: number;
  correctScore: number;
}

interface RetrievalCheck {
  caseCount: number;
  top1Rate: number;
  top3Rate: number;
  mrr: number;
  passed: boolean;
  cases: RetrievalCaseResult[];
}

interface NamedScore {
  name: string;
  textA: string;
  textB: string;
  score: number;
}

interface SeparationCheck {
  relatedAvg: number;
  unrelatedAvg: number;
  margin: number;
  passed: boolean;
  relatedPairs: NamedScore[];
  unrelatedPairs: NamedScore[];
}

interface TestResults {
  success: boolean;
  message: string;
  modelInfo: ModelInfo;
  health: HealthCheck;
  retrieval: RetrievalCheck;
  separation: SeparationCheck;
}

interface BenchmarkVariant {
  version: string;
  label: string;
  embeddingDimensions: number;
  sampleCount: number;
  averageMs: number;
  p95Ms: number;
  minMs: number;
  maxMs: number;
  relatedAverage: number;
  unrelatedAverage: number;
  separationMargin: number;
  retrievalTop1: number;
  retrievalMrr: number;
  pairScores: Array<{ pairName: string; category: string; similarity: number }>;
}

interface DevBenchmarkResults {
  maxTokensUsed: number;
  configuredV4Dimensions: number;
  v3: BenchmarkVariant;
  v4: BenchmarkVariant;
  averageSpeedupV4VsV3: number;
}

type TabKey = "retrieval" | "separation" | "compare" | "benchmark";
type RetrievalFilter = "all" | "failed" | "passed";

function pct(value: number) {
  return `${Math.round(value * 100)}%`;
}

function customVerdict(score: number) {
  if (score >= 0.6) return "very high similarity, likely near-duplicate";
  if (score >= 0.4) return "similar, paraphrase or same scene";
  if (score >= 0.25) return "loosely related, same general domain";
  if (score >= 0.1) return "weakly related";
  return "unrelated";
}

function simpleVerdict(score: number) {
  if (score >= 0.6) return { label: "Same meaning", tone: "accent" as const };
  if (score >= 0.4) return { label: "Similar", tone: "accent" as const };
  if (score >= 0.25) return { label: "Loosely related", tone: "neutral" as const };
  if (score >= 0.1) return { label: "Weakly related", tone: "neutral" as const };
  return { label: "Unrelated", tone: "muted" as const };
}

function pairVerdict(score: number, kind: "related" | "unrelated") {
  const good = kind === "related" ? score >= 0.3 : score < 0.3;
  if (kind === "related") {
    return {
      label: good ? "Recognised as similar" : "Not strongly linked",
      good,
    };
  }
  return {
    label: good ? "Recognised as different" : "Too close",
    good,
  };
}

function separationLabel(margin: number) {
  if (margin >= 0.25) return "Strong";
  if (margin >= 0.15) return "Healthy";
  if (margin >= 0.1) return "OK";
  if (margin >= 0.05) return "Weak";
  return "Poor";
}

function HeaderBanner({
  status,
  results,
  error,
  onRetry,
}: {
  status: TestStatus;
  results: TestResults | null;
  error: string | null;
  onRetry: () => void;
}) {
  const passing =
    status === "passed" || (results !== null && results.success && status !== "testing");

  return (
    <div
      className={cn(
        "rounded-2xl border p-4 sm:p-5",
        status === "testing" && "border-info/25 bg-info/5",
        status === "passed" && "border-accent/25 bg-accent/5",
        status === "failed" && "border-danger/30 bg-danger/5",
        status === "idle" && "border-fg/10 bg-fg/5",
      )}
    >
      <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:gap-5">
        <div
          className={cn(
            "flex h-12 w-12 shrink-0 items-center justify-center rounded-2xl border",
            status === "testing" && "border-info/30 bg-info/10",
            status === "passed" && "border-accent/30 bg-accent/15",
            status === "failed" && "border-danger/30 bg-danger/15",
            status === "idle" && "border-fg/10 bg-fg/10",
          )}
        >
          {status === "testing" && <Loader2 className="h-6 w-6 text-info animate-spin" />}
          {status === "passed" && <CheckCircle className="h-6 w-6 text-accent/90" />}
          {status === "failed" && <XCircle className="h-6 w-6 text-danger/90" />}
          {status === "idle" && <Beaker className="h-6 w-6 text-fg/60" />}
        </div>

        <div className="flex-1 min-w-0">
          <div className="flex flex-wrap items-center gap-2">
            <h2 className="text-base font-semibold text-fg">
              {status === "testing" && "Running checks..."}
              {status === "passed" && "Model healthy"}
              {status === "failed" && (error ? "Test error" : "Issues found")}
              {status === "idle" && "Embedding tests"}
            </h2>
            {results?.modelInfo && (
              <>
                <Pill>{results.modelInfo.version}</Pill>
                <Pill>{results.modelInfo.embeddingDimensions}d</Pill>
                <Pill>{results.modelInfo.maxTokens.toLocaleString()} tok</Pill>
              </>
            )}
          </div>
          <p className="mt-1 text-xs text-fg/55 sm:text-sm">
            {status === "testing" && "Three checks: identity, retrieval, separation."}
            {passing &&
              "Identity stable. Retrieval ranks correctly. Related and unrelated text separate cleanly."}
            {status === "failed" && (error ?? results?.message ?? "Some checks did not pass.")}
            {status === "idle" && "Tap retry to start."}
          </p>
        </div>

        <button
          onClick={onRetry}
          disabled={status === "testing"}
          className={cn(
            "inline-flex items-center justify-center gap-2 rounded-xl border px-3 py-2 text-xs font-medium",
            "border-fg/15 bg-fg/5 text-fg",
            interactive.transition.fast,
            "hover:bg-fg/10 disabled:opacity-50 disabled:cursor-not-allowed sm:self-center",
          )}
        >
          <RefreshCw className={cn("h-3.5 w-3.5", status === "testing" && "animate-spin")} />
          Retry
        </button>
      </div>
    </div>
  );
}

function Pill({ children }: { children: React.ReactNode }) {
  return (
    <span className="inline-flex items-center rounded-full border border-fg/10 bg-fg/5 px-2 py-0.5 text-[10px] font-medium uppercase tracking-wider text-fg/60">
      {children}
    </span>
  );
}

function StatTile({
  icon,
  label,
  value,
  caption,
  passed,
  loading,
}: {
  icon: React.ReactNode;
  label: string;
  value: string;
  caption?: string;
  passed?: boolean;
  loading?: boolean;
}) {
  const tone =
    passed === undefined
      ? "border-fg/10 bg-fg/5"
      : passed
        ? "border-accent/20 bg-accent/5"
        : "border-danger/25 bg-danger/5";

  const valueTone =
    passed === undefined ? "text-fg" : passed ? "text-accent" : "text-danger/80";

  return (
    <div className={cn("rounded-2xl border p-4", tone)}>
      <div className="flex items-center justify-between">
        <span className="inline-flex items-center gap-1.5 text-[10px] font-semibold uppercase tracking-[0.2em] text-fg/45">
          <span className={passed === undefined ? "text-fg/50" : valueTone}>{icon}</span>
          {label}
        </span>
        {passed !== undefined &&
          (passed ? (
            <CheckCircle className="h-3.5 w-3.5 text-accent/80" />
          ) : (
            <XCircle className="h-3.5 w-3.5 text-danger/80" />
          ))}
      </div>
      <div className={cn("mt-3 text-2xl font-semibold tabular-nums", valueTone)}>
        {loading ? <Loader2 className="h-6 w-6 animate-spin text-fg/40" /> : value}
      </div>
      {caption && <div className="mt-1 text-[11px] text-fg/45">{caption}</div>}
    </div>
  );
}

function RetrievalPanel({
  retrieval,
  technical,
}: {
  retrieval: RetrievalCheck;
  technical: boolean;
}) {
  const [filter, setFilter] = useState<RetrievalFilter>("all");
  const [showAll, setShowAll] = useState(false);

  const filteredAll = useMemo(() => {
    if (filter === "failed") return retrieval.cases.filter((c) => !c.correct);
    if (filter === "passed") return retrieval.cases.filter((c) => c.correct);
    return retrieval.cases;
  }, [retrieval.cases, filter]);

  const failedCount = retrieval.cases.filter((c) => !c.correct).length;
  const passedCount = retrieval.cases.length - failedCount;

  const COLLAPSED_LIMIT = 12;
  const filtered = showAll ? filteredAll : filteredAll.slice(0, COLLAPSED_LIMIT);
  const hiddenCount = filteredAll.length - filtered.length;

  return (
    <div className="space-y-4">
      <div className="grid grid-cols-3 gap-2 sm:gap-3">
        <MiniStat
          label={technical ? "Top-1" : "Found first"}
          value={pct(retrieval.top1Rate)}
          highlight={retrieval.passed}
        />
        <MiniStat
          label={technical ? "Top-3" : "Found in top 3"}
          value={pct(retrieval.top3Rate)}
        />
        <MiniStat
          label={technical ? "MRR" : "Average rank"}
          value={
            technical
              ? retrieval.mrr.toFixed(2)
              : retrieval.mrr > 0
                ? `#${(1 / retrieval.mrr).toFixed(1)}`
                : "n/a"
          }
        />
      </div>

      <div className="flex flex-wrap items-center justify-between gap-2">
        <FilterChips
          options={[
            { key: "all", label: `All ${retrieval.cases.length}` },
            { key: "failed", label: `Failed ${failedCount}`, accent: failedCount > 0 ? "danger" : undefined },
            { key: "passed", label: `Passed ${passedCount}`, accent: "accent" },
          ]}
          active={filter}
          onChange={(k) => {
            setFilter(k as RetrievalFilter);
            setShowAll(false);
          }}
        />
        <span className="text-[11px] text-fg/40">
          {filteredAll.length} of {retrieval.cases.length}
        </span>
      </div>

      {filteredAll.length === 0 ? (
        <div className="rounded-xl border border-fg/10 bg-fg/5 p-8 text-center text-xs text-fg/45">
          Nothing in this filter.
        </div>
      ) : (
        <div className="grid gap-2 md:grid-cols-2">
          {filtered.map((c) => (
            <RetrievalCaseCard key={c.name} c={c} technical={technical} />
          ))}
        </div>
      )}

      {hiddenCount > 0 && (
        <button
          onClick={() => setShowAll(true)}
          className={cn(
            "w-full rounded-xl border border-fg/10 bg-fg/5 px-3 py-2 text-xs font-medium text-fg/70",
            "hover:bg-fg/10 transition-colors",
          )}
        >
          Show {hiddenCount} more
        </button>
      )}
    </div>
  );
}

function RetrievalCaseCard({ c, technical }: { c: RetrievalCaseResult; technical: boolean }) {
  const badgeText = technical
    ? c.rank > 0
      ? `#${c.rank}`
      : "n/a"
    : c.correct
      ? "Found"
      : "Missed";
  return (
    <div
      className={cn(
        "rounded-xl border p-3 space-y-2",
        c.correct ? "border-fg/10 bg-fg/5" : "border-danger/25 bg-danger/5",
      )}
    >
      <div className="flex items-start justify-between gap-2">
        <span className="text-xs font-medium text-fg/85 leading-tight">{c.name}</span>
        <span
          className={cn(
            "shrink-0 rounded-full px-2 py-0.5 text-[10px] font-bold uppercase tracking-wider tabular-nums",
            c.correct
              ? "bg-accent/15 text-accent/90"
              : "bg-danger/15 text-danger/90",
          )}
        >
          {badgeText}
        </span>
      </div>
      <div className="text-[11px] italic text-fg/55">"{c.query}"</div>
      <div className="space-y-1 border-t border-fg/5 pt-2">
        <div className="flex items-baseline gap-1.5 text-[11px]">
          <span className="text-fg/35 shrink-0">{c.correct ? "found:" : "got:"}</span>
          <span className="text-fg/75 line-clamp-2">{c.topText}</span>
          {technical && (
            <span className="ml-auto shrink-0 tabular-nums text-fg/45">
              {c.topScore.toFixed(3)}
            </span>
          )}
        </div>
        {!c.correct && c.expectedText && (
          <div className="flex items-baseline gap-1.5 text-[11px]">
            <span className="shrink-0 text-fg/35">expected:</span>
            <span className="line-clamp-2 text-fg/75">{c.expectedText}</span>
            {technical && (
              <span className="ml-auto shrink-0 tabular-nums text-fg/45">
                {c.correctScore.toFixed(3)}
              </span>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

function SeparationPanel({
  separation,
  technical,
}: {
  separation: SeparationCheck;
  technical: boolean;
}) {
  return (
    <div className="space-y-4">
      {!technical ? (
        <div className="grid grid-cols-3 gap-2 sm:gap-3">
          <MiniStat
            label="Related text"
            value={
              separation.relatedAvg >= 0.3
                ? "Recognised"
                : separation.relatedAvg >= 0.2
                  ? "Weak"
                  : "Missed"
            }
          />
          <MiniStat
            label="Unrelated text"
            value={
              separation.unrelatedAvg < 0.25
                ? "Kept apart"
                : separation.unrelatedAvg < 0.35
                  ? "Close"
                  : "Confused"
            }
          />
          <MiniStat
            label="Overall"
            value={separationLabel(separation.margin)}
            highlight={separation.passed}
          />
        </div>
      ) : (
        <div className="grid grid-cols-3 gap-2 sm:gap-3">
          <MiniStat label="Related" value={separation.relatedAvg.toFixed(3)} />
          <MiniStat label="Unrelated" value={separation.unrelatedAvg.toFixed(3)} />
          <MiniStat
            label="Margin"
            value={separation.margin.toFixed(3)}
            highlight={separation.passed}
          />
        </div>
      )}

      <div className="grid gap-4 lg:grid-cols-2">
        <PairList
          title="Related pairs"
          pairs={separation.relatedPairs}
          kind="related"
          technical={technical}
        />
        <PairList
          title="Unrelated pairs"
          pairs={separation.unrelatedPairs}
          kind="unrelated"
          technical={technical}
        />
      </div>
    </div>
  );
}

function PairList({
  title,
  pairs,
  kind,
  technical,
}: {
  title: string;
  pairs: NamedScore[];
  kind: "related" | "unrelated";
  technical: boolean;
}) {
  return (
    <div className="space-y-2">
      <div className="px-1 text-[10px] font-semibold uppercase tracking-[0.2em] text-fg/40">
        {title}
      </div>
      <div className="space-y-2">
        {pairs.map((p) => {
          const v = pairVerdict(p.score, kind);
          return (
            <div
              key={`${kind}-${p.name}`}
              className={cn(
                "rounded-xl border p-3",
                v.good ? "border-fg/10 bg-fg/5" : "border-warning/25 bg-warning/5",
              )}
            >
              <div className="flex items-center justify-between gap-2">
                <span className="text-xs font-medium text-fg/85">{p.name}</span>
                {technical ? (
                  <span
                    className={cn(
                      "text-xs font-semibold tabular-nums",
                      v.good ? "text-accent/90" : "text-fg/55",
                    )}
                  >
                    {p.score.toFixed(3)}
                  </span>
                ) : (
                  <span
                    className={cn(
                      "rounded-full px-2 py-0.5 text-[10px] font-semibold uppercase tracking-wider",
                      v.good
                        ? "bg-accent/15 text-accent/90"
                        : "bg-warning/15 text-warning/90",
                    )}
                  >
                    {v.good ? "Pass" : "Borderline"}
                  </span>
                )}
              </div>
              <div className="mt-1 line-clamp-1 text-[11px] text-fg/55">"{p.textA}"</div>
              <div className="line-clamp-1 text-[11px] text-fg/55">"{p.textB}"</div>
              {!technical && (
                <div className="mt-1 text-[10px] text-fg/45">{v.label}</div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}

function ComparePanel({ technical }: { technical: boolean }) {
  const [textA, setTextA] = useState("");
  const [textB, setTextB] = useState("");
  const [score, setScore] = useState<number | null>(null);
  const [loading, setLoading] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  const compare = async () => {
    if (!textA.trim() || !textB.trim()) {
      setErr("Both texts are required.");
      return;
    }
    setLoading(true);
    setErr(null);
    setScore(null);
    try {
      const s = await storageBridge.compareCustomTexts(textA, textB);
      setScore(s);
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_320px]">
      <div className="space-y-3">
        <div>
          <label className="mb-1 block text-[10px] font-semibold uppercase tracking-wider text-fg/40">
            Text A
          </label>
          <textarea
            value={textA}
            onChange={(e) => setTextA(e.target.value)}
            placeholder="First text..."
            rows={4}
            className={cn(
              "w-full resize-none rounded-xl border border-fg/10 bg-surface-el/30 px-3 py-2",
              "text-sm text-fg placeholder-fg/30",
              "focus:border-fg/25 focus:outline-none",
            )}
          />
        </div>
        <div>
          <label className="mb-1 block text-[10px] font-semibold uppercase tracking-wider text-fg/40">
            Text B
          </label>
          <textarea
            value={textB}
            onChange={(e) => setTextB(e.target.value)}
            placeholder="Second text..."
            rows={4}
            className={cn(
              "w-full resize-none rounded-xl border border-fg/10 bg-surface-el/30 px-3 py-2",
              "text-sm text-fg placeholder-fg/30",
              "focus:border-fg/25 focus:outline-none",
            )}
          />
        </div>
        <button
          onClick={compare}
          disabled={loading || !textA.trim() || !textB.trim()}
          className={cn(
            "flex w-full items-center justify-center gap-2 rounded-xl py-2.5 text-sm font-medium",
            "border border-info/30 bg-info/10 text-info",
            "hover:bg-info/20 disabled:opacity-50 disabled:cursor-not-allowed",
            interactive.transition.fast,
          )}
        >
          {loading ? <Loader2 className="h-4 w-4 animate-spin" /> : <Search className="h-4 w-4" />}
          {loading ? "Comparing..." : "Compare"}
        </button>
      </div>

      <div className="rounded-2xl border border-fg/10 bg-fg/5 p-4">
        <div className="text-[10px] font-semibold uppercase tracking-[0.2em] text-fg/40">
          Result
        </div>
        {err && (
          <div className="mt-3 rounded-lg border border-danger/25 bg-danger/10 p-3 text-xs text-danger/85">
            {err}
          </div>
        )}
        {score === null && !err && (
          <div className="mt-6 text-center text-xs text-fg/40">
            Enter two texts and tap Compare.
          </div>
        )}
        {score !== null && (
          <div className="mt-3 space-y-3">
            {(() => {
              const v = simpleVerdict(score);
              return (
                <div className="text-center">
                  <div
                    className={cn(
                      "text-2xl font-semibold leading-tight",
                      v.tone === "accent" && "text-accent",
                      v.tone === "neutral" && "text-fg",
                      v.tone === "muted" && "text-fg/60",
                    )}
                  >
                    {v.label}
                  </div>
                  {technical && (
                    <div className="mt-1 text-[11px] uppercase tracking-wider text-fg/40 tabular-nums">
                      cosine {score.toFixed(3)}
                    </div>
                  )}
                </div>
              );
            })()}
            {technical && (
              <div className="rounded-lg border border-fg/10 bg-surface-el/30 p-3 text-center text-xs text-fg/65">
                {customVerdict(score)}
              </div>
            )}
            <div className="text-[10px] leading-relaxed text-fg/40">
              {technical
                ? "v4 is retrieval-tuned. Paraphrases land around 0.40, near-duplicates above 0.60, unrelated text below 0.25."
                : "We compare meaning, not exact wording. Two sentences that say the same thing differently still count as similar."}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

function BenchmarkPanel() {
  const [status, setStatus] = useState<"idle" | "running" | "done" | "failed">("idle");
  const [results, setResults] = useState<DevBenchmarkResults | null>(null);
  const [err, setErr] = useState<string | null>(null);

  const run = async () => {
    setStatus("running");
    setErr(null);
    setResults(null);
    try {
      const r = await storageBridge.runEmbeddingDevBenchmark();
      setResults(r);
      setStatus("done");
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
      setStatus("failed");
    }
  };

  return (
    <div className="space-y-4">
      <div className="flex flex-col gap-3 rounded-2xl border border-info/25 bg-info/5 p-4 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <h3 className="text-sm font-semibold text-info">v3 vs v4 benchmark</h3>
          <p className="mt-1 text-xs text-info/70">
            Compare latency, separation, and retrieval quality side by side.
          </p>
        </div>
        <button
          onClick={run}
          disabled={status === "running"}
          className={cn(
            "inline-flex items-center justify-center gap-2 rounded-xl border px-3 py-2 text-xs font-medium",
            status === "running"
              ? "cursor-not-allowed border-info/20 bg-info/10 text-info/60"
              : "border-info/40 bg-info/15 text-info hover:bg-info/25",
          )}
        >
          {status === "running" ? (
            <>
              <Loader2 className="h-3.5 w-3.5 animate-spin" />
              Running...
            </>
          ) : (
            <>
              <Cpu className="h-3.5 w-3.5" />
              Run benchmark
            </>
          )}
        </button>
      </div>

      {err && (
        <div className="rounded-xl border border-danger/25 bg-danger/10 p-3 text-xs text-danger/85">
          {err}
        </div>
      )}

      {results && (
        <>
          <div className="rounded-xl border border-info/20 bg-info/5 p-3 text-xs text-info/80">
            v4 is benchmarked at the configured Matryoshka size:{" "}
            <span className="font-semibold text-info">{results.configuredV4Dimensions}d</span>.
            Speed ratio (v4 vs v3):{" "}
            <span className="font-semibold text-info">
              {results.averageSpeedupV4VsV3.toFixed(2)}x
            </span>
            .
          </div>

          <div className="grid gap-3 md:grid-cols-2">
            {[results.v3, results.v4].map((v) => (
              <BenchmarkVariantCard key={v.label} v={v} />
            ))}
          </div>
        </>
      )}
    </div>
  );
}

function BenchmarkVariantCard({ v }: { v: BenchmarkVariant }) {
  return (
    <div className="rounded-xl border border-fg/10 bg-fg/5 p-4 space-y-3">
      <div className="flex items-center justify-between">
        <div className="text-sm font-semibold text-fg">{v.label}</div>
        <Pill>{v.embeddingDimensions}d</Pill>
      </div>
      <div className="grid grid-cols-2 gap-2">
        <MiniStat label="Avg ms" value={v.averageMs.toFixed(2)} />
        <MiniStat label="P95 ms" value={v.p95Ms.toFixed(2)} />
        <MiniStat label="Top-1" value={pct(v.retrievalTop1)} />
        <MiniStat label="Margin" value={v.separationMargin.toFixed(3)} />
      </div>
      <div className="text-[11px] text-fg/45">
        related {v.relatedAverage.toFixed(3)} · unrelated {v.unrelatedAverage.toFixed(3)} · mrr{" "}
        {v.retrievalMrr.toFixed(2)}
      </div>
    </div>
  );
}

function MiniStat({
  label,
  value,
  highlight,
}: {
  label: string;
  value: string;
  highlight?: boolean;
}) {
  return (
    <div className="rounded-lg border border-fg/10 bg-surface-el/30 px-3 py-2 text-center">
      <div className="text-[10px] uppercase tracking-wider text-fg/40">{label}</div>
      <div
        className={cn(
          "mt-0.5 text-sm font-semibold tabular-nums",
          highlight === undefined ? "text-fg" : highlight ? "text-accent" : "text-fg",
        )}
      >
        {value}
      </div>
    </div>
  );
}

function FilterChips({
  options,
  active,
  onChange,
}: {
  options: { key: string; label: string; accent?: "accent" | "danger" }[];
  active: string;
  onChange: (key: string) => void;
}) {
  return (
    <div className="flex flex-wrap gap-1.5">
      {options.map((o) => {
        const isActive = active === o.key;
        return (
          <button
            key={o.key}
            onClick={() => onChange(o.key)}
            className={cn(
              "rounded-full border px-3 py-1 text-xs font-medium transition-colors",
              isActive
                ? "border-fg/30 bg-fg/15 text-fg"
                : "border-fg/10 bg-fg/5 text-fg/60 hover:bg-fg/10",
            )}
          >
            {o.label}
          </button>
        );
      })}
    </div>
  );
}

function TabButton({
  icon,
  label,
  active,
  onClick,
  badge,
  badgeTone,
}: {
  icon: React.ReactNode;
  label: string;
  active: boolean;
  onClick: () => void;
  badge?: string;
  badgeTone?: "accent" | "danger" | "neutral";
}) {
  return (
    <button
      onClick={onClick}
      className={cn(
        "relative inline-flex shrink-0 items-center gap-2 rounded-xl border px-3 py-2 text-xs font-medium transition-colors",
        active
          ? "border-fg/25 bg-fg/10 text-fg"
          : "border-fg/10 bg-fg/5 text-fg/60 hover:bg-fg/10",
      )}
    >
      <span className={active ? "text-fg" : "text-fg/50"}>{icon}</span>
      {label}
      {badge && (
        <span
          className={cn(
            "rounded-full px-1.5 py-0.5 text-[10px] font-semibold tabular-nums",
            badgeTone === "accent" && "bg-accent/15 text-accent/90",
            badgeTone === "danger" && "bg-danger/15 text-danger/90",
            badgeTone === "neutral" && "bg-fg/10 text-fg/70",
          )}
        >
          {badge}
        </span>
      )}
    </button>
  );
}

export function EmbeddingTestPage() {
  const { t: _t } = useI18n();
  const [testStatus, setTestStatus] = useState<TestStatus>("idle");
  const [testResults, setTestResults] = useState<TestResults | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [progress, setProgress] = useState<{ current: number; total: number; stage?: string } | null>(
    null,
  );
  const [tab, setTab] = useState<TabKey>("retrieval");
  const [technical, setTechnical] = useState(false);
  const isDevBuild = import.meta.env.DEV;

  const runTest = async () => {
    setTestStatus("testing");
    setError(null);
    setTestResults(null);
    setProgress({ current: 0, total: 3 });

    try {
      await storageBridge.initializeEmbeddingModel();
      const results = await storageBridge.runEmbeddingTest();
      setTestResults(results);
      setTestStatus(results.success ? "passed" : "failed");
      if (!results.retrieval.passed) setTab("retrieval");
      else if (!results.separation.passed) setTab("separation");
    } catch (err) {
      setTestStatus("failed");
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    const setup = async () => {
      unlisten = await listen<{ current: number; total: number; stage?: string }>(
        "embedding_test_progress",
        (e) => {
          setProgress({
            current: e.payload.current,
            total: e.payload.total,
            stage: e.payload.stage,
          });
        },
      );
    };
    setup();
    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  useEffect(() => {
    runTest();
  }, []);

  const failedRetrievalCount = testResults
    ? testResults.retrieval.cases.filter((c) => !c.correct).length
    : 0;

  const stageLabel = (s?: string) => {
    switch (s) {
      case "health":
        return "Identity probe";
      case "retrieval":
        return "Retrieval cases";
      case "separation":
        return "Separation pairs";
      case "completed":
        return "Done";
      case "starting":
      default:
        return "Starting";
    }
  };

  return (
    <div className="flex min-h-screen flex-col">
      <main className="flex-1 px-4 pb-24 pt-4 sm:pt-6">
        <div className="mx-auto w-full max-w-5xl space-y-5">
          <HeaderBanner
            status={testStatus}
            results={testResults}
            error={error}
            onRetry={runTest}
          />

          {testStatus === "testing" && progress && (
            <div className="rounded-xl border border-fg/10 bg-fg/5 px-4 py-3 text-xs text-fg/60">
              <div className="flex items-center justify-between">
                <span>{stageLabel(progress.stage)}</span>
                <span className="tabular-nums text-fg/45">
                  {progress.current}/{progress.total}
                </span>
              </div>
              <div className="mt-2 h-1 overflow-hidden rounded-full bg-fg/10">
                <motion.div
                  initial={{ width: 0 }}
                  animate={{
                    width: `${
                      progress.total > 0 ? (progress.current / progress.total) * 100 : 0
                    }%`,
                  }}
                  transition={{ duration: 0.3 }}
                  className="h-full bg-info/80"
                />
              </div>
            </div>
          )}

          {/* Technical toggle + explainer */}
          {testResults && (
            <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
              {!technical ? (
                <p className="text-xs text-fg/55 leading-relaxed">
                  Showing plain results. The model uses retrieval-tuned scores that look low on
                  paper but rank correctly — what matters is whether it finds the right memory.
                </p>
              ) : (
                <p className="text-xs text-fg/55 leading-relaxed">
                  Showing raw scores. Values are cosine similarities from a hard-negative-trained
                  retrieval model and are compressed by design.
                </p>
              )}
              <button
                onClick={() => setTechnical((v) => !v)}
                className={cn(
                  "inline-flex shrink-0 items-center gap-1.5 rounded-full border px-3 py-1 text-[11px] font-medium transition-colors",
                  technical
                    ? "border-fg/25 bg-fg/10 text-fg"
                    : "border-fg/10 bg-fg/5 text-fg/60 hover:bg-fg/10",
                )}
              >
                {technical ? <EyeOff className="h-3 w-3" /> : <Eye className="h-3 w-3" />}
                {technical ? "Hide raw scores" : "Show raw scores"}
              </button>
            </div>
          )}

          {/* 3 stat tiles */}
          {testResults && (
            <div className="grid gap-3 sm:grid-cols-3">
              <StatTile
                icon={<Activity className="h-3.5 w-3.5" />}
                label="Stability"
                value={
                  technical
                    ? testResults.health.identityCosine.toFixed(4)
                    : testResults.health.passed
                      ? "Stable"
                      : "Unstable"
                }
                caption={
                  technical
                    ? "identity cosine, floor 0.9990"
                    : "the model returns the same vector for the same text"
                }
                passed={testResults.health.passed}
              />
              <StatTile
                icon={<Target className="h-3.5 w-3.5" />}
                label="Memory finding"
                value={pct(testResults.retrieval.top1Rate)}
                caption={
                  technical
                    ? `top-1 over ${testResults.retrieval.caseCount} cases · floor ${pct(0.6)}`
                    : `correct memory found first across ${testResults.retrieval.caseCount} test queries`
                }
                passed={testResults.retrieval.passed}
              />
              <StatTile
                icon={<Sparkles className="h-3.5 w-3.5" />}
                label="Telling apart"
                value={
                  technical
                    ? testResults.separation.margin.toFixed(3)
                    : separationLabel(testResults.separation.margin)
                }
                caption={
                  technical
                    ? "related minus unrelated · floor 0.10"
                    : "how well related and unrelated text are kept apart"
                }
                passed={testResults.separation.passed}
              />
            </div>
          )}

          {testStatus === "failed" && !error && testResults && (
            (() => {
              const failingChecks = [
                !testResults.health.passed,
                !testResults.retrieval.passed,
                !testResults.separation.passed,
              ].filter(Boolean).length;
              if (failingChecks < 2) return null;
              return (
                <div className="flex items-start gap-3 rounded-xl border border-danger/30 bg-danger/10 p-3">
                  <AlertTriangle className="h-5 w-5 shrink-0 text-danger/85" />
                  <div className="min-w-0">
                    <p className="text-sm font-medium text-danger/85">Model may be corrupted</p>
                    <p className="mt-0.5 text-xs text-danger/75">
                      Two or more checks failed. Reinstalling the model is the fastest fix.
                    </p>
                  </div>
                </div>
              );
            })()
          )}

          {/* Tabs */}
          {testResults && (
            <>
              <div className="-mx-4 overflow-x-auto px-4 sm:mx-0 sm:px-0">
                <div className="flex gap-2">
                  <TabButton
                    icon={<Target className="h-3.5 w-3.5" />}
                    label="Retrieval"
                    active={tab === "retrieval"}
                    onClick={() => setTab("retrieval")}
                    badge={
                      failedRetrievalCount > 0 ? `${failedRetrievalCount} failed` : undefined
                    }
                    badgeTone="danger"
                  />
                  <TabButton
                    icon={<Sparkles className="h-3.5 w-3.5" />}
                    label="Separation"
                    active={tab === "separation"}
                    onClick={() => setTab("separation")}
                  />
                  <TabButton
                    icon={<Search className="h-3.5 w-3.5" />}
                    label="Compare"
                    active={tab === "compare"}
                    onClick={() => setTab("compare")}
                  />
                  {isDevBuild && (
                    <TabButton
                      icon={<Cpu className="h-3.5 w-3.5" />}
                      label="Benchmark"
                      active={tab === "benchmark"}
                      onClick={() => setTab("benchmark")}
                    />
                  )}
                </div>
              </div>

              <AnimatePresence mode="wait">
                <motion.div
                  key={tab}
                  initial={{ opacity: 0, y: 8 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: -4 }}
                  transition={{ duration: 0.15 }}
                >
                  {tab === "retrieval" && (
                    <RetrievalPanel retrieval={testResults.retrieval} technical={technical} />
                  )}
                  {tab === "separation" && (
                    <SeparationPanel separation={testResults.separation} technical={technical} />
                  )}
                  {tab === "compare" && <ComparePanel technical={technical} />}
                  {tab === "benchmark" && isDevBuild && <BenchmarkPanel />}
                </motion.div>
              </AnimatePresence>
            </>
          )}

          {/* Compare and benchmark accessible even before tests finish */}
          {!testResults && testStatus !== "testing" && (
            <ComparePanel technical={technical} />
          )}
        </div>
      </main>
    </div>
  );
}
