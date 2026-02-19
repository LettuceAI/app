import { useMemo, useState } from "react";
import { motion } from "framer-motion";
import { Upload, FileCode, ArrowRight, Loader2, CheckCircle2 } from "lucide-react";
import { cn, typography, radius, interactive } from "../../design-tokens";
import { convertImportToFormat, convertImportToUec } from "../../../core/storage/genericTransfer";
import {
  readFileAsText,
  downloadJson,
  generateExportFilenameWithFormat,
  type CharacterFileFormat,
} from "../../../core/storage/characterTransfer";
import { generateExportFilename as generatePersonaFilename } from "../../../core/storage/personaTransfer";
import { toast } from "../../components/toast";

type UecKind = "character" | "persona";
type DetectedKind = UecKind | "unknown";
type DetectedFormat = CharacterFileFormat | "unknown";

const TARGET_LABELS: Record<
  Exclude<CharacterFileFormat, "legacy_json" | "chara_card_v1">,
  string
> = {
  uec: "Unified Entity Card (UEC)",
  chara_card_v3: "Character Card V3",
  chara_card_v2: "Character Card V2",
};

function formatBytes(bytes: number): string {
  if (!bytes) return "0 B";
  const k = 1024;
  const sizes = ["B", "KB", "MB", "GB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
}

export function ConvertPage() {
  const [selectedFile, setSelectedFile] = useState<File | null>(null);
  const [converting, setConverting] = useState(false);
  const [convertedFilename, setConvertedFilename] = useState<string | null>(null);
  const [convertedKind, setConvertedKind] = useState<UecKind | null>(null);
  const [detectedKind, setDetectedKind] = useState<DetectedKind>("unknown");
  const [detectedFormat, setDetectedFormat] = useState<DetectedFormat>("unknown");
  const [targetFormat, setTargetFormat] =
    useState<Exclude<CharacterFileFormat, "legacy_json" | "chara_card_v1">>("uec");
  const [fileContents, setFileContents] = useState<string | null>(null);

  const detectImportInfo = (contents: string) => {
    try {
      const parsed = JSON.parse(contents);
      if (parsed?.schema?.name === "UEC") {
        return {
          kind: parsed.kind === "persona" ? ("persona" as const) : ("character" as const),
          format: "uec" as const,
        };
      }
      if (parsed?.spec === "chara_card_v3") {
        return { kind: "character" as const, format: "chara_card_v3" as const };
      }
      if (parsed?.spec === "chara_card_v2") {
        return { kind: "character" as const, format: "chara_card_v2" as const };
      }
      if (
        typeof parsed?.name === "string" &&
        typeof parsed?.description === "string" &&
        typeof parsed?.personality === "string" &&
        typeof parsed?.scenario === "string" &&
        typeof parsed?.first_mes === "string" &&
        typeof parsed?.mes_example === "string"
      ) {
        return { kind: "character" as const, format: "chara_card_v1" as const };
      }
      if (parsed?.type === "persona") {
        return { kind: "persona" as const, format: "legacy_json" as const };
      }
      if (parsed?.type === "character") {
        return { kind: "character" as const, format: "legacy_json" as const };
      }
    } catch (err) {
      console.warn("[ConvertPage] Failed to detect import metadata:", err);
    }
    return { kind: "unknown" as const, format: "unknown" as const };
  };

  const handleFileChange = async (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (!file) return;
    setSelectedFile(file);
    setConvertedFilename(null);
    setConvertedKind(null);
    setDetectedKind("unknown");
    setDetectedFormat("unknown");
    setTargetFormat("uec");
    setFileContents(null);
    try {
      const contents = await readFileAsText(file);
      setFileContents(contents);
      const detected = detectImportInfo(contents);
      setDetectedKind(detected.kind);
      setDetectedFormat(detected.format);
      if (detected.kind === "character" && detected.format === "chara_card_v3") {
        setTargetFormat("chara_card_v2");
      } else {
        setTargetFormat("uec");
      }
    } catch (error) {
      console.error("Failed to read file:", error);
      toast.error("Unable to read the selected file.");
    }
    event.target.value = "";
  };

  const availableTargets = useMemo(() => {
    if (detectedKind !== "character") return [];
    if (detectedFormat === "chara_card_v3") return ["uec", "chara_card_v2"] as const;
    if (detectedFormat === "uec") return [];
    return ["uec"] as const;
  }, [detectedKind, detectedFormat]);

  const isPersonaUec = detectedKind === "persona" && detectedFormat === "uec";
  const isAlreadyUec = detectedKind === "character" && detectedFormat === "uec";
  const canConvert = Boolean(selectedFile) && !converting && availableTargets.length > 0;

  const handleConvert = async () => {
    if (!selectedFile || converting || !canConvert) return;
    try {
      setConverting(true);
      const contents = fileContents ?? (await readFileAsText(selectedFile));
      let converted = "";
      if (targetFormat === "uec") {
        converted = await convertImportToUec(contents);
      } else {
        converted = await convertImportToFormat(contents, targetFormat);
      }
      const parsed = JSON.parse(converted) as { kind?: string; payload?: Record<string, any> };
      const kind = parsed.kind === "persona" ? "persona" : "character";
      const title =
        kind === "persona" ? (parsed.payload?.title ?? "") : (parsed.payload?.name ?? "");
      const baseName = selectedFile.name.replace(/\.[^.]+$/, "") || kind;
      const filename =
        kind === "persona"
          ? generatePersonaFilename(title || baseName)
          : generateExportFilenameWithFormat(title || baseName, targetFormat);

      await downloadJson(converted, filename);
      setConvertedFilename(filename);
      setConvertedKind(kind);
      toast.success("Conversion complete", `Saved ${filename}`);
    } catch (error: any) {
      console.error("Failed to convert file:", error);
      toast.error("Conversion failed", error?.message || "Unable to convert this file");
    } finally {
      setConverting(false);
    }
  };

  return (
    <div className="flex h-full flex-col pb-16 text-fg/90">
      <main className="flex-1 overflow-y-auto px-4 pt-4">
        <motion.div
          initial={{ opacity: 0, y: 12 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.2, ease: "easeOut" }}
          className="space-y-4"
        >
          <div className="rounded-2xl border border-warning/30 bg-warning/10 p-4">
            <div className="flex items-start gap-3">
              <div className="flex h-9 w-9 items-center justify-center rounded-full border border-warning/40 bg-warning/15">
                <FileCode className="h-4 w-4 text-warning/80" />
              </div>
              <div>
                <h2 className={cn(typography.body.size, "font-semibold text-warning/90")}>
                  Convert legacy exports
                </h2>
                <p className={cn(typography.caption.size, "mt-1 text-warning/70")}>
                  JSON exports are deprecated. Convert them to Unified Entity Card (UEC) files for
                  future compatibility.
                </p>
              </div>
            </div>
          </div>

          <div className="rounded-2xl border border-fg/10 bg-fg/5 p-4">
            <div className="flex items-center justify-between gap-3">
              <div>
                <h3 className={cn(typography.body.size, "text-fg font-medium")}>
                  Choose a file
                </h3>
                <p className={cn(typography.caption.size, "text-fg/50")}>
                  Supports legacy .json exports and existing Unified Entity Card (UEC) files.
                </p>
              </div>
              <label
                className={cn(
                  "inline-flex items-center gap-2 px-3 py-2",
                  radius.md,
                  "border border-info/40 bg-info/15 text-info",
                  interactive.transition.default,
                  "hover:border-info/60 hover:bg-info/25",
                )}
              >
                <Upload className="h-4 w-4" />
                Browse
                <input
                  type="file"
                  accept="application/json,.json,.uec"
                  onChange={handleFileChange}
                  className="hidden"
                />
              </label>
            </div>

            {selectedFile ? (
              <div className="mt-4 flex flex-col gap-3 rounded-xl border border-fg/10 bg-surface-el/30 px-3 py-2">
                <div className="flex items-start justify-between gap-3">
                  <div className="min-w-0">
                    <p className={cn(typography.caption.size, "truncate text-fg/70")}>
                      {selectedFile.name}
                    </p>
                    <p className={cn(typography.caption.size, "text-fg/35")}>
                      {formatBytes(selectedFile.size)}
                    </p>
                  </div>
                </div>

                <div className="flex flex-col gap-1 rounded-lg border border-fg/10 bg-fg/5 px-3 py-2 text-[11px] text-fg/60">
                  <div className="flex items-center justify-between">
                    <span>Detected type</span>
                    <span className="text-fg/80">
                      {detectedKind === "unknown" ? "Unknown" : detectedKind}
                    </span>
                  </div>
                  <div className="flex items-center justify-between">
                    <span>Detected format</span>
                    <span className="text-fg/80">
                      {detectedFormat === "unknown"
                        ? "Unknown"
                        : (TARGET_LABELS[detectedFormat as keyof typeof TARGET_LABELS] ??
                          detectedFormat)}
                    </span>
                  </div>
                </div>
              </div>
            ) : (
              <div className="mt-4 rounded-xl border border-dashed border-fg/15 bg-fg/5 px-4 py-6 text-center">
                <p className={cn(typography.caption.size, "text-fg/40")}>
                  No file selected yet
                </p>
              </div>
            )}
          </div>

          <div className="rounded-2xl border border-fg/10 bg-fg/5 p-4">
            <div>
              <h3 className={cn(typography.body.size, "text-fg font-medium")}>
                Output format
              </h3>
              <p className={cn(typography.caption.size, "text-fg/50")}>
                Choose how the converted file should be saved.
              </p>
            </div>

            {availableTargets.length > 0 ? (
              <div className="mt-3 flex flex-wrap gap-2">
                {availableTargets.map((format) => (
                  <button
                    key={format}
                    onClick={() => setTargetFormat(format)}
                    className={cn(
                      "rounded-full border px-3 py-1 text-xs font-medium transition",
                      targetFormat === format
                        ? "border-accent/60 bg-accent/20 text-accent/90"
                        : "border-fg/10 bg-fg/5 text-fg/60 hover:border-fg/20 hover:text-fg",
                    )}
                  >
                    {TARGET_LABELS[format]}
                  </button>
                ))}
              </div>
            ) : (
              <div className="mt-3 rounded-lg border border-fg/10 bg-surface-el/20 px-3 py-2 text-xs text-fg/60">
                {isPersonaUec
                  ? "Persona UEC files are already in the only supported persona format."
                  : isAlreadyUec
                    ? "This file is already a UEC export."
                    : selectedFile
                      ? "Unable to convert this file."
                      : "Select a file to see output options."}
              </div>
            )}

            <button
              onClick={handleConvert}
              disabled={!canConvert}
              className={cn(
                "mt-4 flex w-full items-center justify-center gap-2 py-3 text-sm font-semibold",
                radius.md,
                interactive.transition.default,
                "border border-accent/40 bg-accent/20 text-accent/90",
                "hover:border-accent/60 hover:bg-accent/30",
                "disabled:cursor-not-allowed disabled:opacity-50",
              )}
            >
              {converting ? (
                <>
                  <Loader2 className="h-4 w-4 animate-spin" />
                  Converting...
                </>
              ) : (
                <>
                  <ArrowRight className="h-4 w-4" />
                  {targetFormat === "uec"
                    ? "Convert to Unified Entity Card (UEC)"
                    : `Convert to ${TARGET_LABELS[targetFormat]}`}
                </>
              )}
            </button>

            {convertedFilename && (
              <div className="mt-3 flex items-center gap-2 rounded-xl border border-accent/20 bg-accent/10 px-3 py-2">
                <CheckCircle2 className="h-4 w-4 text-accent/80" />
                <p className={cn(typography.caption.size, "text-accent/80")}>
                  Saved {convertedKind} as {convertedFilename}
                </p>
              </div>
            )}
          </div>
        </motion.div>
      </main>
    </div>
  );
}
