import { useEffect, useMemo, useRef, useState } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";
import { AnimatePresence, motion } from "framer-motion";
import {
  AlertTriangle,
  ArrowRight,
  Check,
  CheckCircle2,
  ChevronRight,
  Loader2,
  Plus,
  RefreshCw,
  Trash2,
  Upload,
  User,
  UserCircle,
  Wand2,
  X,
} from "lucide-react";

import { BottomMenu } from "../../components/BottomMenu";
import { AvatarImage } from "../../components/AvatarImage";
import { useAvatar } from "../../hooks/useAvatar";
import { isRenderableImageUrl } from "../../../core/utils/image";
import {
  CharacterSelectorSingle,
  PersonaSelector,
} from "../group-chats/components/settings";
import { animations, cn, interactive } from "../../design-tokens";

import {
  detectFileKind,
  lorebookGenApproveOutline,
  lorebookGenApplyCoherence,
  lorebookGenCommit,
  lorebookGenCreate,
  lorebookGenDefaultTargetCount,
  lorebookGenDraftNext,
  lorebookGenEditDraft,
  lorebookGenRefineEntry,
  lorebookGenRunCoherence,
  lorebookGenRunPlanner,
  lorebookGenSetDraftApproved,
  readFileAsBase64,
  type LorebookGeneratorEntryPlan,
  type LorebookGeneratorJobState,
  type LorebookGeneratorSourceInput,
} from "../../../core/lorebook/generator";
import { listCharacters, listLorebooks, listPersonas } from "../../../core/storage/repo";
import type { Character, Lorebook, Persona } from "../../../core/storage/schemas";

const MIN_TARGET = 5;
const MAX_TARGET = 50;
const FILE_LIMIT_BYTES = 50 * 1024 * 1024;

type PageStage =
  | "brief"
  | "planning"
  | "outline"
  | "drafting"
  | "coherence"
  | "commit"
  | "done";

interface PendingFile {
  name: string;
  kind: "txt" | "md" | "pdf";
  size: number;
  dataBase64: string;
}

interface PendingTextSource {
  id: string;
  label: string;
  body: string;
}

