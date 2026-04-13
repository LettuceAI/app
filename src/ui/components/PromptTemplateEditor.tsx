import { useState, useEffect } from "react";
import { X, Eye, Code2, BookTemplate, Check } from "lucide-react";
import { cn } from "../design-tokens";
import { BottomMenu } from "./BottomMenu";
import { createPromptTemplate, updatePromptTemplate } from "../../core/prompts/service";
import { renderPromptPreview } from "../../core/prompts/service";
import { listCharacters, listPersonas } from "../../core/storage";
import type { SystemPromptTemplate, Character, Persona } from "../../core/storage/schemas";
import { useI18n } from "../../core/i18n/context";

interface PromptTemplateEditorProps {
  template?: SystemPromptTemplate;
  onClose: () => void;
  onSave: () => void;
}

export function PromptTemplateEditor({ template, onClose, onSave }: PromptTemplateEditorProps) {
  const { t } = useI18n();
  const isEditing = !!template;

  const [name, setName] = useState(template?.name || "");
  const [content, setContent] = useState(template?.content || "");

  const [characters, setCharacters] = useState<Character[]>([]);
  const [saving, setSaving] = useState(false);
  const [personas, setPersonas] = useState<Persona[]>([]);
  const [previewCharacterId, setPreviewCharacterId] = useState<string | null>(null);
  const [previewPersonaId, setPreviewPersonaId] = useState<string | null>(null);
  const [preview, setPreview] = useState<string>("");
  const [previewing, setPreviewing] = useState(false);
  const [previewMode, setPreviewMode] = useState<"rendered" | "raw">("rendered");
  const [showVariables, setShowVariables] = useState(false);
  const [copiedVar, setCopiedVar] = useState<string | null>(null);

  useEffect(() => {
    loadData();
  }, []);

  async function loadData() {
    try {
      const chars = await listCharacters();
      setCharacters(chars);
      setPreviewCharacterId(chars[0]?.id ?? null);

      const pers = await listPersonas();
      setPersonas(pers);
      setPreviewPersonaId(pers.find(p => p.isDefault)?.id ?? null);
    } catch (error) {
      console.error("Failed to load data:", error);
    } finally { }
  }

  async function handleSave() {
    if (!name.trim() || !content.trim()) {
      alert(t("components.promptTemplate.nameContentRequired"));
      return;
    }

    setSaving(true);
    try {
      if (isEditing) {
        await updatePromptTemplate(template.id, {
          name: name.trim(),
          content: content.trim(),
        });
      } else {
        await createPromptTemplate(name.trim(), "undefined", content.trim());
      }

      onSave();
      onClose();
    } catch (error) {
      console.error("Failed to save template:", error);
      alert(t("components.promptTemplate.saveError"));
    } finally {
      setSaving(false);
    }
  }

  async function handlePreview() {
    if (!previewCharacterId) return;
    setPreviewing(true);
    try {
      const rendered = await renderPromptPreview(content, {
        characterId: previewCharacterId,
        personaId: previewPersonaId ?? undefined,
      });
      setPreview(rendered);
    } catch (e) {
      console.error("Preview failed", e);
      setPreview("<failed to render preview>");
    } finally {
      setPreviewing(false);
    }
  }

  async function copyVariable(variable: string) {
    await navigator.clipboard.writeText(variable);
    setCopiedVar(variable);
    setTimeout(() => setCopiedVar(null), 2000);
  }

  const charCount = content.length;
  const charCountColor =
    charCount > 8000 ? "text-red-400" :
      charCount > 5000 ? "text-amber-400" :
        "text-white/40";

  const variables = [
    { var: "{{char.name}}", label: t("components.promptTemplate.variables.charName"), desc: t("components.promptTemplate.variables.charNameDesc") },
    { var: "{{char.desc}}", label: t("components.promptTemplate.variables.charDesc"), desc: t("components.promptTemplate.variables.charDescDesc") },
    { var: "{{scene}}", label: t("components.promptTemplate.variables.scene"), desc: t("components.promptTemplate.variables.sceneDesc") },
    { var: "{{persona.name}}", label: t("components.promptTemplate.variables.userName"), desc: t("components.promptTemplate.variables.userNameDesc") },
    { var: "{{persona.desc}}", label: t("components.promptTemplate.variables.userDesc"), desc: t("components.promptTemplate.variables.userDescDesc") },
    { var: "{{rules}}", label: t("components.promptTemplate.variables.rules"), desc: t("components.promptTemplate.variables.rulesDesc") },
    { var: "{{context_summary}}", label: t("components.promptTemplate.variables.contextSummary"), desc: t("components.promptTemplate.variables.contextSummaryDesc") },
    { var: "{{key_memories}}", label: t("components.promptTemplate.variables.keyMemories"), desc: t("components.promptTemplate.variables.keyMemoriesDesc") },
  ];

  return (
    <>
      <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm p-4">
        <div className="w-full max-w-2xl rounded-2xl border border-white/20 bg-[#0b0b0d] shadow-2xl">
          {/* Header */}
          <div className="flex items-center justify-between border-b border-white/10 px-5 py-4">
            <h2 className="text-lg font-semibold text-white">
              {isEditing ? t("components.promptTemplate.editTitle") : t("components.promptTemplate.createTitle")}
            </h2>
            <button
              onClick={onClose}
              className="rounded-lg border border-white/10 bg-white/5 p-2 text-white/70 transition hover:bg-white/10 hover:text-white"
            >
              <X className="h-4 w-4" />
            </button>
          </div>

          {/* Content */}
          <div className="max-h-[70vh] space-y-4 overflow-y-auto px-5 py-4">
            {/* Name Input */}
            <div className="space-y-2">
              <label className="text-xs font-semibold uppercase tracking-wider text-white/70">
                {t("components.promptTemplate.name")}
              </label>
              <input
                type="text"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder={t("components.promptTemplate.namePlaceholder")}
                className="w-full rounded-xl border border-white/10 bg-black/30 px-3.5 py-2.5 text-white placeholder-white/30 transition focus:border-blue-400/40 focus:bg-black/40 focus:outline-none"
              />
            </div>

            {/* Content Textarea */}
            <div className="space-y-2">
              <div className="flex items-center justify-between">
                <label className="text-xs font-semibold uppercase tracking-wider text-white/70">
                  {t("components.promptTemplate.content")}
                </label>
                <button
                  onClick={() => setShowVariables(true)}
                  className="flex items-center gap-1.5 rounded-lg border border-purple-400/30 bg-purple-400/10 px-2.5 py-1.5 text-xs font-medium text-purple-200 transition active:scale-95"
                >
                  <BookTemplate className="h-3 w-3" />
                  {t("components.promptTemplate.variablesButton")}
                </button>
              </div>

              <textarea
                value={content}
                onChange={(e) => setContent(e.target.value)}
                placeholder="You are a helpful AI assistant...&#10;&#10;Use {{char.name}} and {{scene}} in your prompt."
                rows={12}
                className="w-full resize-none rounded-xl border border-white/10 bg-black/30 px-3.5 py-2.5 font-mono text-sm leading-relaxed text-white placeholder-white/30 transition focus:border-blue-400/40 focus:bg-black/40 focus:outline-none"
              />

              <span className={cn("text-xs font-medium", charCountColor)}>
                {charCount.toLocaleString()} characters
              </span>
            </div>

            {/* Preview Section */}
            <div className="space-y-2">
              <div className="flex items-center justify-between">
                <label className="text-xs font-semibold uppercase tracking-wider text-white/70">
                  {t("components.promptTemplate.preview")}
                </label>
                <div className="flex items-center gap-1 rounded-lg border border-white/10 bg-black/20 p-0.5">
                  <button
                    onClick={() => setPreviewMode("rendered")}
                    className={cn(
                      "flex items-center gap-1 rounded-md px-2 py-1 text-xs font-medium transition",
                      previewMode === "rendered" ? "bg-blue-400/20 text-blue-200" : "text-white/50"
                    )}
                  >
                    <Eye className="h-3 w-3" />
                  </button>
                  <button
                    onClick={() => setPreviewMode("raw")}
                    className={cn(
                      "flex items-center gap-1 rounded-md px-2 py-1 text-xs font-medium transition",
                      previewMode === "raw" ? "bg-blue-400/20 text-blue-200" : "text-white/50"
                    )}
                  >
                    <Code2 className="h-3 w-3" />
                  </button>
                </div>
              </div>

              {previewMode === "rendered" && (
                <>
                  <div className="grid grid-cols-2 gap-2">
                    <select
                      value={previewCharacterId ?? ""}
                      onChange={(e) => setPreviewCharacterId(e.target.value || null)}
                      className="w-full rounded-lg border border-white/10 bg-black/30 px-3 py-2 text-sm text-white transition focus:border-blue-400/40 focus:outline-none"
                    >
                      <option value="">{t("components.promptTemplate.characterPlaceholder")}</option>
                      {characters.map((c) => (
                        <option key={c.id} value={c.id}>{c.name}</option>
                      ))}
                    </select>

                    <select
                      value={previewPersonaId ?? ""}
                      onChange={(e) => setPreviewPersonaId(e.target.value || null)}
                      className="w-full rounded-lg border border-white/10 bg-black/30 px-3 py-2 text-sm text-white transition focus:border-blue-400/40 focus:outline-none"
                    >
                      <option value="">{t("components.promptTemplate.personaPlaceholder")}</option>
                      {personas.map((p) => (
                        <option key={p.id} value={p.id}>{p.title}</option>
                      ))}
                    </select>
                  </div>

                  <button
                    onClick={handlePreview}
                    disabled={!previewCharacterId || previewing}
                    className={cn(
                      "w-full rounded-xl border px-4 py-2 text-sm font-medium transition",
                      !previewCharacterId || previewing
                        ? "border-white/10 bg-white/5 text-white/30"
                        : "border-blue-400/40 bg-blue-400/15 text-blue-100 hover:bg-blue-400/25 active:scale-[0.99]"
                    )}
                  >
                    {previewing ? t("components.promptTemplate.rendering") : t("common.buttons.generate")}
                  </button>
                </>
              )}

              {/* Preview Output */}
              {previewMode === "rendered" && preview && (
                <div className="rounded-xl border border-white/10 bg-black/30 p-3">
                  <pre className="max-h-60 overflow-auto whitespace-pre-wrap text-xs leading-relaxed text-white/80">
                    {preview}
                  </pre>
                </div>
              )}

              {previewMode === "raw" && content && (
                <div className="rounded-xl border border-white/10 bg-black/30 p-3">
                  <pre className="max-h-60 overflow-auto whitespace-pre-wrap text-xs leading-relaxed text-white/80">
                    {content}
                  </pre>
                </div>
              )}

              {!preview && previewMode === "rendered" && (
                <div className="rounded-xl border border-white/10 bg-white/5 p-6 text-center">
                  <p className="text-sm text-white/50">{t("components.promptTemplate.noPreview")}</p>
                </div>
              )}
            </div>
          </div>

          {/* Footer Actions */}
          <div className="flex gap-3 border-t border-white/10 px-5 py-4">
            <button
              onClick={handleSave}
              disabled={saving || !name.trim() || !content.trim()}
              className={cn(
                "flex-1 rounded-xl border px-4 py-2.5 text-sm font-semibold transition",
                saving || !name.trim() || !content.trim()
                  ? "border-white/10 bg-white/5 text-white/30"
                  : "border-emerald-400/40 bg-emerald-400/20 text-emerald-100 hover:bg-emerald-400/30 active:scale-[0.99]"
              )}
            >
              {saving ? t("components.promptTemplate.saving") : isEditing ? t("components.promptTemplate.update") : t("components.promptTemplate.create")}
            </button>
            <button
              onClick={onClose}
              disabled={saving}
              className="flex-1 rounded-xl border border-white/10 bg-white/5 px-4 py-2.5 text-sm font-medium text-white transition hover:bg-white/10 active:scale-[0.99] disabled:opacity-50"
            >
              {t("common.buttons.cancel")}
            </button>
          </div>
        </div>
      </div>

      {/* Variables Bottom Sheet */}
      <BottomMenu
        isOpen={showVariables}
        onClose={() => setShowVariables(false)}
        title={t("components.promptTemplate.variablesTitle")}
      >
        <p className="mb-4 text-xs text-white/50">{t("components.promptTemplate.variablesCopyHint")}</p>
        <div className="max-h-[50vh] space-y-2 overflow-y-auto">
          {variables.map((item) => (
            <button
              key={item.var}
              onClick={() => copyVariable(item.var)}
              className="w-full rounded-xl border border-purple-400/20 bg-purple-400/5 p-4 text-left transition-colors active:bg-purple-400/10"
            >
              <div className="flex items-start justify-between gap-3">
                <div className="flex-1 space-y-1">
                  <div className="flex items-center gap-2">
                    <code className="text-sm font-semibold text-emerald-300">{item.var}</code>
                    {copiedVar === item.var && (
                      <span className="flex items-center gap-1 text-xs text-emerald-400">
                        <Check className="h-3 w-3" />
                        {t("components.promptTemplate.variablesCopied")}
                      </span>
                    )}
                  </div>
                  <p className="text-sm font-medium text-white/90">{item.label}</p>
                  <p className="text-xs text-white/50">{item.desc}</p>
                </div>
              </div>
            </button>
          ))}
        </div>
      </BottomMenu>
    </>
  );
}
