import { useEffect, useMemo, useRef, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { AlertTriangle, Loader2, Play, Plus, Square, X } from "lucide-react";

import {
  kokoroPreview,
  kokoroValidateAssets,
  listAudioProviders,
  listUserVoices,
  upsertUserVoice,
  type AudioProvider,
  type KokoroInstalledVoice,
  type KokoroVoiceBlendEntry,
  type UserVoice,
} from "../../../core/storage/audioProviders";
import { cn } from "../../design-tokens";

const DEFAULT_PREVIEW_TEXT =
  "Hello! This is how I sound when speaking. I can read longer passages with warmth, clarity, and emotion.";

function voiceDisplayName(id: string): string {
  const stripped = id.includes("_") ? id.split("_").slice(1).join("_") : id;
  if (!stripped) return id;
  return stripped.charAt(0).toUpperCase() + stripped.slice(1);
}

type BlendVoice = { id: string; weight: number /* 0..100 */ };

// Map [0..100] slider position to [0.5..2.0] speed with 1.0 at the center.
function sliderToSpeed(pos: number): number {
  const p = Math.max(0, Math.min(100, pos));
  const raw = p <= 50 ? 0.5 + (p / 50) * 0.5 : 1 + ((p - 50) / 50) * 1;
  return Math.round(raw * 20) / 20; 
}

function speedToSlider(speed: number): number {
  const s = Math.max(0.5, Math.min(2, speed));
  if (s <= 1) return ((s - 0.5) / 0.5) * 50;
  return 50 + ((s - 1) / 1) * 50;
}

function parseStored(voiceId: string): BlendVoice[] {
  const trimmed = voiceId.trim();
  if (!trimmed) return [];
  if (!trimmed.startsWith("[")) {
    return [{ id: trimmed, weight: 100 }];
  }
  try {
    const parsed = JSON.parse(trimmed) as Array<{ voiceId?: string; weight?: number }>;
    if (!Array.isArray(parsed)) return [];
    const seen = new Set<string>();
    const out: BlendVoice[] = [];
    for (const e of parsed) {
      if (!e || typeof e.voiceId !== "string" || seen.has(e.voiceId)) continue;
      seen.add(e.voiceId);
      out.push({
        id: e.voiceId,
        weight: Math.round(((typeof e.weight === "number" ? e.weight : 0) || 0) * 100),
      });
    }
    return out;
  } catch {
    return [];
  }
}

export function KokoroBlendEditorPage() {
  const navigate = useNavigate();
  const { providerId, blendId } = useParams<{ providerId: string; blendId?: string }>();

  const [provider, setProvider] = useState<AudioProvider | null>(null);
  const [installedVoices, setInstalledVoices] = useState<KokoroInstalledVoice[]>([]);
  const [name, setName] = useState("");
  const [voices, setVoices] = useState<BlendVoice[]>([]);
  const [speed, setSpeed] = useState(1);
  const [previewText, setPreviewText] = useState(DEFAULT_PREVIEW_TEXT);
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [isPreviewing, setIsPreviewing] = useState(false);
  const [isPlaying, setIsPlaying] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [existing, setExisting] = useState<UserVoice | null>(null);
  const [pickerOpen, setPickerOpen] = useState(false);

  const audioRef = useRef<HTMLAudioElement | null>(null);

  useEffect(() => {
    if (!providerId) return;
    void (async () => {
      setIsLoading(true);
      try {
        const list = await listAudioProviders();
        const found = list.find((p) => p.id === providerId) ?? null;
        setProvider(found);

        if (found?.assetRoot && found.kokoroVariant) {
          const status = await kokoroValidateAssets(found.assetRoot, found.kokoroVariant);
          setInstalledVoices(status.installedVoices);
        }

        if (blendId) {
          const all = await listUserVoices();
          const target = all.find((v) => v.id === blendId) ?? null;
          if (target) {
            setExisting(target);
            setName(target.name);
            setVoices(parseStored(target.voiceId));
            try {
              if (target.prompt) {
                const meta = JSON.parse(target.prompt);
                if (meta && typeof meta.speed === "number" && meta.speed > 0) {
                  setSpeed(meta.speed);
                }
              }
            } catch { }
          }
        }
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      } finally {
        setIsLoading(false);
      }
    })();
  }, [providerId, blendId]);

  useEffect(() => {
    return () => {
      const audio = audioRef.current;
      if (audio) {
        audio.pause();
        audio.removeAttribute("src");
        audio.load();
      }
    };
  }, []);

  const blendForApi: KokoroVoiceBlendEntry[] = useMemo(
    () =>
      voices
        .filter((v) => v.weight > 0)
        .map((v) => ({ voiceId: v.id, weight: v.weight / 100 })),
    [voices],
  );

  const availableToAdd = useMemo(() => {
    const inBlend = new Set(voices.map((v) => v.id));
    return installedVoices.filter((v) => !inBlend.has(v.id));
  }, [installedVoices, voices]);

  const canTest =
    !isPreviewing &&
    Boolean(provider?.assetRoot && provider.kokoroVariant) &&
    blendForApi.length > 0 &&
    previewText.trim().length > 0;

  const canSave = !isSaving && name.trim().length > 0 && blendForApi.length > 0;

  const handleAddVoice = (voiceId: string) => {
    setVoices((prev) => {
      if (prev.some((v) => v.id === voiceId)) return prev;
      return [...prev, { id: voiceId, weight: 50 }];
    });
    setPickerOpen(false);
  };

  const handleUpdateWeight = (voiceId: string, nextWeight: number) => {
    const clamped = Math.max(0, Math.min(100, Math.round(nextWeight)));
    setVoices((prev) => prev.map((v) => (v.id === voiceId ? { ...v, weight: clamped } : v)));
  };

  const handleRemoveVoice = (voiceId: string) => {
    setVoices((prev) => prev.filter((v) => v.id !== voiceId));
  };

  const handleStop = () => {
    const audio = audioRef.current;
    if (audio) {
      audio.pause();
      audio.removeAttribute("src");
      audio.load();
    }
    setIsPlaying(false);
  };

  const handleTest = async () => {
    if (isPlaying) {
      handleStop();
      return;
    }
    if (!provider?.assetRoot || !provider.kokoroVariant || blendForApi.length === 0) return;
    setError(null);
    setIsPreviewing(true);
    try {
      const response = await kokoroPreview(
        provider.assetRoot,
        provider.kokoroVariant,
        blendForApi,
        previewText.trim(),
        speed,
      );
      const url = `data:${response.format};base64,${response.audioBase64}`;
      const existingAudio = audioRef.current;
      if (existingAudio) existingAudio.pause();
      const audio = new Audio(url);
      audioRef.current = audio;
      audio.addEventListener("play", () => setIsPlaying(true));
      audio.addEventListener("pause", () => setIsPlaying(false));
      audio.addEventListener("ended", () => setIsPlaying(false));
      audio.addEventListener("error", () => setIsPlaying(false));
      await audio.play();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setIsPreviewing(false);
    }
  };

  const handleSave = async () => {
    if (!providerId || !provider?.kokoroVariant) return;
    if (!name.trim() || blendForApi.length === 0) return;

    setIsSaving(true);
    setError(null);
    try {
      const promptPayload = JSON.stringify({ speed });
      const payload: UserVoice = {
        id: existing?.id ?? "",
        providerId,
        name: name.trim(),
        modelId: provider.kokoroVariant,
        voiceId: JSON.stringify(blendForApi),
        prompt: promptPayload,
      };
      await upsertUserVoice(payload);
      navigate(-1);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setIsSaving(false);
    }
  };

  if (isLoading) {
    return (
      <div className="flex h-64 items-center justify-center">
        <Loader2 className="h-8 w-8 animate-spin text-fg/40" />
      </div>
    );
  }

  if (!provider) {
    return (
      <div className="flex h-64 flex-col items-center justify-center gap-3">
        <AlertTriangle className="h-10 w-10 text-warning/60" />
        <p className="text-sm text-fg/60">Provider not found</p>
        <button
          onClick={() => navigate(-1)}
          className="rounded-lg border border-fg/10 bg-fg/5 px-4 py-2 text-sm text-fg/70 transition hover:border-fg/20 hover:bg-fg/10"
        >
          Back
        </button>
      </div>
    );
  }

  if (installedVoices.length === 0) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-3 px-4 text-center">
        <AlertTriangle className="h-10 w-10 text-warning/60" />
        <p className="text-sm text-fg/70">Install at least one voice first</p>
        <p className="text-xs text-fg/45">A blend needs installed voices to mix.</p>
        <button
          onClick={() => navigate(`/settings/voices/kokoro/${providerId}`)}
          className="mt-2 rounded-lg border border-accent/30 bg-accent/10 px-4 py-2 text-sm text-accent/90 transition hover:border-accent/50 hover:bg-accent/20"
        >
          Open voice library
        </button>
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col">
      <div className="flex-1 overflow-y-auto px-4 py-4 pb-[calc(env(safe-area-inset-bottom)+88px)]">
        {error && (
          <div className="mb-4 flex items-start gap-2 rounded-lg border border-danger/30 bg-danger/10 px-3 py-2">
            <AlertTriangle className="mt-0.5 h-3.5 w-3.5 shrink-0 text-danger/85" />
            <p className="flex-1 text-[12px] text-danger/85">{error}</p>
            <button
              onClick={() => setError(null)}
              className="text-danger/60 transition hover:text-danger"
            >
              <X className="h-3.5 w-3.5" />
            </button>
          </div>
        )}

        <div className="grid gap-6 lg:grid-cols-[minmax(0,1fr)_minmax(0,420px)]">
          {/* Composer */}
          <div className="min-w-0 space-y-5">
            <div>
              <label className="mb-1.5 block text-[10px] font-bold uppercase tracking-wider text-fg/40">
                Name
              </label>
              <input
                type="text"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="My Character Voice"
                className="w-full rounded-lg border border-fg/10 bg-surface-el/30 px-3 py-2 text-[13px] text-fg placeholder-fg/40 focus:border-fg/25 focus:outline-none"
              />
            </div>

            <div>
              <div className="mb-2 flex items-center justify-between gap-3 px-0.5">
                <div>
                  <h2 className="text-[10px] font-bold uppercase tracking-wider text-fg/40">
                    Voices
                  </h2>
                  <p className="mt-0.5 text-[11px] text-fg/45">
                    {voices.length === 0
                      ? "Add at least one voice to begin"
                      : "Drag the slider or click the value to edit"}
                  </p>
                </div>
                <div className="relative">
                  <button
                    onClick={() => setPickerOpen((v) => !v)}
                    disabled={availableToAdd.length === 0}
                    className="flex items-center gap-1 rounded-full border border-accent/30 bg-accent/10 px-3 py-1 text-[11px] font-medium text-accent/90 transition hover:border-accent/50 hover:bg-accent/15 disabled:opacity-40"
                  >
                    <Plus className="h-3 w-3" />
                    Add voice
                  </button>
                  {pickerOpen && availableToAdd.length > 0 && (
                    <>
                      <div
                        className="fixed inset-0 z-30"
                        onClick={() => setPickerOpen(false)}
                      />
                      <div className="absolute right-0 top-full z-40 mt-1 max-h-64 w-56 overflow-y-auto rounded-lg border border-fg/15 bg-surface-el/95 py-1 shadow-lg backdrop-blur">
                        {availableToAdd.map((voice) => (
                          <button
                            key={voice.id}
                            onClick={() => handleAddVoice(voice.id)}
                            className="flex w-full items-center justify-between gap-2 px-3 py-1.5 text-left transition hover:bg-fg/10"
                          >
                            <span className="truncate text-[12px] font-medium text-fg/85">
                              {voiceDisplayName(voice.id)}
                            </span>
                            <span className="font-mono text-[10px] text-fg/35">{voice.id}</span>
                          </button>
                        ))}
                      </div>
                    </>
                  )}
                </div>
              </div>

              {voices.length === 0 ? (
                <div className="rounded-xl border border-dashed border-fg/15 bg-fg/2.5 px-4 py-8 text-center">
                  <p className="text-[12px] text-fg/55">No voices added yet</p>
                  <button
                    onClick={() => setPickerOpen(true)}
                    className="mt-2 inline-flex items-center gap-1 text-[11px] font-medium text-accent/85 hover:text-accent"
                  >
                    <Plus className="h-3 w-3" />
                    Add your first voice
                  </button>
                </div>
              ) : (
                <div className="space-y-1.5">
                  {voices.map((voice) => (
                    <BlendVoiceRow
                      key={voice.id}
                      voice={voice}
                      onChange={(w) => handleUpdateWeight(voice.id, w)}
                      onRemove={() => handleRemoveVoice(voice.id)}
                    />
                  ))}
                </div>
              )}
            </div>

            <div>
              <div className="mb-1.5 flex items-center justify-between px-0.5">
                <label className="text-[10px] font-bold uppercase tracking-wider text-fg/40">
                  Speed
                </label>
                <SpeedValue value={speed} onChange={setSpeed} />
              </div>
              <div className="rounded-xl border border-fg/10 bg-fg/5 px-4 py-3">
                <input
                  type="range"
                  min={0}
                  max={100}
                  step={1}
                  value={speedToSlider(speed)}
                  onChange={(e) => setSpeed(sliderToSpeed(Number(e.target.value)))}
                  className="w-full accent-accent"
                />
                <div className="mt-1 flex justify-between text-[9px] tabular-nums text-fg/35">
                  <span>0.5×</span>
                  <span>1.0×</span>
                  <span>2.0×</span>
                </div>
              </div>
            </div>
          </div>

          {/* Test bench */}
          <div className="min-w-0 space-y-3">
            <div className="px-0.5">
              <h2 className="text-[10px] font-bold uppercase tracking-wider text-fg/40">
                Test
              </h2>
              <p className="mt-0.5 text-[11px] text-fg/45">
                Hear the blend before you save it.
              </p>
            </div>
            <div className="overflow-hidden rounded-xl border border-fg/10 bg-fg/4">
              <textarea
                value={previewText}
                onChange={(e) => setPreviewText(e.target.value)}
                rows={5}
                placeholder="Type a phrase to test…"
                className="w-full resize-none border-0 bg-transparent px-4 pt-3 pb-1 text-[13px] leading-relaxed text-fg placeholder-fg/35 focus:outline-none"
              />
              <div className="flex items-center justify-between gap-2 border-t border-fg/10 bg-fg/2 px-3 py-2">
                <span className="text-[10px] text-fg/40">
                  {blendForApi.length === 0
                    ? "No active voices"
                    : `${blendForApi.length} voice${blendForApi.length === 1 ? "" : "s"} · ${speed.toFixed(2)}×`}
                </span>
                <button
                  onClick={() => void handleTest()}
                  disabled={!canTest && !isPlaying}
                  className={cn(
                    "inline-flex items-center gap-1.5 rounded-full px-4 py-1.5 text-[11px] font-semibold transition active:scale-[0.98] disabled:opacity-50",
                    isPlaying
                      ? "border border-fg/20 bg-fg/10 text-fg/85 hover:bg-fg/15"
                      : "bg-accent text-bg hover:brightness-110",
                  )}
                >
                  {isPreviewing ? (
                    <Loader2 className="h-3 w-3 animate-spin" />
                  ) : isPlaying ? (
                    <Square className="h-3 w-3" fill="currentColor" />
                  ) : (
                    <Play className="h-3 w-3" fill="currentColor" />
                  )}
                  {isPreviewing
                    ? "Generating…"
                    : isPlaying
                      ? "Stop"
                      : "Test blend"}
                </button>
              </div>
            </div>

            <div className="rounded-xl border border-fg/10 bg-fg/3 px-4 py-3 text-[11px] text-fg/55">
              <p className="font-medium text-fg/70">Tips</p>
              <ul className="mt-1.5 space-y-1 text-fg/50">
                <li>· Higher weights = more of that voice in the mix.</li>
                <li>· Click any number to type an exact value.</li>
                <li>· Voices with weight 0 are skipped on save.</li>
              </ul>
            </div>
          </div>
        </div>
      </div>

      {/* Sticky action bar */}
      <div className="fixed inset-x-0 bottom-0 z-40 border-t border-fg/10 bg-surface-el/85 px-4 py-3 pb-[calc(env(safe-area-inset-bottom)+12px)] backdrop-blur">
        <div className="mx-auto flex max-w-3xl items-center justify-between gap-3">
          <p className="truncate text-[11px] text-fg/50">
            {voices.length === 0
              ? "Add voices to get started"
              : blendForApi.length === 0
                ? "All weights are 0"
                : `${blendForApi.length} active · ${voices.length} total · ${speed.toFixed(2)}×`}
          </p>
          <div className="flex items-center gap-2">
            <button
              onClick={() => navigate(-1)}
              className="rounded-full border border-fg/10 bg-fg/5 px-4 py-1.5 text-[12px] font-medium text-fg/75 transition hover:border-fg/20 hover:bg-fg/10"
            >
              Cancel
            </button>
            <button
              onClick={() => void handleSave()}
              disabled={!canSave}
              className="rounded-full bg-accent px-4 py-1.5 text-[12px] font-semibold text-bg transition hover:brightness-110 active:scale-[0.98] disabled:opacity-50"
            >
              {isSaving ? "Saving…" : existing ? "Save" : "Create"}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

function BlendVoiceRow({
  voice,
  onChange,
  onRemove,
}: {
  voice: BlendVoice;
  onChange: (next: number) => void;
  onRemove: () => void;
}) {
  return (
    <div className="rounded-xl border border-fg/10 bg-fg/5 px-3 py-2.5 transition hover:border-fg/20 hover:bg-fg/[0.07]">
      <div className="mb-2 flex items-start gap-2">
        <div className="min-w-0 flex-1">
          <p className="truncate text-[13px] font-semibold text-fg leading-tight">
            {voiceDisplayName(voice.id)}
          </p>
          <p className="truncate font-mono text-[10px] text-fg/45 leading-tight mt-0.5">
            {voice.id}
          </p>
        </div>
        <button
          onClick={onRemove}
          className="flex h-6 w-6 shrink-0 items-center justify-center rounded-full text-fg/40 transition hover:bg-danger/10 hover:text-danger"
          aria-label="Remove"
        >
          <X className="h-3.5 w-3.5" />
        </button>
      </div>
      <div className="flex items-center gap-3">
        <input
          type="range"
          min={0}
          max={100}
          step={1}
          value={voice.weight}
          onChange={(e) => onChange(Number(e.target.value))}
          className="h-1 flex-1 accent-accent"
        />
        <EditableNumber
          value={voice.weight}
          min={0}
          max={100}
          onChange={onChange}
          className="w-12"
        />
      </div>
    </div>
  );
}

function EditableNumber({
  value,
  min,
  max,
  step = 1,
  onChange,
  format,
  className,
}: {
  value: number;
  min: number;
  max: number;
  step?: number;
  onChange: (next: number) => void;
  format?: (v: number) => string;
  className?: string;
}) {
  const [draft, setDraft] = useState<string | null>(null);
  const display = format ? format(value) : value.toString();

  const commit = () => {
    if (draft == null) return;
    const cleaned = draft.replace(/[^\d.-]/g, "");
    const parsed = Number(cleaned);
    if (Number.isFinite(parsed)) {
      const clamped = Math.max(min, Math.min(max, parsed));
      onChange(clamped);
    }
    setDraft(null);
  };

  return (
    <input
      type="text"
      inputMode="decimal"
      value={draft ?? display}
      onChange={(e) => setDraft(e.target.value)}
      onFocus={(e) => {
        setDraft(value.toString());
        requestAnimationFrame(() => e.currentTarget.select());
      }}
      onBlur={commit}
      onKeyDown={(e) => {
        if (e.key === "Enter") {
          commit();
          (e.currentTarget as HTMLInputElement).blur();
        } else if (e.key === "Escape") {
          setDraft(null);
          (e.currentTarget as HTMLInputElement).blur();
        } else if (e.key === "ArrowUp") {
          e.preventDefault();
          onChange(Math.min(max, value + step));
        } else if (e.key === "ArrowDown") {
          e.preventDefault();
          onChange(Math.max(min, value - step));
        }
      }}
      className={cn(
        "rounded-md border border-transparent bg-fg/5 px-2 py-0.5 text-right font-mono text-[11px] tabular-nums text-fg/85 transition hover:border-fg/15 focus:border-fg/25 focus:bg-fg/10 focus:outline-none",
        className,
      )}
    />
  );
}

function SpeedValue({
  value,
  onChange,
}: {
  value: number;
  onChange: (next: number) => void;
}) {
  return (
    <EditableNumber
      value={value}
      min={0.5}
      max={2}
      step={0.05}
      onChange={onChange}
      format={(v) => `${v.toFixed(2)}×`}
      className="w-16"
    />
  );
}