export function LorebookGeneratorFlowPage() {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const initialName = searchParams.get("name") ?? "";

  const [pageStage, setPageStage] = useState<PageStage>("brief");
  const [job, setJob] = useState<LorebookGeneratorJobState | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const [brief, setBrief] = useState("");
  const [textSources, setTextSources] = useState<PendingTextSource[]>([]);
  const [files, setFiles] = useState<PendingFile[]>([]);
  const [targetCount, setTargetCount] = useState<number>(12);
  const [lorebookName, setLorebookName] = useState<string>(initialName);
  const [characters, setCharacters] = useState<Character[]>([]);
  const [personas, setPersonas] = useState<Persona[]>([]);
  const [characterId, setCharacterId] = useState<string>("");
  const [personaId, setPersonaId] = useState<string>("");
  const fileInputRef = useRef<HTMLInputElement | null>(null);

  const [refineFor, setRefineFor] = useState<number | null>(null);
  const [refineText, setRefineText] = useState("");

  const [editFor, setEditFor] = useState<number | null>(null);

  const [expandedDrafts, setExpandedDrafts] = useState<Set<number>>(new Set());
  const toggleDraftExpanded = (idx: number) => {
    setExpandedDrafts((prev) => {
      const next = new Set(prev);
      if (next.has(idx)) next.delete(idx);
      else next.add(idx);
      return next;
    });
  };
  const collapseDraft = (idx: number) => {
    setExpandedDrafts((prev) => {
      if (!prev.has(idx)) return prev;
      const next = new Set(prev);
      next.delete(idx);
      return next;
    });
  };
  const expandDraft = (idx: number) => {
    setExpandedDrafts((prev) => {
      if (prev.has(idx)) return prev;
      const next = new Set(prev);
      next.add(idx);
      return next;
    });
  };

  const [acceptedChangeIds, setAcceptedChangeIds] = useState<Set<string>>(new Set());

  const [existingLorebooks, setExistingLorebooks] = useState<Lorebook[]>([]);
  const [commitTarget, setCommitTarget] = useState<string>("");

  useEffect(() => {
    void lorebookGenDefaultTargetCount()
      .then((n) => setTargetCount(Math.min(MAX_TARGET, Math.max(MIN_TARGET, n))))
      .catch(() => undefined);
    void listCharacters().then(setCharacters).catch(() => undefined);
    void listPersonas().then(setPersonas).catch(() => undefined);
  }, []);

  useEffect(() => {
    if (!job) return;
    setExpandedDrafts((prev) => {
      let changed = false;
      const next = new Set(prev);
      for (const d of job.drafts) {
        if (d.status === "drafted" && !prev.has(d.planIdx) && d.revisions.length === 0) {
          next.add(d.planIdx);
          changed = true;
        }
        if (d.status === "approved" && prev.has(d.planIdx)) {
          next.delete(d.planIdx);
          changed = true;
        }
      }
      return changed ? next : prev;
    });
  }, [job?.drafts]);

  const totalSize = useMemo(() => {
    return (
      textSources.reduce((s, t) => s + t.body.length, 0) +
      files.reduce((s, f) => s + f.size, 0)
    );
  }, [textSources, files]);

  const canStart = brief.trim().length > 0 && lorebookName.trim().length > 0 && !busy;

  const addTextSource = () => {
    setTextSources((prev) => [
      ...prev,
      { id: `t_${Date.now()}_${prev.length}`, label: `Text ${prev.length + 1}`, body: "" },
    ]);
  };

  const updateTextSource = (id: string, patch: Partial<PendingTextSource>) => {
    setTextSources((prev) => prev.map((t) => (t.id === id ? { ...t, ...patch } : t)));
  };

  const removeTextSource = (id: string) => {
    setTextSources((prev) => prev.filter((t) => t.id !== id));
  };

  const handleFileChange = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const list = e.target.files;
    if (!list || list.length === 0) return;
    const next: PendingFile[] = [...files];
    for (let i = 0; i < list.length; i++) {
      const f = list.item(i);
      if (!f) continue;
      const kind = detectFileKind(f.name);
      if (!kind) {
        setError(`Unsupported file type: ${f.name}. Use .txt, .md, or .pdf.`);
        continue;
      }
      if (f.size > FILE_LIMIT_BYTES) {
        setError(`File "${f.name}" exceeds 50 MB limit.`);
        continue;
      }
      try {
        const b64 = await readFileAsBase64(f);
        next.push({ name: f.name, kind, size: f.size, dataBase64: b64 });
      } catch (err) {
        setError(`Failed to read ${f.name}: ${err instanceof Error ? err.message : String(err)}`);
      }
    }
    setFiles(next);
    e.target.value = "";
  };

  const removeFile = (name: string) => {
    setFiles((prev) => prev.filter((f) => f.name !== name));
  };

  const startGeneration = async () => {
    setError(null);
    setBusy(true);
    try {
      const sources: LorebookGeneratorSourceInput[] = [];
      const character = characters.find((c) => c.id === characterId);
      if (character) {
        const parts: string[] = [];
        if (character.description) parts.push(character.description);
        if (character.definition) parts.push(character.definition);
        const body = parts.join("\n\n").trim();
        if (body) {
          sources.push({
            type: "text",
            label: `Character: ${character.name}`,
            body,
          });
        }
      }
      const persona = personas.find((p) => p.id === personaId);
      if (persona) {
        const body = (persona.description ?? "").trim();
        if (body) {
          sources.push({
            type: "text",
            label: `Persona: ${persona.title}`,
            body,
          });
        }
      }
      sources.push(
        ...textSources
          .filter((t) => t.body.trim().length > 0)
          .map<LorebookGeneratorSourceInput>((t) => ({
            type: "text",
            label: t.label || "Pasted text",
            body: t.body,
          })),
      );
      sources.push(
        ...files.map<LorebookGeneratorSourceInput>((f) => ({
          type: "file",
          name: f.name,
          dataBase64: f.dataBase64,
          kind: f.kind,
        })),
      );
      const created = await lorebookGenCreate({
        brief: brief.trim(),
        sources,
        targetCount,
        initialLorebookName: lorebookName.trim() || null,
      });
      setJob(created);
      setPageStage("planning");
      const planned = await lorebookGenRunPlanner(created.id);
      setJob(planned);
      setPageStage("outline");
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setPageStage("brief");
    } finally {
      setBusy(false);
    }
  };

  const updateOutlineEntry = (idx: number, patch: Partial<LorebookGeneratorEntryPlan>) => {
    if (!job) return;
    const next = job.outline.map((p, i) => (i === idx ? { ...p, ...patch } : p));
    setJob({ ...job, outline: next });
  };

  const removeOutlineEntry = (idx: number) => {
    if (!job) return;
    setJob({ ...job, outline: job.outline.filter((_, i) => i !== idx) });
  };

  const addOutlineEntry = () => {
    if (!job) return;
    setJob({
      ...job,
      outline: [
        ...job.outline,
        {
          idx: job.outline.length,
          title: "New entry",
          category: "other",
          proposedKeys: [],
          rationale: "",
          sourceRefs: [],
        },
      ],
    });
  };

  const approveOutline = async () => {
    if (!job) return;
    setError(null);
    setBusy(true);
    try {
      const { lorebookGenUpdateOutline } = await import("../../../core/lorebook/generator");
      const updated = await lorebookGenUpdateOutline({
        jobId: job.id,
        outline: job.outline,
      });
      const approved = await lorebookGenApproveOutline(updated.id);
      setJob(approved);
      setPageStage("drafting");
      let current = approved;
      while (current.drafts.some((d) => d.status === "pending" || d.status === "drafting")) {
        current = await lorebookGenDraftNext(current.id);
        setJob(current);
        if (current.stage === "draftsReady") break;
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleApproveDraft = async (entryIdx: number, approved: boolean) => {
    if (!job) return;
    setBusy(true);
    try {
      const next = await lorebookGenSetDraftApproved({
        jobId: job.id,
        entryIdx,
        approved,
      });
      setJob(next);
      if (approved) {
        collapseDraft(entryIdx);
      } else {
        expandDraft(entryIdx);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleRefine = async () => {
    if (!job || refineFor === null || !refineText.trim()) return;
    setBusy(true);
    setError(null);
    try {
      const next = await lorebookGenRefineEntry({
        jobId: job.id,
        entryIdx: refineFor,
        feedback: refineText.trim(),
      });
      setJob(next);
      expandDraft(refineFor);
      setRefineFor(null);
      setRefineText("");
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleSaveEdit = async (
    entryIdx: number,
    title: string,
    keywords: string[],
    content: string,
    alwaysActive: boolean,
  ) => {
    if (!job) return;
    setBusy(true);
    try {
      const next = await lorebookGenEditDraft({
        jobId: job.id,
        entryIdx,
        title,
        keywords,
        content,
        alwaysActive,
      });
      setJob(next);
      setEditFor(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  const runCoherence = async () => {
    if (!job) return;
    setBusy(true);
    setError(null);
    try {
      const next = await lorebookGenRunCoherence(job.id);
      setJob(next);
      setAcceptedChangeIds(new Set(next.coherenceProposals.map((c) => c.id)));
      setPageStage("coherence");
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  const applyCoherence = async () => {
    if (!job) return;
    setBusy(true);
    try {
      const next = await lorebookGenApplyCoherence({
        jobId: job.id,
        acceptedChangeIds: Array.from(acceptedChangeIds),
      });
      setJob(next);
      setPageStage("drafting");
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  const skipCoherence = () => {
    setPageStage("commit");
    void listLorebooks().then((list) => setExistingLorebooks(list as Lorebook[]));
  };

  const goToCommit = async () => {
    setPageStage("commit");
    try {
      const list = await listLorebooks();
      setExistingLorebooks(list as Lorebook[]);
    } catch {
      // ignore
    }
  };

  const handleCommit = async () => {
    if (!job) return;
    setBusy(true);
    setError(null);
    try {
      const result = await lorebookGenCommit({
        jobId: job.id,
        targetLorebookId: commitTarget || null,
        newName: commitTarget ? null : lorebookName.trim() || null,
      });
      setPageStage("done");
      navigate(`/library/lorebook/${encodeURIComponent(result.lorebookId)}`);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="flex min-h-screen flex-col bg-bg text-fg">
      <StageStepper pageStage={pageStage} />
      <main className="flex-1 px-4 pb-24 pt-4 sm:pt-6">
        <div className="mx-auto w-full max-w-3xl space-y-6">
          {error && (
            <div className="flex items-start gap-2 rounded-xl border border-red-500/30 bg-red-500/10 px-3 py-2 text-sm text-red-200">
              <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0" />
              <div className="flex-1">{error}</div>
              <button
                type="button"
                onClick={() => setError(null)}
                className="text-red-200/70 hover:text-red-100"
              >
                <X className="h-4 w-4" />
              </button>
            </div>
          )}

          <AnimatePresence mode="wait">
          {pageStage === "brief" && (
            <motion.div key="brief" {...animations.fadeIn}>
            <BriefForm
              brief={brief}
              setBrief={setBrief}
              textSources={textSources}
              addTextSource={addTextSource}
              updateTextSource={updateTextSource}
              removeTextSource={removeTextSource}
              files={files}
              fileInputRef={fileInputRef}
              onFileChange={handleFileChange}
              onRemoveFile={removeFile}
              targetCount={targetCount}
              setTargetCount={setTargetCount}
              lorebookName={lorebookName}
              setLorebookName={setLorebookName}
              characters={characters}
              personas={personas}
              characterId={characterId}
              setCharacterId={setCharacterId}
              personaId={personaId}
              setPersonaId={setPersonaId}
              totalSize={totalSize}
              canStart={canStart}
              onStart={() => void startGeneration()}
              busy={busy}
            />
            </motion.div>
          )}

          {pageStage === "planning" && (
            <motion.div
              key="planning"
              {...animations.fadeIn}
              className="flex flex-col items-center gap-4 py-16 text-fg/70"
            >
              <div className="relative">
                <div className="absolute inset-0 rounded-full bg-accent/20 blur-xl" />
                <Loader2 className="relative h-10 w-10 animate-spin text-accent" />
              </div>
              <p className="text-sm">Planning entries…</p>
            </motion.div>
          )}

          {pageStage === "outline" && job && (
            <motion.div key="outline" {...animations.fadeIn}>
            <OutlineReview
              job={job}
              onUpdate={updateOutlineEntry}
              onRemove={removeOutlineEntry}
              onAdd={addOutlineEntry}
              onApprove={() => void approveOutline()}
              onBack={() => setPageStage("brief")}
              busy={busy}
            />
            </motion.div>
          )}

          {pageStage === "drafting" && job && (
            <motion.div key="drafting" {...animations.fadeIn}>
            <DraftReview
              job={job}
              expandedDrafts={expandedDrafts}
              toggleExpanded={toggleDraftExpanded}
              onApprove={(idx, approved) => void handleApproveDraft(idx, approved)}
              onOpenRefine={(idx) => {
                setRefineFor(idx);
                setRefineText("");
              }}
              onOpenEdit={(idx) => setEditFor(idx)}
              onRetryFailed={() => void lorebookGenDraftNext(job.id).then((j) => setJob(j))}
              onRunCoherence={() => void runCoherence()}
              onSkipToCommit={() => void goToCommit()}
              busy={busy}
            />
            </motion.div>
          )}

          {pageStage === "coherence" && job && (
            <motion.div key="coherence" {...animations.fadeIn}>
            <DraftReview
              job={job}
              expandedDrafts={expandedDrafts}
              toggleExpanded={toggleDraftExpanded}
              onApprove={(idx, approved) => void handleApproveDraft(idx, approved)}
              onOpenRefine={(idx) => {
                setRefineFor(idx);
                setRefineText("");
              }}
              onOpenEdit={(idx) => setEditFor(idx)}
              onRetryFailed={() => void lorebookGenDraftNext(job.id).then((j) => setJob(j))}
              onRunCoherence={() => void runCoherence()}
              onSkipToCommit={() => void goToCommit()}
              busy={busy}
              coherenceBanner={
                <CoherenceBanner
                  job={job}
                  accepted={acceptedChangeIds}
                  setAccepted={setAcceptedChangeIds}
                  onApply={() => void applyCoherence()}
                  onSkip={() => skipCoherence()}
                  busy={busy}
                />
              }
            />
            </motion.div>
          )}

          {pageStage === "commit" && job && (
            <motion.div key="commit" {...animations.fadeIn}>
            <CommitForm
              lorebooks={existingLorebooks}
              commitTarget={commitTarget}
              setCommitTarget={setCommitTarget}
              newLorebookName={lorebookName.trim() || initialName.trim() || "Untitled"}
              draftCount={job.drafts.length}
              onCommit={() => void handleCommit()}
              onBack={() => setPageStage("drafting")}
              busy={busy}
            />
            </motion.div>
          )}

          {pageStage === "done" && (
            <motion.div
              key="done"
              {...animations.scaleIn}
              className="flex flex-col items-center gap-4 py-16 text-fg/70"
            >
              <div className="relative">
                <div className="absolute inset-0 rounded-full bg-emerald-500/20 blur-xl" />
                <CheckCircle2 className="relative h-12 w-12 text-emerald-400" />
              </div>
              <p className="text-sm">Lorebook saved.</p>
            </motion.div>
          )}
          </AnimatePresence>
        </div>
      </main>

      <RefineSheet
        isOpen={refineFor !== null && !!job}
        draftTitle={
          job && refineFor !== null
            ? job.drafts.find((d) => d.planIdx === refineFor)?.title ??
              `Entry ${refineFor + 1}`
            : ""
        }
        feedback={refineText}
        setFeedback={setRefineText}
        onClose={() => {
          setRefineFor(null);
          setRefineText("");
        }}
        onSubmit={() => void handleRefine()}
        busy={busy}
      />

      {editFor !== null && job && (
        <EditDraftModal
          initial={job.drafts.find((d) => d.planIdx === editFor)!}
          onCancel={() => setEditFor(null)}
          onSave={(title, keywords, content, alwaysActive) =>
            void handleSaveEdit(editFor, title, keywords, content, alwaysActive)
          }
          busy={busy}
        />
      )}
    </div>
  );
}

const STEPS: Array<{ key: PageStage; label: string; matches: PageStage[] }> = [
  { key: "brief", label: "Brief", matches: ["brief", "planning"] },
  { key: "outline", label: "Outline", matches: ["outline"] },
  { key: "drafting", label: "Drafts", matches: ["drafting", "coherence"] },
  { key: "commit", label: "Save", matches: ["commit", "done"] },
];

function activeStepIndex(s: PageStage): number {
  const i = STEPS.findIndex((step) => step.matches.includes(s));
  return i === -1 ? 0 : i;
}

function StageStepper({ pageStage }: { pageStage: PageStage }) {
  const activeIdx = activeStepIndex(pageStage);
  return (
    <div className="sticky top-0 z-10 border-b border-fg/10 bg-bg/85 px-4 py-3 backdrop-blur-md">
      <div className="mx-auto flex max-w-3xl items-center gap-2 sm:gap-3">
        {STEPS.map((step, i) => {
          const isActive = i === activeIdx;
          const isComplete = i < activeIdx;
          return (
            <div key={step.key} className="flex flex-1 items-center gap-2 sm:gap-3">
              <div className="flex flex-1 items-center gap-2">
                <motion.div
                  layout
                  className={cn(
                    "relative flex h-7 w-7 shrink-0 items-center justify-center rounded-full border text-[11px] font-semibold",
                    interactive.transition.default,
                    isActive
                      ? "border-accent bg-accent/15 text-accent"
                      : isComplete
                        ? "border-emerald-500/50 bg-emerald-500/15 text-emerald-300"
                        : "border-fg/15 bg-fg/5 text-fg/40",
                  )}
                >
                  {isActive && (
                    <motion.span
                      layoutId="stepper-glow"
                      className="absolute inset-0 rounded-full bg-accent/15 blur-sm"
                      transition={{ type: "spring", stiffness: 500, damping: 30 }}
                    />
                  )}
                  <span className="relative">
                    {isComplete ? <Check className="h-3.5 w-3.5" /> : i + 1}
                  </span>
                </motion.div>
                <span
                  className={cn(
                    "hidden truncate text-xs font-medium sm:inline",
                    isActive ? "text-fg" : isComplete ? "text-fg/70" : "text-fg/40",
                  )}
                >
                  {step.label}
                </span>
              </div>
              {i < STEPS.length - 1 && (
                <div className="relative h-px flex-1 bg-fg/10">
                  <motion.div
                    initial={false}
                    animate={{ scaleX: i < activeIdx ? 1 : 0 }}
                    style={{ transformOrigin: "left" }}
                    transition={{ duration: 0.3, ease: [0.25, 0.46, 0.45, 0.94] }}
                    className="absolute inset-0 bg-emerald-500/50"
                  />
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}

function BriefForm({
  brief,
  setBrief,
  textSources,
  addTextSource,
  updateTextSource,
  removeTextSource,
  files,
  fileInputRef,
  onFileChange,
  onRemoveFile,
  targetCount,
  setTargetCount,
  lorebookName,
  setLorebookName,
  characters,
  personas,
  characterId,
  setCharacterId,
  personaId,
  setPersonaId,
  totalSize,
  canStart,
  onStart,
  busy,
}: {
  brief: string;
  setBrief: (v: string) => void;
  textSources: PendingTextSource[];
  addTextSource: () => void;
  updateTextSource: (id: string, patch: Partial<PendingTextSource>) => void;
  removeTextSource: (id: string) => void;
  files: PendingFile[];
  fileInputRef: React.MutableRefObject<HTMLInputElement | null>;
  onFileChange: (e: React.ChangeEvent<HTMLInputElement>) => Promise<void>;
  onRemoveFile: (name: string) => void;
  targetCount: number;
  setTargetCount: (n: number) => void;
  lorebookName: string;
  setLorebookName: (v: string) => void;
  characters: Character[];
  personas: Persona[];
  characterId: string;
  setCharacterId: (v: string) => void;
  personaId: string;
  setPersonaId: (v: string) => void;
  totalSize: number;
  canStart: boolean;
  onStart: () => void;
  busy: boolean;
}) {
  return (
    <div className="space-y-7">
      <Field label="Lorebook name">
        <input
          value={lorebookName}
          onChange={(e) => setLorebookName(e.target.value)}
          placeholder="My Worldbook"
          className={inputClass}
        />
      </Field>

      <Field label="Brief">
        <textarea
          value={brief}
          onChange={(e) => setBrief(e.target.value)}
          placeholder="Describe the world, setting, or topic."
          rows={5}
          className={cn(inputClass, "resize-y")}
        />
      </Field>

      <div className="grid grid-cols-1 gap-5 sm:grid-cols-2">
        <CharacterTrigger
          characters={characters}
          selectedId={characterId}
          onChange={setCharacterId}
        />
        <PersonaTrigger
          personas={personas}
          selectedId={personaId}
          onChange={setPersonaId}
        />
      </div>

      <Field
        label="Pasted text"
        hint={textSources.length > 0 ? `${textSources.length}` : "optional"}
        action={
          <button type="button" onClick={addTextSource} className={pillButtonClass}>
            <Plus className="h-3 w-3" /> Add
          </button>
        }
      >
        {textSources.length === 0 ? (
          <p className="text-xs text-fg/40">
            Reference material to ground entries in.
          </p>
        ) : (
          <div className="space-y-2">
            <AnimatePresence initial={false}>
              {textSources.map((t) => (
                <motion.div
                  key={t.id}
                  layout
                  {...animations.fadeInFast}
                  className="space-y-2"
                >
                  <div className="flex items-center gap-2">
                    <input
                      value={t.label}
                      onChange={(e) => updateTextSource(t.id, { label: e.target.value })}
                      className={cn(inputClass, "px-3 py-1.5 text-xs")}
                    />
                    <button
                      type="button"
                      onClick={() => removeTextSource(t.id)}
                      className="rounded-lg p-1.5 text-fg/40 transition hover:bg-fg/5 hover:text-red-300"
                    >
                      <Trash2 className="h-3.5 w-3.5" />
                    </button>
                  </div>
                  <textarea
                    value={t.body}
                    onChange={(e) => updateTextSource(t.id, { body: e.target.value })}
                    rows={4}
                    placeholder="Paste source text…"
                    className={cn(inputClass, "px-3 py-2 text-xs")}
                  />
                </motion.div>
              ))}
            </AnimatePresence>
          </div>
        )}
      </Field>

      <Field
        label="Files"
        hint={files.length > 0 ? `${files.length}` : ".txt, .md, .pdf"}
        action={
          <>
            <button
              type="button"
              onClick={() => fileInputRef.current?.click()}
              className={pillButtonClass}
            >
              <Upload className="h-3 w-3" /> Upload
            </button>
            <input
              ref={fileInputRef}
              type="file"
              multiple
              accept=".txt,.md,.markdown,.pdf,.text"
              className="hidden"
              onChange={(e) => {
                void onFileChange(e);
              }}
            />
          </>
        }
      >
        {files.length === 0 ? (
          <p className="text-xs text-fg/40">
            Up to 50 MB per file, 200 MB total.
          </p>
        ) : (
          <div className="divide-y divide-fg/5">
            <AnimatePresence initial={false}>
              {files.map((f) => (
                <motion.div
                  key={f.name}
                  layout
                  {...animations.fadeInFast}
                  className="flex items-center justify-between gap-3 py-2 text-xs"
                >
                  <div className="flex min-w-0 items-center gap-2">
                    <span className="shrink-0 rounded bg-fg/10 px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide">
                      {f.kind}
                    </span>
                    <span className="truncate">{f.name}</span>
                    <span className="shrink-0 text-fg/40">
                      {Math.ceil(f.size / 1024)} KB
                    </span>
                  </div>
                  <button
                    type="button"
                    onClick={() => onRemoveFile(f.name)}
                    className="rounded-lg p-1.5 text-fg/40 transition hover:bg-fg/5 hover:text-red-300"
                  >
                    <Trash2 className="h-3.5 w-3.5" />
                  </button>
                </motion.div>
              ))}
            </AnimatePresence>
            <p className="pt-2 text-[11px] text-fg/40">
              Total {(totalSize / 1024).toFixed(0)} KB
            </p>
          </div>
        )}
      </Field>

      <Field label="Target entry count">
        <div className="flex items-center gap-3">
          <input
            type="range"
            min={MIN_TARGET}
            max={MAX_TARGET}
            step={1}
            value={targetCount}
            onChange={(e) => setTargetCount(Number(e.target.value))}
            className="min-w-0 flex-1 accent-accent"
          />
          <input
            type="number"
            min={MIN_TARGET}
            max={MAX_TARGET}
            step={1}
            value={targetCount}
            onChange={(e) => {
              const n = Number(e.target.value);
              if (!Number.isFinite(n)) return;
              setTargetCount(Math.min(MAX_TARGET, Math.max(MIN_TARGET, Math.round(n))));
            }}
            className="w-16 shrink-0 rounded-lg border border-fg/10 bg-transparent px-2 py-1.5 text-center text-sm tabular-nums focus:border-fg/30 focus:outline-none"
          />
        </div>
        <p className="text-[11px] text-fg/40">
          Range {MIN_TARGET}–{MAX_TARGET}.
        </p>
      </Field>

      <motion.button
        type="button"
        onClick={onStart}
        disabled={!canStart}
        whileTap={canStart ? { scale: 0.98 } : undefined}
        className={cn(
          "flex w-full items-center justify-center gap-2 rounded-xl border border-accent/40 bg-accent/10 py-3 text-sm font-semibold text-accent",
          interactive.transition.fast,
          "hover:bg-accent/20 disabled:opacity-40",
        )}
      >
        {busy ? <Loader2 className="h-4 w-4 animate-spin" /> : <Wand2 className="h-4 w-4" />}
        Plan entries
      </motion.button>
    </div>
  );
}

const inputClass =
  "w-full rounded-lg border border-fg/10 bg-transparent px-3 py-2.5 text-sm transition focus:border-fg/30 focus:outline-none";
const pillButtonClass =
  "flex items-center gap-1.5 rounded-md border border-fg/10 px-2.5 py-1 text-[11px] font-medium text-fg/70 transition hover:border-fg/20 hover:text-fg";

function Field({
  label,
  hint,
  action,
  children,
}: {
  label: string;
  hint?: string;
  action?: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <section className="space-y-2">
      <div className="flex items-center gap-2">
        <span className="text-[11px] font-semibold uppercase tracking-[0.18em] text-fg/45">
          {label}
        </span>
        {hint && <span className="text-[10px] text-fg/30">· {hint}</span>}
        {action && <span className="ml-auto">{action}</span>}
      </div>
      {children}
    </section>
  );
}

function OutlineReview({
  job,
  onUpdate,
  onRemove,
  onAdd,
  onApprove,
  onBack,
  busy,
}: {
  job: LorebookGeneratorJobState;
  onUpdate: (idx: number, patch: Partial<LorebookGeneratorEntryPlan>) => void;
  onRemove: (idx: number) => void;
  onAdd: () => void;
  onApprove: () => void;
  onBack: () => void;
  busy: boolean;
}) {
  return (
    <div className="space-y-4">
      <p className="text-sm text-fg/60">
        Review and edit the planned entries. Approve to draft each one (in batches of 3).
      </p>
      {job.outline.map((p, i) => (
        <div key={i} className="space-y-2 rounded-xl border border-fg/10 bg-fg/[0.03] p-3">
          <div className="flex items-center gap-2">
            <input
              value={p.title}
              onChange={(e) => onUpdate(i, { title: e.target.value })}
              className="flex-1 rounded-lg border border-fg/10 bg-surface-el/20 px-2 py-1 text-sm font-medium"
            />
            <select
              value={p.category}
              onChange={(e) => onUpdate(i, { category: e.target.value })}
              className="rounded-lg border border-fg/10 bg-surface-el/20 px-2 py-1 text-xs"
            >
              {[
                "character",
                "location",
                "faction",
                "item",
                "event",
                "concept",
                "rule",
                "other",
              ].map((c) => (
                <option key={c} value={c}>
                  {c}
                </option>
              ))}
            </select>
            <button
              type="button"
              onClick={() => onRemove(i)}
              className="rounded-lg p-1 text-fg/40 hover:bg-fg/5 hover:text-red-300"
            >
              <Trash2 className="h-3.5 w-3.5" />
            </button>
          </div>
          <input
            value={p.proposedKeys.join(", ")}
            onChange={(e) =>
              onUpdate(i, {
                proposedKeys: e.target.value
                  .split(",")
                  .map((s) => s.trim())
                  .filter((s) => s.length > 0),
              })
            }
            placeholder="comma-separated keys"
            className="w-full rounded-lg border border-fg/10 bg-surface-el/20 px-2 py-1 text-xs"
          />
          <textarea
            value={p.rationale}
            onChange={(e) => onUpdate(i, { rationale: e.target.value })}
            rows={2}
            placeholder="rationale"
            className="w-full rounded-lg border border-fg/10 bg-surface-el/20 px-2 py-1 text-xs"
          />
        </div>
      ))}
      <button
        type="button"
        onClick={onAdd}
        className="flex w-full items-center justify-center gap-1 rounded-xl border border-dashed border-fg/15 px-3 py-2 text-xs text-fg/60 hover:bg-fg/5"
      >
        <Plus className="h-3.5 w-3.5" /> Add entry
      </button>

      <div className="flex gap-3 pt-2">
        <button
          type="button"
          onClick={onBack}
          className="flex-1 rounded-xl border border-fg/10 py-3 text-sm hover:bg-fg/5"
        >
          Back
        </button>
        <button
          type="button"
          onClick={onApprove}
          disabled={busy || job.outline.length === 0}
          className="flex flex-1 items-center justify-center gap-2 rounded-xl border border-accent/40 bg-accent/15 py-3 text-sm font-medium text-accent hover:bg-accent/25 disabled:opacity-50"
        >
          {busy ? <Loader2 className="h-4 w-4 animate-spin" /> : <ArrowRight className="h-4 w-4" />}
          Approve & draft
        </button>
      </div>
    </div>
  );
}

function DraftReview({
  job,
  expandedDrafts,
  toggleExpanded,
  onApprove,
  onOpenRefine,
  onOpenEdit,
  onRetryFailed,
  onRunCoherence,
  onSkipToCommit,
  busy,
  coherenceBanner,
}: {
  job: LorebookGeneratorJobState;
  expandedDrafts: Set<number>;
  toggleExpanded: (idx: number) => void;
  onApprove: (idx: number, approved: boolean) => void;
  onOpenRefine: (idx: number) => void;
  onOpenEdit: (idx: number) => void;
  onRetryFailed: () => void;
  onRunCoherence: () => void;
  onSkipToCommit: () => void;
  busy: boolean;
  coherenceBanner?: React.ReactNode;
}) {
  const allDone = job.drafts.every(
    (d) => d.status === "drafted" || d.status === "approved",
  );
  const anyPending = job.drafts.some(
    (d) => d.status === "pending" || d.status === "drafting",
  );
  const anyFailed = job.drafts.some((d) => d.status === "failed");

  return (
    <div className="space-y-4">
      <p className="text-sm text-fg/60">
        Drafts are produced 3 at a time. Approve or ask for changes per entry.
      </p>
      {coherenceBanner}
      <div className="divide-y divide-fg/10">
        <AnimatePresence initial={false}>
        {job.drafts.map((d, index) => {
          const expandable = d.status === "drafted" || d.status === "approved";
          const expanded = expandable && expandedDrafts.has(d.planIdx);
          const inFlight = d.status === "pending" || d.status === "drafting";
          return (
          <motion.div
            key={d.planIdx}
            layout
            initial={{ opacity: 0, y: 8 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -4 }}
            transition={{ delay: index * 0.02, duration: 0.18 }}
            className={cn(
              "space-y-2 py-3 first:pt-0 last:pb-0",
              d.status === "approved" && "border-l-2 border-emerald-500/40 pl-4",
              d.status === "failed" && "border-l-2 border-red-500/30 pl-4",
            )}
          >
          <button
            type="button"
            onClick={() => expandable && toggleExpanded(d.planIdx)}
            disabled={!expandable}
            className={cn(
              "flex w-full items-center gap-2 text-left",
              expandable && "cursor-pointer",
            )}
          >
            {expandable ? (
              <motion.span
                animate={{ rotate: expanded ? 90 : 0 }}
                transition={{ duration: 0.15 }}
                className="text-fg/40"
              >
                <ChevronRight className="h-4 w-4" />
              </motion.span>
            ) : (
              <span className="h-4 w-4 shrink-0" />
            )}
            <span className="truncate text-sm font-semibold">
              {d.title || `Entry ${d.planIdx + 1}`}
            </span>
            <span className="ml-auto whitespace-nowrap text-[11px] text-fg/40">
              {d.alwaysActive ? "always active · " : ""}
              {d.keywords.length} keys
            </span>
            {inFlight && (
              <Loader2 className="h-3.5 w-3.5 shrink-0 animate-spin text-accent" />
            )}
            {statusBadge(d.status)}
          </button>
          {d.status === "failed" && (
            <p className="pl-6 text-xs text-red-300">Drafting failed. Retry to try again.</p>
          )}
          <AnimatePresence initial={false}>
          {expanded && (
            <motion.div
              key="body"
              initial={{ opacity: 0, height: 0 }}
              animate={{ opacity: 1, height: "auto" }}
              exit={{ opacity: 0, height: 0 }}
              transition={{ duration: 0.2 }}
              className="overflow-hidden"
            >
              <div className="space-y-2 pl-6 pt-2">
              <p className="whitespace-pre-wrap text-xs leading-relaxed text-fg/80">
                {d.content}
              </p>
              {d.keywords.length > 0 && (
                <div className="flex flex-wrap gap-1">
                  {d.keywords.map((k) => (
                    <span
                      key={k}
                      className="rounded-full border border-fg/10 bg-fg/5 px-2 py-0.5 text-[10px] text-fg/70"
                    >
                      {k}
                    </span>
                  ))}
                </div>
              )}
              <div className="flex flex-wrap gap-2 pt-1">
                <motion.button
                  type="button"
                  onClick={() => onApprove(d.planIdx, d.status !== "approved")}
                  whileTap={{ scale: 0.96 }}
                  className={cn(
                    "rounded-lg border px-3 py-1.5 text-xs font-medium transition",
                    d.status === "approved"
                      ? "border-emerald-500/40 bg-emerald-500/15 text-emerald-200"
                      : "border-fg/10 hover:border-fg/20 hover:bg-fg/5",
                  )}
                >
                  {d.status === "approved" ? (
                    <span className="flex items-center gap-1.5">
                      <Check className="h-3 w-3" /> Approved
                    </span>
                  ) : (
                    "Approve"
                  )}
                </motion.button>
                <button
                  type="button"
                  onClick={() => onOpenRefine(d.planIdx)}
                  className="rounded-lg border border-fg/10 px-3 py-1.5 text-xs transition hover:border-fg/20 hover:bg-fg/5"
                >
                  Ask for changes
                </button>
                <button
                  type="button"
                  onClick={() => onOpenEdit(d.planIdx)}
                  className="rounded-lg border border-fg/10 px-3 py-1.5 text-xs transition hover:border-fg/20 hover:bg-fg/5"
                >
                  Edit manually
                </button>
              </div>
              </div>
            </motion.div>
          )}
          </AnimatePresence>
          </motion.div>
          );
        })}
        </AnimatePresence>
      </div>

      <div className="flex flex-wrap gap-3 pt-2">
        {anyPending && (
          <motion.button
            type="button"
            onClick={onRetryFailed}
            disabled={busy}
            whileTap={{ scale: 0.98 }}
            className="flex flex-1 items-center justify-center gap-2 rounded-xl border border-fg/10 py-3 text-sm transition hover:border-fg/20 hover:bg-fg/5 disabled:opacity-50"
          >
            {busy && <Loader2 className="h-4 w-4 animate-spin" />} Continue drafting
          </motion.button>
        )}
        {anyFailed && !anyPending && (
          <button
            type="button"
            onClick={onRetryFailed}
            disabled={busy}
            className="flex flex-1 items-center justify-center gap-2 rounded-xl border border-fg/10 py-3 text-sm hover:bg-fg/5 disabled:opacity-50"
          >
            <RefreshCw className="h-4 w-4" /> Retry failed
          </button>
        )}
        {allDone && (
          <>
            <button
              type="button"
              onClick={onRunCoherence}
              disabled={busy}
              className="flex flex-1 items-center justify-center gap-2 rounded-xl border border-fg/10 py-3 text-sm hover:bg-fg/5 disabled:opacity-50"
            >
              {busy ? <Loader2 className="h-4 w-4 animate-spin" /> : <Wand2 className="h-4 w-4" />}{" "}
              Run coherence check
            </button>
            <button
              type="button"
              onClick={onSkipToCommit}
              disabled={busy}
              className="flex flex-1 items-center justify-center gap-2 rounded-xl border border-accent/40 bg-accent/15 py-3 text-sm font-medium text-accent hover:bg-accent/25 disabled:opacity-50"
            >
              <ArrowRight className="h-4 w-4" /> Save lorebook
            </button>
          </>
        )}
      </div>
    </div>
  );
}

function EntityTriggerButton({
  label,
  iconUrl,
  fallbackIcon,
  primary,
  secondary,
  placeholder,
  onClick,
}: {
  label: string;
  iconUrl: string | undefined;
  fallbackIcon: React.ReactNode;
  primary: string | null;
  secondary?: string | null;
  placeholder: string;
  onClick: () => void;
}) {
  const hasAvatar = iconUrl && isRenderableImageUrl(iconUrl);
  return (
    <div className="space-y-2">
      <span className="text-[11px] font-semibold uppercase tracking-[0.18em] text-fg/45">
        {label}
      </span>
      <button
        type="button"
        onClick={onClick}
        className={cn(
          "flex w-full items-center gap-3 rounded-xl border border-fg/10 bg-fg/3 px-3 py-2.5 text-left",
          interactive.transition.default,
          "hover:border-fg/20 hover:bg-fg/6 active:scale-[0.99]",
        )}
      >
        <div
          className={cn(
            "h-9 w-9 shrink-0 overflow-hidden rounded-full border",
            primary ? "border-fg/15 bg-fg/5" : "border-dashed border-fg/15 bg-transparent",
            "flex items-center justify-center",
          )}
        >
          {hasAvatar ? (
            <AvatarImage src={iconUrl!} alt={primary ?? label} />
          ) : (
            <span className="text-fg/45">{fallbackIcon}</span>
          )}
        </div>
        <div className="min-w-0 flex-1">
          <p className={cn("truncate text-sm", primary ? "text-fg" : "text-fg/45")}>
            {primary ?? placeholder}
          </p>
          {secondary && (
            <p className="mt-0.5 truncate text-[11px] text-fg/45">{secondary}</p>
          )}
        </div>
        <ChevronRight className="h-4 w-4 shrink-0 text-fg/30" />
      </button>
    </div>
  );
}

function CharacterTrigger({
  characters,
  selectedId,
  onChange,
}: {
  characters: Character[];
  selectedId: string;
  onChange: (id: string) => void;
}) {
  const [open, setOpen] = useState(false);
  const selected = characters.find((c) => c.id === selectedId) ?? null;
  const avatarUrl = useAvatar(
    "character",
    selected?.id ?? "",
    selected?.avatarPath,
    "round",
  );
  return (
    <>
      <EntityTriggerButton
        label="Character"
        iconUrl={avatarUrl}
        fallbackIcon={<User className="h-5 w-5" />}
        primary={selected?.name ?? null}
        secondary={selected?.description ?? selected?.definition ?? null}
        placeholder="None"
        onClick={() => setOpen(true)}
      />
      <CharacterSelectorSingle
        isOpen={open}
        onClose={() => setOpen(false)}
        characters={characters}
        selectedCharacterId={selectedId || null}
        onSelect={(id) => onChange(id ?? "")}
      />
    </>
  );
}

function PersonaTrigger({
  personas,
  selectedId,
  onChange,
}: {
  personas: Persona[];
  selectedId: string;
  onChange: (id: string) => void;
}) {
  const [open, setOpen] = useState(false);
  const selected = personas.find((p) => p.id === selectedId) ?? null;
  const avatarUrl = useAvatar(
    "persona",
    selected?.id ?? "",
    selected?.avatarPath,
    "round",
  );
  return (
    <>
      <EntityTriggerButton
        label="Persona"
        iconUrl={avatarUrl}
        fallbackIcon={<UserCircle className="h-5 w-5" />}
        primary={selected?.title ?? null}
        secondary={selected?.description ?? null}
        placeholder="None"
        onClick={() => setOpen(true)}
      />
      <PersonaSelector
        isOpen={open}
        onClose={() => setOpen(false)}
        personas={personas}
        selectedPersonaId={selectedId || null}
        onSelect={(id) => onChange(id ?? "")}
      />
    </>
  );
}

function statusBadge(status: string) {
  const map: Record<string, { label: string; className: string }> = {
    pending: { label: "pending", className: "bg-fg/10 text-fg/60" },
    drafting: { label: "drafting", className: "bg-accent/15 text-accent" },
    drafted: { label: "drafted", className: "bg-emerald-500/10 text-emerald-300" },
    approved: { label: "approved", className: "bg-emerald-500/20 text-emerald-200" },
    failed: { label: "failed", className: "bg-red-500/15 text-red-300" },
  };
  const tag = map[status] ?? { label: status, className: "bg-fg/10 text-fg/60" };
  if (status === "drafting") {
    return (
      <motion.span
        animate={{ opacity: [0.6, 1, 0.6] }}
        transition={{ duration: 1.4, repeat: Infinity }}
        className={`rounded-full px-2 py-0.5 text-[10px] font-medium ${tag.className}`}
      >
        {tag.label}
      </motion.span>
    );
  }
  return (
    <span className={`rounded-full px-2 py-0.5 text-[10px] font-medium ${tag.className}`}>
      {tag.label}
    </span>
  );
}

function CoherenceBanner({
  job,
  accepted,
  setAccepted,
  onApply,
  onSkip,
  busy,
}: {
  job: LorebookGeneratorJobState;
  accepted: Set<string>;
  setAccepted: React.Dispatch<React.SetStateAction<Set<string>>>;
  onApply: () => void;
  onSkip: () => void;
  busy: boolean;
}) {
  const toggle = (id: string) => {
    setAccepted((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  if (job.coherenceProposals.length === 0) {
    return (
      <motion.div
        {...animations.fadeInFast}
        className="flex items-center gap-3 rounded-xl border border-emerald-500/30 bg-emerald-500/[0.06] px-4 py-3"
      >
        <CheckCircle2 className="h-5 w-5 shrink-0 text-emerald-400" />
        <p className="flex-1 text-sm text-fg/80">No coherence issues found.</p>
        <button
          type="button"
          onClick={onSkip}
          className="rounded-lg border border-fg/10 px-3 py-1.5 text-xs transition hover:border-fg/20 hover:bg-fg/5"
        >
          Continue
        </button>
      </motion.div>
    );
  }

  return (
    <motion.div
      {...animations.fadeInFast}
      className="space-y-3 rounded-xl border border-info/30 bg-info/[0.06] p-4"
    >
      <p className="text-xs font-semibold uppercase tracking-[0.18em] text-info">
        Coherence proposals · {job.coherenceProposals.length}
      </p>
      <p className="text-xs text-fg/60">
        Toggle each change to accept or reject before applying.
      </p>
      {job.coherenceProposals.map((c) => {
        const checked = accepted.has(c.id);
        let summary = "";
        switch (c.kind) {
          case "mergeKeys":
            summary = `Remove keys [${c.removeKeys.join(", ")}] from entry ${c.entryIdx + 1}. ${c.reason}`;
            break;
          case "renameTerm":
            summary = `Rename "${c.oldTerm}" → "${c.newTerm}" across entries ${c.affectedEntryIdxs.length === 0 ? "(all)" : c.affectedEntryIdxs.map((n) => n + 1).join(", ")}. ${c.reason}`;
            break;
          case "flagContradiction":
            summary = `Contradiction in entries ${c.entryIdxs.map((n) => n + 1).join(", ")}: ${c.description}`;
            break;
          case "toggleAlwaysActive":
            summary = `Set entry ${c.entryIdx + 1} alwaysActive to ${c.newValue}. ${c.reason}`;
            break;
        }
        return (
          <label
            key={c.id}
            className="flex cursor-pointer items-start gap-3 rounded-lg border border-fg/10 bg-fg/[0.02] p-2.5 transition hover:border-fg/20"
          >
            <input
              type="checkbox"
              checked={checked}
              onChange={() => toggle(c.id)}
              className="mt-1 h-4 w-4 accent-accent"
            />
            <div className="flex-1 text-xs leading-relaxed text-fg/80">
              <span className="mr-2 rounded bg-fg/10 px-1.5 py-0.5 text-[10px] font-semibold uppercase">
                {c.kind}
              </span>
              {summary}
            </div>
          </label>
        );
      })}
      <div className="flex gap-3 pt-1">
        <button
          type="button"
          onClick={onSkip}
          className="flex-1 rounded-lg border border-fg/10 py-2 text-xs transition hover:border-fg/20 hover:bg-fg/5"
        >
          Skip
        </button>
        <motion.button
          type="button"
          onClick={onApply}
          disabled={busy}
          whileTap={{ scale: 0.98 }}
          className="flex flex-1 items-center justify-center gap-2 rounded-lg border border-accent/40 bg-accent/10 py-2 text-xs font-semibold text-accent transition hover:bg-accent/20 disabled:opacity-50"
        >
          {busy && <Loader2 className="h-3.5 w-3.5 animate-spin" />} Apply selected
        </motion.button>
      </div>
    </motion.div>
  );
}

function CommitForm({
  lorebooks,
  commitTarget,
  setCommitTarget,
  newLorebookName,
  draftCount,
  onCommit,
  onBack,
  busy,
}: {
  lorebooks: Lorebook[];
  commitTarget: string;
  setCommitTarget: (v: string) => void;
  newLorebookName: string;
  draftCount: number;
  onCommit: () => void;
  onBack: () => void;
  busy: boolean;
}) {
  return (
    <div className="space-y-5">
      <p className="text-sm text-fg/60">
        Save {draftCount} entries into a lorebook.
      </p>
      <Field label="Destination">
        <select
          value={commitTarget}
          onChange={(e) => setCommitTarget(e.target.value)}
          className={inputClass}
        >
          <option value="">Create new lorebook · "{newLorebookName}"</option>
          {lorebooks.map((lb) => (
            <option key={lb.id} value={lb.id}>
              Append to: {lb.name}
            </option>
          ))}
        </select>
      </Field>
      <div className="flex gap-3 pt-2">
        <button
          type="button"
          onClick={onBack}
          className="flex-1 rounded-xl border border-fg/10 py-3 text-sm transition hover:border-fg/20 hover:bg-fg/5"
        >
          Back
        </button>
        <motion.button
          type="button"
          onClick={onCommit}
          disabled={busy}
          whileTap={{ scale: 0.98 }}
          className="flex flex-1 items-center justify-center gap-2 rounded-xl border border-accent/40 bg-accent/10 py-3 text-sm font-semibold text-accent transition hover:bg-accent/20 disabled:opacity-50"
        >
          {busy ? <Loader2 className="h-4 w-4 animate-spin" /> : <CheckCircle2 className="h-4 w-4" />}
          Save
        </motion.button>
      </div>
    </div>
  );
}

function RefineSheet({
  isOpen,
  draftTitle,
  feedback,
  setFeedback,
  onClose,
  onSubmit,
  busy,
}: {
  isOpen: boolean;
  draftTitle: string;
  feedback: string;
  setFeedback: (v: string) => void;
  onClose: () => void;
  onSubmit: () => void;
  busy: boolean;
}) {
  return (
    <BottomMenu isOpen={isOpen} onClose={onClose} title={`Ask for changes: ${draftTitle}`}>
      <div className="space-y-3">
        <textarea
          autoFocus
          value={feedback}
          onChange={(e) => setFeedback(e.target.value)}
          rows={5}
          placeholder="Describe what to change…"
          className="w-full rounded-xl border border-fg/10 bg-fg/[0.02] px-3.5 py-3 text-sm transition focus:border-fg/30 focus:outline-none"
        />
        <div className="flex gap-3">
          <button
            type="button"
            onClick={onClose}
            className="flex-1 rounded-xl border border-fg/10 py-2.5 text-sm transition hover:border-fg/20 hover:bg-fg/5"
          >
            Cancel
          </button>
          <motion.button
            type="button"
            onClick={onSubmit}
            disabled={busy || !feedback.trim()}
            whileTap={{ scale: 0.98 }}
            className="flex flex-1 items-center justify-center gap-2 rounded-xl border border-accent/40 bg-accent/10 py-2.5 text-sm font-semibold text-accent transition hover:bg-accent/20 disabled:opacity-50"
          >
            {busy && <Loader2 className="h-4 w-4 animate-spin" />} Apply
          </motion.button>
        </div>
      </div>
    </BottomMenu>
  );
}

function EditDraftModal({
  initial,
  onCancel,
  onSave,
  busy,
}: {
  initial: { title: string; keywords: string[]; content: string; alwaysActive: boolean };
  onCancel: () => void;
  onSave: (
    title: string,
    keywords: string[],
    content: string,
    alwaysActive: boolean,
  ) => void;
  busy: boolean;
}) {
  const [title, setTitle] = useState(initial.title);
  const [keywordsText, setKeywordsText] = useState(initial.keywords.join(", "));
  const [content, setContent] = useState(initial.content);
  const [alwaysActive, setAlwaysActive] = useState(initial.alwaysActive);

  return (
    <div className="fixed inset-0 z-30 flex items-center justify-center bg-black/60 px-4">
      <div className="w-full max-w-xl space-y-3 rounded-2xl border border-fg/10 bg-surface p-5">
        <h3 className="text-sm font-semibold">Edit entry</h3>
        <input
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          placeholder="Title"
          className="w-full rounded-xl border border-fg/10 bg-surface-el/20 px-3.5 py-2 text-sm"
        />
        <input
          value={keywordsText}
          onChange={(e) => setKeywordsText(e.target.value)}
          placeholder="Comma-separated keywords"
          className="w-full rounded-xl border border-fg/10 bg-surface-el/20 px-3.5 py-2 text-sm"
        />
        <textarea
          value={content}
          onChange={(e) => setContent(e.target.value)}
          rows={8}
          className="w-full rounded-xl border border-fg/10 bg-surface-el/20 px-3.5 py-3 text-sm"
        />
        <label className="flex items-center gap-2 text-xs">
          <input
            type="checkbox"
            checked={alwaysActive}
            onChange={(e) => setAlwaysActive(e.target.checked)}
            className="accent-accent"
          />
          Always active (no keyword required)
        </label>
        <div className="flex gap-3">
          <button
            type="button"
            onClick={onCancel}
            className="flex-1 rounded-xl border border-fg/10 py-2.5 text-sm hover:bg-fg/5"
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={() =>
              onSave(
                title.trim(),
                keywordsText
                  .split(",")
                  .map((s) => s.trim())
                  .filter((s) => s.length > 0),
                content,
                alwaysActive,
              )
            }
            disabled={busy}
            className="flex flex-1 items-center justify-center gap-2 rounded-xl border border-accent/40 bg-accent/15 py-2.5 text-sm font-medium text-accent hover:bg-accent/25 disabled:opacity-50"
          >
            {busy && <Loader2 className="h-4 w-4 animate-spin" />} Save
          </button>
        </div>
      </div>
    </div>
  );
}
