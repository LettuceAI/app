import { useCallback, useEffect, useRef, useState } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { motion, AnimatePresence } from "framer-motion";
import { MessageSquare, Trash2, MoreVertical, Loader2, Download } from "lucide-react";
import { listCharacters, saveCharacter } from "../../../core/storage/repo";
import type { Character, ChatTemplate } from "../../../core/storage/schemas";
import { BottomMenu, ChatTemplateExportMenu, MenuButton } from "../../components";
import { useI18n } from "../../../core/i18n/context";
import { toast } from "../../components/toast";
import {
  exportChatTemplateAsUsc,
  generateChatTemplateExportFilename,
  importChatTemplate,
  serializeChatTemplateExport,
} from "../../../core/storage/chatTemplateTransfer";
import { downloadJson, readFileAsText } from "../../../core/storage/personaTransfer";
import type { ChatTemplateExportFormat } from "../../components/ChatTemplateExportMenu";

export default function ChatTemplateListPage() {
  const { characterId } = useParams<{ characterId: string }>();
  const navigate = useNavigate();
  const { t } = useI18n();
  const [character, setCharacter] = useState<Character | null>(null);
  const [loading, setLoading] = useState(true);
  const [menuTemplateId, setMenuTemplateId] = useState<string | null>(null);
  const [exportTarget, setExportTarget] = useState<ChatTemplate | null>(null);
  const [showExportMenu, setShowExportMenu] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [importing, setImporting] = useState(false);
  const importInputRef = useRef<HTMLInputElement | null>(null);

  const loadCharacter = useCallback(async () => {
    if (!characterId) return;
    try {
      const chars = await listCharacters();
      const c = chars.find((ch) => ch.id === characterId);
      if (c) setCharacter(c);
    } finally {
      setLoading(false);
    }
  }, [characterId]);

  useEffect(() => {
    loadCharacter();
  }, [loadCharacter]);

  useEffect(() => {
    const handleAdd = () => navigate(`/settings/characters/${characterId}/templates/new`);
    window.addEventListener("templates:add", handleAdd);
    return () => window.removeEventListener("templates:add", handleAdd);
  }, [characterId, navigate]);

  useEffect(() => {
    const handleImport = () => importInputRef.current?.click();
    window.addEventListener("templates:import", handleImport);
    return () => window.removeEventListener("templates:import", handleImport);
  }, [importInputRef]);

  const templates = character?.chatTemplates ?? [];

  const handleDelete = useCallback(
    async (templateId: string) => {
      if (!character) return;
      const updated = {
        ...character,
        chatTemplates: templates.filter((t) => t.id !== templateId),
      };
      await saveCharacter(updated);
      setCharacter(updated);
      setMenuTemplateId(null);
    },
    [character, templates],
  );

  const handleExport = useCallback(
    async (format: ChatTemplateExportFormat) => {
      if (!exportTarget || exporting) return;
      try {
        setExporting(true);
        const exportJson =
          format === "usc"
            ? await exportChatTemplateAsUsc(exportTarget)
            : serializeChatTemplateExport(exportTarget);
        await downloadJson(
          exportJson,
          generateChatTemplateExportFilename(exportTarget.name, format),
        );
        setShowExportMenu(false);
        setExportTarget(null);
      } catch (error) {
        console.error("Failed to export chat template:", error);
        toast.error(t("characters.templates.exportFailed"), String(error));
      } finally {
        setExporting(false);
      }
    },
    [exportTarget, exporting, t],
  );

  const handleImportFile = useCallback(
    async (file: File) => {
      if (!character || importing) return;
      try {
        setImporting(true);
        const raw = await readFileAsText(file);
        const imported = importChatTemplate(raw);
        const updated = {
          ...character,
          chatTemplates: [...templates, imported],
        };
        await saveCharacter(updated);
        setCharacter(updated);
        toast.success(
          t("characters.templates.importedToast.title"),
          t("characters.templates.importedToast.message", { name: imported.name }),
        );
      } catch (error) {
        console.error("Failed to import chat template:", error);
        toast.error(t("characters.templates.importFailed"), String(error));
      } finally {
        setImporting(false);
        if (importInputRef.current) {
          importInputRef.current.value = "";
        }
      }
    },
    [character, importing, templates, importInputRef, t],
  );

  const menuTemplate = templates.find((t) => t.id === menuTemplateId);

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center">
        <Loader2 className="h-6 w-6 animate-spin text-fg/40" />
      </div>
    );
  }

  if (!character) {
    return (
      <div className="flex h-full items-center justify-center text-fg/50">
        {t("characters.templates.characterNotFound")}
      </div>
    );
  }

  return (
    <motion.div
      initial={{ opacity: 0, y: 16 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.2, ease: "easeOut" }}
      className="space-y-4"
    >
      {templates.length > 0 && (
        <div className="flex items-center justify-between">
          <p className="text-xs text-fg/50">
            {t("characters.templates.templateCount", { count: templates.length })}
          </p>
        </div>
      )}

      <input
        ref={(node) => {
          importInputRef.current = node;
        }}
        type="file"
        className="hidden"
        onChange={(event) => {
          const file = event.target.files?.[0];
          if (file) {
            void handleImportFile(file);
          }
        }}
      />

      {templates.length === 0 ? (
        <div className="flex flex-col items-center justify-center py-16 text-center">
          <div className="mb-3 rounded-2xl border border-fg/10 bg-fg/5 p-4">
            <MessageSquare className="h-8 w-8 text-fg/30" />
          </div>
          <p className="text-sm font-medium text-fg/60">
            {t("characters.templates.noTemplatesYet")}
          </p>
          <p className="mt-1 max-w-xs text-xs text-fg/40">
            {t("characters.templates.explanation", { name: character.name })}
          </p>
        </div>
      ) : (
        <div className="space-y-2">
          <AnimatePresence initial={false} mode="sync">
            {templates.map((template) => (
              <motion.div
                key={template.id}
                layout="position"
                initial={{ opacity: 0, y: 8 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: -8 }}
                transition={{
                  opacity: { duration: 0.14, ease: "easeOut" },
                  y: { duration: 0.18, ease: "easeOut" },
                  layout: { type: "spring", stiffness: 420, damping: 34, mass: 0.72 },
                }}
                className="rounded-xl border border-fg/10 bg-surface-el/30 transition active:bg-surface-el/50"
              >
                <button
                  type="button"
                  className="flex w-full items-start gap-3 p-3.5 text-left"
                  onClick={() =>
                    navigate(`/settings/characters/${characterId}/templates/${template.id}`)
                  }
                >
                  <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg border border-fg/10 bg-fg/5">
                    <MessageSquare className="h-4 w-4 text-fg/50" />
                  </div>
                  <div className="min-w-0 flex-1">
                    <span className="truncate text-sm font-medium text-fg">{template.name}</span>
                    <p className="mt-0.5 text-xs text-fg/50">
                      {t("characters.templates.messageCount", { count: template.messages.length })}
                    </p>
                    {template.messages.length > 0 && (
                      <p className="mt-1 line-clamp-2 text-xs text-fg/40">
                        {template.messages[0].content.slice(0, 120)}
                        {template.messages[0].content.length > 120 ? "..." : ""}
                      </p>
                    )}
                  </div>
                  <button
                    type="button"
                    onClick={(e) => {
                      e.stopPropagation();
                      setMenuTemplateId(template.id);
                    }}
                    className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg text-fg/40 transition active:bg-fg/10"
                  >
                    <MoreVertical className="h-4 w-4" />
                  </button>
                </button>
              </motion.div>
            ))}
          </AnimatePresence>
        </div>
      )}

      {/* Context menu */}
      <BottomMenu
        isOpen={!!menuTemplateId}
        onClose={() => setMenuTemplateId(null)}
        title={menuTemplate?.name ?? t("characters.templates.contextMenuFallbackTitle")}
      >
        <div className="space-y-3">
          <MenuButton
            icon={<Download className="h-4 w-4" />}
            title={t("common.buttons.export")}
            color="from-emerald-500 to-emerald-600"
            onClick={() => {
              if (!menuTemplate) return;
              setExportTarget(menuTemplate);
              setMenuTemplateId(null);
              setShowExportMenu(true);
            }}
          />
          <MenuButton
            icon={<Trash2 className="h-4 w-4" />}
            title={t("characters.templates.deleteTemplate")}
            color="from-rose-500 to-red-600"
            onClick={() => menuTemplateId && handleDelete(menuTemplateId)}
          />
        </div>
      </BottomMenu>

      <ChatTemplateExportMenu
        isOpen={showExportMenu}
        onClose={() => {
          if (exporting) return;
          setShowExportMenu(false);
          setExportTarget(null);
        }}
        onSelect={(format) => {
          void handleExport(format);
        }}
        exporting={exporting}
      />
    </motion.div>
  );
}
