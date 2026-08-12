import { Fragment, useEffect, useMemo, useRef, useState } from "react";
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
import { NumberInput } from "../../components/NumberInput";
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
  lorebookGenUpdateOutline,
  readFileAsBase64,
  type LorebookGeneratorEntryPlan,
  type LorebookGeneratorJobState,
  type LorebookGeneratorSourceInput,
} from "../../../core/lorebook/generator";
import { listCharacters, listLorebooks, listPersonas } from "../../../core/storage/repo";
import type { Character, Lorebook, Persona } from "../../../core/storage/schemas";
import { useI18n, type TranslationKey } from "../../../core/i18n/context";

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
  const { t } = useI18n();
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
        setError(t("lorebookGen.flow.errorUnsupportedFile", { name: f.name }));
        continue;
      }
      if (f.size > FILE_LIMIT_BYTES) {
        setError(t("lorebookGen.flow.errorFileTooLarge", { name: f.name }));
        continue;
      }
      try {
        const b64 = await readFileAsBase64(f);
        next.push({ name: f.name, kind, size: f.size, dataBase64: b64 });
      } catch (err) {
        setError(t("lorebookGen.flow.errorReadFile", {
          name: f.name,
          error: err instanceof Error ? err.message : String(err),
        }));
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
          title: t("lorebookGen.flow.newEntry"),
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
    <div className="mx-auto w-full max-w-3xl space-y-6">
      <StageStepper pageStage={pageStage} />
      {error && (
        <div className="flex items-start gap-2 rounded-xl border border-danger/30 bg-danger/10 px-3 py-2 text-sm text-danger">
          <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0" />
          <div className="flex-1">{error}</div>
          <button
            type="button"
            onClick={() => setError(null)}
            className="text-danger/70 hover:text-danger"
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
            className="flex flex-col items-center gap-4 py-16"
          >
            <Loader2 className="h-8 w-8 animate-spin text-accent/70" />
            <p className="text-sm text-fg/55">{t("lorebookGen.flow.planningEntries")}</p>
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
            newLorebookName={lorebookName.trim() || initialName.trim() || t("common.labels.untitled")}
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
            className="flex flex-col items-center gap-4 py-16"
          >
            <CheckCircle2 className="h-10 w-10 text-accent" />
            <p className="text-sm text-fg/55">{t("lorebookGen.flow.lorebookSaved")}</p>
          </motion.div>
        )}
      </AnimatePresence>

      <RefineSheet
        isOpen={refineFor !== null && !!job}
        draftTitle={
          job && refineFor !== null
            ? job.drafts.find((d) => d.planIdx === refineFor)?.title ??
              t("lorebookGen.flow.entryFallback", { index: refineFor + 1 })
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

const STEPS: Array<{ key: PageStage; labelKey: TranslationKey; matches: PageStage[] }> = [
  { key: "brief", labelKey: "lorebookGen.flow.stepBrief", matches: ["brief", "planning"] },
  { key: "outline", labelKey: "lorebookGen.flow.stepOutline", matches: ["outline"] },
  { key: "drafting", labelKey: "lorebookGen.flow.stepDrafts", matches: ["drafting", "coherence"] },
  { key: "commit", labelKey: "lorebookGen.flow.stepSave", matches: ["commit", "done"] },
];

function activeStepIndex(s: PageStage): number {
  const i = STEPS.findIndex((step) => step.matches.includes(s));
  return i === -1 ? 0 : i;
}

function StageStepper({ pageStage }: { pageStage: PageStage }) {
  const { t } = useI18n();
  const activeIdx = activeStepIndex(pageStage);
  return (
    <div className="flex items-center gap-2 px-1 pt-3 pb-1 sm:gap-3">
      {STEPS.map((step, i) => {
        const isActive = i === activeIdx;
        const isComplete = i < activeIdx;
        return (
          <Fragment key={step.key}>
            <div className="flex shrink-0 items-center gap-2">
              <motion.div
                layout
                className={cn(
                  "flex h-7 w-7 shrink-0 items-center justify-center rounded-full border text-[11px] font-semibold",
                  interactive.transition.default,
                  isActive
                    ? "border-accent bg-accent/15 text-accent"
                    : isComplete
                      ? "border-accent/30 bg-accent/10 text-accent/80"
                      : "border-fg/15 bg-fg/5 text-fg/40",
                )}
              >
                <span>
                  {isComplete ? <Check className="h-3.5 w-3.5" /> : i + 1}
                </span>
              </motion.div>
              <span
                className={cn(
                  "hidden truncate text-xs font-medium sm:inline",
                  isActive ? "text-fg" : isComplete ? "text-fg/70" : "text-fg/40",
                )}
              >
                {t(step.labelKey)}
              </span>
            </div>
            {i < STEPS.length - 1 && (
              <div className="relative h-px flex-1 bg-fg/10">
                <motion.div
                  initial={false}
                  animate={{ scaleX: i < activeIdx ? 1 : 0 }}
                  style={{ transformOrigin: "left" }}
                  transition={{ duration: 0.3, ease: [0.25, 0.46, 0.45, 0.94] }}
                  className="absolute inset-0 bg-accent/40"
                />
              </div>
            )}
          </Fragment>
        );
      })}
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
  const { t } = useI18n();
  return (
    <div className="space-y-6">
      <div className={groupCardClass}>
        <div className="px-4 py-3">
          <Field label={t("lorebookGen.flow.lorebookNameLabel")}>
            <input
              value={lorebookName}
              onChange={(e) => setLorebookName(e.target.value)}
              placeholder={t("lorebookGen.flow.lorebookNamePlaceholder")}
              className={inputClass}
            />
          </Field>
        </div>
        <div className="px-4 py-3">
          <Field label={t("lorebookGen.flow.briefLabel")}>
            <textarea
              value={brief}
              onChange={(e) => setBrief(e.target.value)}
              placeholder={t("lorebookGen.flow.briefPlaceholder")}
              rows={5}
              className={cn(inputClass, "resize-y")}
            />
          </Field>
        </div>
      </div>

      <div className={groupCardClass}>
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

      <div className={groupCardClass}>
        <div className="flex items-center gap-2 px-4 py-3">
          <span className="text-[11px] font-medium text-fg/70">
            {t("lorebookGen.flow.pastedTextLabel")}
          </span>
          <span className="text-[10px] text-fg/40">
            · {textSources.length > 0 ? `${textSources.length}` : t("lorebookGen.flow.optional")}
          </span>
          <button
            type="button"
            onClick={addTextSource}
            className={cn(pillButtonClass, "ml-auto")}
          >
            <Plus className="h-3 w-3" /> {t("common.buttons.add")}
          </button>
        </div>
        {textSources.length === 0 ? (
          <p className="px-4 py-3 text-xs text-fg/40">
            {t("lorebookGen.flow.referenceMaterialHint")}
          </p>
        ) : (
          <AnimatePresence initial={false}>
            {textSources.map((src) => (
              <motion.div
                key={src.id}
                layout
                {...animations.fadeInFast}
                className="space-y-2 px-4 py-3"
              >
                <div className="flex items-center gap-2">
                  <input
                    value={src.label}
                    onChange={(e) => updateTextSource(src.id, { label: e.target.value })}
                    className={cn(inputClass, "px-3 py-1.5 text-xs")}
                  />
                  <button
                    type="button"
                    onClick={() => removeTextSource(src.id)}
                    className="rounded-lg p-1.5 text-fg/40 transition hover:bg-fg/5 hover:text-danger"
                  >
                    <Trash2 className="h-3.5 w-3.5" />
                  </button>
                </div>
                <textarea
                  value={src.body}
                  onChange={(e) => updateTextSource(src.id, { body: e.target.value })}
                  rows={4}
                  placeholder={t("lorebookGen.flow.pasteSourcePlaceholder")}
                  className={cn(inputClass, "px-3 py-2 text-xs")}
                />
              </motion.div>
            ))}
          </AnimatePresence>
        )}
      </div>

      <div className={groupCardClass}>
        <div className="flex items-center gap-2 px-4 py-3">
          <span className="text-[11px] font-medium text-fg/70">
            {t("lorebookGen.flow.filesLabel")}
          </span>
          <span className="text-[10px] text-fg/40">
            · {files.length > 0 ? `${files.length}` : ".txt, .md, .pdf"}
          </span>
          <button
            type="button"
            onClick={() => fileInputRef.current?.click()}
            className={cn(pillButtonClass, "ml-auto")}
          >
            <Upload className="h-3 w-3" /> {t("lorebookGen.flow.upload")}
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
        </div>
        {files.length === 0 ? (
          <p className="px-4 py-3 text-xs text-fg/40">
            {t("lorebookGen.flow.fileSizeHint")}
          </p>
        ) : (
          <>
            <AnimatePresence initial={false}>
              {files.map((f) => (
                <motion.div
                  key={f.name}
                  layout
                  {...animations.fadeInFast}
                  className="flex items-center justify-between gap-3 px-4 py-2 text-xs"
                >
                  <div className="flex min-w-0 items-center gap-2">
                    <span className="shrink-0 rounded bg-fg/10 px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-fg/60">
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
                    className="rounded-lg p-1.5 text-fg/40 transition hover:bg-fg/5 hover:text-danger"
                  >
                    <Trash2 className="h-3.5 w-3.5" />
                  </button>
                </motion.div>
              ))}
            </AnimatePresence>
            <p className="px-4 py-2 text-[11px] text-fg/40">
              {t("lorebookGen.flow.totalKb", { size: (totalSize / 1024).toFixed(0) })}
            </p>
          </>
        )}
      </div>

      <div className={groupCardClass}>
        <div className="px-4 py-3">
          <Field label={t("lorebookGen.flow.targetEntryCount")}>
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
              <NumberInput
                min={MIN_TARGET}
                max={MAX_TARGET}
                step={1}
                value={targetCount}
                onChange={(next) => {
                  if (next !== null) setTargetCount(Math.round(next));
                }}
                className="w-16 shrink-0 rounded-lg border border-fg/10 bg-surface-el/20 px-2 py-1.5 text-center text-sm tabular-nums focus:border-fg/30 focus:outline-none"
              />
            </div>
            <p className="text-[11px] text-fg/40">
              {t("lorebookGen.flow.rangeHint", { min: MIN_TARGET, max: MAX_TARGET })}
            </p>
          </Field>
        </div>
      </div>

      <motion.button
        type="button"
        onClick={onStart}
        disabled={!canStart}
        whileTap={canStart ? { scale: 0.98 } : undefined}
        className={cn(btnPrimary, "h-11 w-full")}
      >
        {busy ? <Loader2 className="h-4 w-4 animate-spin" /> : <Wand2 className="h-4 w-4" />}
        {t("lorebookGen.flow.planEntries")}
      </motion.button>
    </div>
  );
}

const inputClass =
  "w-full rounded-lg border border-fg/10 bg-surface-el/20 px-3 py-2 text-sm text-fg placeholder-fg/40 focus:border-fg/30 focus:outline-none";
const pillButtonClass =
  "flex items-center gap-1.5 rounded-lg border border-fg/10 bg-surface/60 px-2.5 py-1 text-[11px] font-medium text-fg/70 transition hover:bg-surface-el/65 hover:text-fg";
const groupCardClass =
  "overflow-hidden rounded-xl border border-fg/10 bg-fg/[0.025] divide-y divide-fg/[0.06]";
const btnSecondary =
  "flex items-center justify-center gap-2 rounded-lg border border-fg/10 bg-surface/60 px-3.5 text-sm font-medium text-fg transition hover:bg-surface-el/65 disabled:opacity-50";
const btnPrimary =
  "flex items-center justify-center gap-2 rounded-lg border border-accent/30 bg-accent/10 px-3.5 text-sm font-medium text-accent transition hover:bg-accent/15 disabled:opacity-40";

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
        <span className="text-[11px] font-medium text-fg/70">
          {label}
        </span>
        {hint && <span className="text-[10px] text-fg/40">· {hint}</span>}
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
  const { t } = useI18n();
  return (
    <div className="space-y-4">
      <p className="text-sm text-fg/55">
        {t("lorebookGen.flow.outlineIntro")}
      </p>
      <div className={groupCardClass}>
        {job.outline.map((p, i) => (
          <div key={i} className="space-y-2 px-3 py-3">
            <div className="flex items-center gap-2">
              <input
                value={p.title}
                onChange={(e) => onUpdate(i, { title: e.target.value })}
                className={cn(inputClass, "flex-1 px-2 py-1 font-medium")}
              />
              <select
                value={p.category}
                onChange={(e) => onUpdate(i, { category: e.target.value })}
                className={cn(inputClass, "w-auto px-2 py-1 text-xs")}
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
                className="rounded-lg p-1 text-fg/40 transition hover:bg-fg/5 hover:text-danger"
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
              placeholder={t("lorebookGen.flow.commaSeparatedKeys")}
              className={cn(inputClass, "px-2 py-1 text-xs")}
            />
            <textarea
              value={p.rationale}
              onChange={(e) => onUpdate(i, { rationale: e.target.value })}
              rows={2}
              placeholder={t("lorebookGen.flow.rationalePlaceholder")}
              className={cn(inputClass, "px-2 py-1 text-xs")}
            />
          </div>
        ))}
      </div>
      <button
        type="button"
        onClick={onAdd}
        className="flex w-full items-center justify-center gap-1 rounded-xl border border-dashed border-fg/15 px-3 py-2 text-xs text-fg/60 transition hover:bg-fg/[0.04]"
      >
        <Plus className="h-3.5 w-3.5" /> {t("lorebookGen.flow.addEntry")}
      </button>

      <div className="flex gap-3 pt-2">
        <button
          type="button"
          onClick={onBack}
          className={cn(btnSecondary, "h-11 flex-1")}
        >
          {t("common.buttons.back")}
        </button>
        <button
          type="button"
          onClick={onApprove}
          disabled={busy || job.outline.length === 0}
          className={cn(btnPrimary, "h-11 flex-1")}
        >
          {busy ? <Loader2 className="h-4 w-4 animate-spin" /> : <ArrowRight className="h-4 w-4" />}
          {t("lorebookGen.flow.approveAndDraft")}
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
  const { t } = useI18n();
  const allDone = job.drafts.every(
    (d) => d.status === "drafted" || d.status === "approved",
  );
  const anyPending = job.drafts.some(
    (d) => d.status === "pending" || d.status === "drafting",
  );
  const anyFailed = job.drafts.some((d) => d.status === "failed");

  return (
    <div className="space-y-4">
      <p className="text-sm text-fg/55">
        {t("lorebookGen.flow.draftsIntro")}
      </p>
      {coherenceBanner}
      <div className={groupCardClass}>
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
            className="space-y-2 px-3 py-3"
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
            <span
              className={cn(
                "h-4 w-0.5 shrink-0 rounded-full",
                d.status === "approved"
                  ? "bg-accent"
                  : d.status === "failed"
                    ? "bg-danger/70"
                    : "bg-transparent",
              )}
            />
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
              {d.title || t("lorebookGen.flow.entryFallback", { index: d.planIdx + 1 })}
            </span>
            <span className="ml-auto whitespace-nowrap text-[11px] text-fg/40">
              {d.alwaysActive ? t("lorebookGen.flow.alwaysActivePrefix") : ""}
              {t("lorebookGen.flow.keysCount", { count: d.keywords.length })}
            </span>
            {inFlight && (
              <Loader2 className="h-3.5 w-3.5 shrink-0 animate-spin text-accent" />
            )}
            {statusBadge(d.status, t)}
          </button>
          {d.status === "failed" && (
            <p className="pl-6 text-xs text-danger/80">{t("lorebookGen.flow.draftingFailedRetry")}</p>
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
                      ? "border-accent/30 bg-accent/10 text-accent"
                      : "border-fg/10 bg-surface/60 hover:bg-surface-el/65",
                  )}
                >
                  {d.status === "approved" ? (
                    <span className="flex items-center gap-1.5">
                      <Check className="h-3 w-3" /> {t("lorebookGen.flow.approved")}
                    </span>
                  ) : (
                    t("lorebookGen.flow.approve")
                  )}
                </motion.button>
                <button
                  type="button"
                  onClick={() => onOpenRefine(d.planIdx)}
                  className="rounded-lg border border-fg/10 bg-surface/60 px-3 py-1.5 text-xs font-medium transition hover:bg-surface-el/65"
                >
                  {t("lorebookGen.flow.askForChanges")}
                </button>
                <button
                  type="button"
                  onClick={() => onOpenEdit(d.planIdx)}
                  className="rounded-lg border border-fg/10 bg-surface/60 px-3 py-1.5 text-xs font-medium transition hover:bg-surface-el/65"
                >
                  {t("lorebookGen.flow.editManually")}
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
            className={cn(btnSecondary, "h-11 flex-1")}
          >
            {busy && <Loader2 className="h-4 w-4 animate-spin" />} {t("lorebookGen.flow.continueDrafting")}
          </motion.button>
        )}
        {anyFailed && !anyPending && (
          <button
            type="button"
            onClick={onRetryFailed}
            disabled={busy}
            className={cn(btnSecondary, "h-11 flex-1")}
          >
            <RefreshCw className="h-4 w-4" /> {t("lorebookGen.flow.retryFailed")}
          </button>
        )}
        {allDone && (
          <>
            <button
              type="button"
              onClick={onRunCoherence}
              disabled={busy}
              className={cn(btnSecondary, "h-11 flex-1")}
            >
              {busy ? <Loader2 className="h-4 w-4 animate-spin" /> : <Wand2 className="h-4 w-4" />}{" "}
              {t("lorebookGen.flow.runCoherenceCheck")}
            </button>
            <button
              type="button"
              onClick={onSkipToCommit}
              disabled={busy}
              className={cn(btnPrimary, "h-11 flex-1")}
            >
              <ArrowRight className="h-4 w-4" /> {t("lorebookGen.flow.saveLorebook")}
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
  placeholder,
  onClick,
}: {
  label: string;
  iconUrl: string | undefined;
  fallbackIcon: React.ReactNode;
  primary: string | null;
  placeholder: string;
  onClick: () => void;
}) {
  const hasAvatar = iconUrl && isRenderableImageUrl(iconUrl);
  return (
    <button
      type="button"
      onClick={onClick}
      className="flex w-full items-center gap-3 px-4 py-3 text-left transition-colors duration-150 hover:bg-fg/[0.04] focus:bg-fg/[0.04] focus:outline-none"
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
        <p className="truncate text-sm font-medium text-fg">{label}</p>
        <p className={cn("mt-0.5 truncate text-[11px]", primary ? "text-fg/60" : "text-fg/40")}>
          {primary ?? placeholder}
        </p>
      </div>
      <ChevronRight className="h-4 w-4 shrink-0 text-fg/30" />
    </button>
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
  const { t } = useI18n();
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
        label={t("lorebookGen.flow.characterLabel")}
        iconUrl={avatarUrl}
        fallbackIcon={<User className="h-5 w-5" />}
        primary={selected?.name ?? null}
        placeholder={t("lorebookGen.flow.none")}
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
  const { t } = useI18n();
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
        label={t("lorebookGen.flow.personaLabel")}
        iconUrl={avatarUrl}
        fallbackIcon={<UserCircle className="h-5 w-5" />}
        primary={selected?.title ?? null}
        placeholder={t("lorebookGen.flow.none")}
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

function statusBadge(status: string, t: (key: TranslationKey) => string) {
  const map: Record<string, { labelKey: TranslationKey; className: string }> = {
    pending: { labelKey: "lorebookGen.flow.draftStatus.pending", className: "bg-fg/10 text-fg/60" },
    drafting: { labelKey: "lorebookGen.flow.draftStatus.drafting", className: "bg-accent/15 text-accent" },
    drafted: { labelKey: "lorebookGen.flow.draftStatus.drafted", className: "bg-fg/10 text-fg/60" },
    approved: { labelKey: "lorebookGen.flow.draftStatus.approved", className: "bg-accent/15 text-accent" },
    failed: { labelKey: "lorebookGen.flow.draftStatus.failed", className: "bg-danger/15 text-danger" },
  };
  const entry = map[status];
  const tag = entry
    ? { label: t(entry.labelKey), className: entry.className }
    : { label: status, className: "bg-fg/10 text-fg/60" };
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
  const { t } = useI18n();
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
        className="flex items-center gap-3 rounded-xl border border-fg/10 bg-fg/[0.025] px-4 py-3"
      >
        <CheckCircle2 className="h-5 w-5 shrink-0 text-accent" />
        <p className="flex-1 text-sm text-fg/80">{t("lorebookGen.flow.noCoherenceIssues")}</p>
        <button
          type="button"
          onClick={onSkip}
          className="rounded-lg border border-fg/10 bg-surface/60 px-3 py-1.5 text-xs font-medium transition hover:bg-surface-el/65"
        >
          {t("common.buttons.continue")}
        </button>
      </motion.div>
    );
  }

  return (
    <motion.div
      {...animations.fadeInFast}
      className="space-y-3 rounded-xl border border-fg/10 bg-fg/[0.025] p-4"
    >
      <p className="text-[10px] font-bold uppercase tracking-wider text-fg/40">
        {t("lorebookGen.flow.coherenceProposals", { count: job.coherenceProposals.length })}
      </p>
      <p className="text-xs text-fg/60">
        {t("lorebookGen.flow.coherenceToggleHint")}
      </p>
      {job.coherenceProposals.map((c) => {
        const checked = accepted.has(c.id);
        let summary = "";
        switch (c.kind) {
          case "mergeKeys":
            summary = t("lorebookGen.flow.coherenceMergeKeys", {
              keys: c.removeKeys.join(", "),
              entry: c.entryIdx + 1,
              reason: c.reason,
            });
            break;
          case "renameTerm":
            summary = t("lorebookGen.flow.coherenceRenameTerm", {
              oldTerm: c.oldTerm,
              newTerm: c.newTerm,
              entries:
                c.affectedEntryIdxs.length === 0
                  ? t("lorebookGen.flow.coherenceAll")
                  : c.affectedEntryIdxs.map((n) => n + 1).join(", "),
              reason: c.reason,
            });
            break;
          case "flagContradiction":
            summary = t("lorebookGen.flow.coherenceContradiction", {
              entries: c.entryIdxs.map((n) => n + 1).join(", "),
              description: c.description,
            });
            break;
          case "toggleAlwaysActive":
            summary = t("lorebookGen.flow.coherenceToggleAlwaysActive", {
              entry: c.entryIdx + 1,
              value: String(c.newValue),
              reason: c.reason,
            });
            break;
        }
        return (
          <label
            key={c.id}
            className="flex cursor-pointer items-start gap-3 rounded-lg border border-fg/10 bg-surface-el/20 p-2.5 transition hover:bg-fg/[0.04]"
          >
            <input
              type="checkbox"
              checked={checked}
              onChange={() => toggle(c.id)}
              className="mt-1 h-4 w-4 accent-accent"
            />
            <div className="flex-1 text-xs leading-relaxed text-fg/80">
              <span className="mr-2 rounded bg-fg/10 px-1.5 py-0.5 text-[10px] font-semibold uppercase text-fg/60">
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
          className={cn(btnSecondary, "flex-1 py-2 text-xs")}
        >
          {t("lorebookGen.flow.skip")}
        </button>
        <motion.button
          type="button"
          onClick={onApply}
          disabled={busy}
          whileTap={{ scale: 0.98 }}
          className={cn(btnPrimary, "flex-1 py-2 text-xs")}
        >
          {busy && <Loader2 className="h-3.5 w-3.5 animate-spin" />} {t("lorebookGen.flow.applySelected")}
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
  const { t } = useI18n();
  return (
    <div className="space-y-5">
      <p className="text-sm text-fg/55">
        {t("lorebookGen.flow.commitIntro", { count: draftCount })}
      </p>
      <div className={groupCardClass}>
        <div className="px-4 py-3">
          <Field label={t("lorebookGen.flow.destination")}>
            <select
              value={commitTarget}
              onChange={(e) => setCommitTarget(e.target.value)}
              className={inputClass}
            >
              <option value="">{t("lorebookGen.flow.createNewLorebook", { name: newLorebookName })}</option>
              {lorebooks.map((lb) => (
                <option key={lb.id} value={lb.id}>
                  {t("lorebookGen.flow.appendTo", { name: lb.name })}
                </option>
              ))}
            </select>
          </Field>
        </div>
      </div>
      <div className="flex gap-3 pt-2">
        <button
          type="button"
          onClick={onBack}
          className={cn(btnSecondary, "h-11 flex-1")}
        >
          {t("common.buttons.back")}
        </button>
        <motion.button
          type="button"
          onClick={onCommit}
          disabled={busy}
          whileTap={{ scale: 0.98 }}
          className={cn(btnPrimary, "h-11 flex-1")}
        >
          {busy ? <Loader2 className="h-4 w-4 animate-spin" /> : <CheckCircle2 className="h-4 w-4" />}
          {t("common.buttons.save")}
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
  const { t } = useI18n();
  return (
    <BottomMenu isOpen={isOpen} onClose={onClose} title={t("lorebookGen.flow.askForChangesTitle", { title: draftTitle })}>
      <div className="space-y-3">
        <textarea
          autoFocus
          value={feedback}
          onChange={(e) => setFeedback(e.target.value)}
          rows={5}
          placeholder={t("lorebookGen.flow.describeChangesPlaceholder")}
          className={inputClass}
        />
        <div className="flex gap-3">
          <button
            type="button"
            onClick={onClose}
            className={cn(btnSecondary, "flex-1 py-2.5")}
          >
            {t("common.buttons.cancel")}
          </button>
          <motion.button
            type="button"
            onClick={onSubmit}
            disabled={busy || !feedback.trim()}
            whileTap={{ scale: 0.98 }}
            className={cn(btnPrimary, "flex-1 py-2.5")}
          >
            {busy && <Loader2 className="h-4 w-4 animate-spin" />} {t("lorebookGen.flow.apply")}
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
  const { t } = useI18n();
  const [title, setTitle] = useState(initial.title);
  const [keywordsText, setKeywordsText] = useState(initial.keywords.join(", "));
  const [content, setContent] = useState(initial.content);
  const [alwaysActive, setAlwaysActive] = useState(initial.alwaysActive);

  return (
    <div className="fixed inset-0 z-30 flex items-center justify-center bg-black/60 px-4 pt-[var(--titlebar-h,0px)]">
      <div className="max-h-[80vh] w-full max-w-xl space-y-3 overflow-y-auto rounded-xl border border-fg/10 bg-surface p-5">
        <h3 className="text-sm font-semibold">{t("lorebookGen.flow.editEntry")}</h3>
        <input
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          placeholder={t("lorebookGen.flow.titleLabel")}
          className={inputClass}
        />
        <input
          value={keywordsText}
          onChange={(e) => setKeywordsText(e.target.value)}
          placeholder={t("lorebookGen.flow.commaSeparatedKeywords")}
          className={inputClass}
        />
        <textarea
          value={content}
          onChange={(e) => setContent(e.target.value)}
          rows={8}
          className={inputClass}
        />
        <label className="flex items-center gap-2 text-xs">
          <input
            type="checkbox"
            checked={alwaysActive}
            onChange={(e) => setAlwaysActive(e.target.checked)}
            className="accent-accent"
          />
          {t("lorebookGen.flow.alwaysActiveNoKeyword")}
        </label>
        <div className="flex gap-3">
          <button
            type="button"
            onClick={onCancel}
            className={cn(btnSecondary, "flex-1 py-2.5")}
          >
            {t("common.buttons.cancel")}
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
            className={cn(btnPrimary, "flex-1 py-2.5")}
          >
            {busy && <Loader2 className="h-4 w-4 animate-spin" />} {t("common.buttons.save")}
          </button>
        </div>
      </div>
    </div>
  );
}
