import { useNavigate } from "react-router-dom";
import { User, Trash2, Edit2, Star, ChevronRight, Download, Loader2 } from "lucide-react";
import { motion, AnimatePresence } from "framer-motion";
import type { Persona } from "../../../core/storage/schemas";
import { BottomMenu } from "../../components";
import { AvatarImage } from "../../components/AvatarImage";
import { usePersonasController } from "../personas/hooks/usePersonasController";
import { useAvatar } from "../../hooks/useAvatar";
import { cn } from "../../design-tokens";
import {
  exportPersona,
  downloadJson,
  generateExportFilename,
} from "../../../core/storage/personaTransfer";
import { useState } from "react";

const PersonaAvatar = ({ persona }: { persona: Persona }) => {
  const avatarDataUrl = useAvatar("persona", persona.id, persona.avatarPath);

  return (
    <div
      className={`relative flex h-10 w-10 shrink-0 items-center justify-center overflow-hidden rounded-lg border ${
        persona.isDefault ? "border-accent/40 bg-accent/20" : "border-fg/15 bg-fg/8"
      }`}
    >
      {avatarDataUrl ? (
        <AvatarImage src={avatarDataUrl} alt={persona.title} crop={persona.avatarCrop} applyCrop />
      ) : (
        <User className={`h-5 w-5 ${persona.isDefault ? "text-accent/80" : "text-fg/70"}`} />
      )}
    </div>
  );
};
const PersonaSkeleton = () => (
  <div className="space-y-3">
    {[1, 2, 3].map((i) => (
      <div key={i} className="rounded-xl border border-fg/10 bg-surface/90 px-4 py-3">
        <div className="flex items-center gap-3">
          <div className="h-10 w-10 animate-pulse rounded-lg bg-fg/10" />
          <div className="flex-1 space-y-2">
            <div className="h-4 w-24 animate-pulse rounded bg-fg/10" />
            <div className="h-3 w-40 animate-pulse rounded bg-fg/10" />
          </div>
        </div>
      </div>
    ))}
  </div>
);

const EmptyState = ({ onCreate }: { onCreate: () => void }) => (
  <div className="flex h-64 flex-col items-center justify-center">
    <User className="mb-3 h-12 w-12 text-fg/20" />
    <h3 className="mb-1 text-lg font-medium text-fg">No personas yet</h3>
    <p className="mb-4 text-center text-sm text-fg/50">
      Create a persona to define how the AI should address you
    </p>
    <button
      onClick={onCreate}
      className="rounded-full border border-accent/40 bg-accent/20 px-6 py-2 text-sm font-medium text-accent/90 transition hover:bg-accent/30"
    >
      Create Persona
    </button>
  </div>
);

export function PersonasPage() {
  const navigate = useNavigate();
  const {
    state: { personas, loading, selectedPersona, showDeleteConfirm, deleting },
    setSelectedPersona,
    setShowDeleteConfirm,
    handleDelete,
    handleSetDefault,
  } = usePersonasController();

  const [exporting, setExporting] = useState(false);

  const handleEditPersona = (persona: Persona) => {
    navigate(`/settings/personas/${persona.id}/edit`);
  };

  const handleExport = async () => {
    if (!selectedPersona) return;

    try {
      setExporting(true);
      const exportJson = await exportPersona(selectedPersona.id);
      const filename = generateExportFilename(selectedPersona.title);
      await downloadJson(exportJson, filename);
      setSelectedPersona(null);
    } catch (err) {
      console.error("Failed to export persona:", err);
    } finally {
      setExporting(false);
    }
  };

  const defaultPersona = personas.find((p) => p.isDefault);

  return (
    <div className="flex h-full flex-col pb-16 text-fg/90">
      <main className="flex-1 overflow-y-auto px-4 pt-4">
        {loading ? (
          <PersonaSkeleton />
        ) : personas.length === 0 ? (
          <EmptyState onCreate={() => navigate("/create/persona")} />
        ) : (
          <div className="space-y-3">
            {/* Default Persona Indicator */}
            {defaultPersona && (
              <div className="rounded-xl border border-accent/30 bg-accent/10 p-3">
                <div className="flex items-center gap-2">
                  <Star className="h-4 w-4 fill-accent text-accent" />
                  <div className="flex-1">
                    <div className="text-sm font-medium text-accent/80">Default Persona</div>
                    <div className="text-xs text-accent/70">{defaultPersona.title}</div>
                  </div>
                </div>
              </div>
            )}

            {/* Personas List */}
            <AnimatePresence>
              {personas.map((persona) => (
                <motion.button
                  key={persona.id}
                  onClick={() => setSelectedPersona(persona)}
                  className={`group relative flex w-full items-center gap-3 overflow-hidden rounded-xl border px-4 py-3 text-left transition-all duration-200 active:scale-[0.995] ${
                    persona.isDefault
                      ? "border-accent/40 bg-accent/10 hover:border-accent/60 hover:bg-accent/15"
                      : "border-fg/10 bg-surface/90 hover:border-fg/25 hover:bg-surface/95"
                  }`}
                >
                  <div
                    className={cn(
                      "absolute inset-y-0 right-0 w-1/4 transition",
                      "bg-linear-to-l from-secondary/10 via-transparent to-transparent opacity-0 group-hover:opacity-100",
                    )}
                  />

                  <PersonaAvatar persona={persona} />

                  <div className="relative min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <h3 className="truncate text-sm font-semibold text-fg">{persona.title}</h3>
                      {persona.isDefault && (
                        <Star className="h-3 w-3 shrink-0 fill-accent text-accent" />
                      )}
                    </div>
                    <p className="line-clamp-1 text-xs text-fg/60">{persona.description}</p>
                  </div>

                  <span
                    className={`relative flex h-9 w-9 shrink-0 items-center justify-center rounded-full border transition ${
                      persona.isDefault
                        ? "border-accent/30 bg-accent/10 text-accent/80 group-hover:border-accent/50"
                        : "border-fg/10 bg-fg/5 text-fg/70 group-hover:border-fg/25 group-hover:text-fg"
                    }`}
                  >
                    <ChevronRight size={16} />
                  </span>
                </motion.button>
              ))}
            </AnimatePresence>
          </div>
        )}
      </main>

      {/* Persona Actions Menu */}
      <BottomMenu
        isOpen={Boolean(selectedPersona)}
        onClose={() => setSelectedPersona(null)}
        title={selectedPersona?.title || ""}
      >
        {selectedPersona && (
          <div className="space-y-2">
            <button
              onClick={() => handleEditPersona(selectedPersona)}
              className="flex w-full items-center gap-3 rounded-xl border border-fg/10 bg-fg/5 px-4 py-3 text-left transition hover:border-fg/20 hover:bg-fg/10"
            >
              <div className="flex h-8 w-8 items-center justify-center rounded-lg border border-fg/10 bg-fg/10">
                <Edit2 className="h-4 w-4 text-fg/70" />
              </div>
              <span className="text-sm font-medium text-fg">Edit Persona</span>
            </button>

            <button
              onClick={() => void handleSetDefault(selectedPersona)}
              className="flex w-full items-center gap-3 rounded-xl border border-fg/10 bg-fg/5 px-4 py-3 text-left transition hover:border-fg/20 hover:bg-fg/10"
            >
              <div className="flex h-8 w-8 items-center justify-center rounded-lg border border-fg/10 bg-fg/10">
                <Star
                  className={`h-4 w-4 ${
                    selectedPersona.isDefault
                      ? "fill-accent text-accent"
                      : "text-fg/70"
                  }`}
                />
              </div>
              <div className="flex-1">
                <span className="text-sm font-medium text-fg">
                  {selectedPersona.isDefault ? "Unset as Default" : "Set as Default"}
                </span>
                <p className="text-xs text-fg/50">
                  {selectedPersona.isDefault
                    ? "Remove default status"
                    : "Use this for all new chats"}
                </p>
              </div>
            </button>

            <button
              onClick={handleExport}
              disabled={exporting}
              className="flex w-full items-center gap-3 rounded-xl border border-info/30 bg-info/10 px-4 py-3 text-left transition hover:border-info/50 hover:bg-info/20 disabled:opacity-50"
            >
              <div className="flex h-8 w-8 items-center justify-center rounded-lg border border-info/30 bg-info/20">
                {exporting ? (
                  <Loader2 className="h-4 w-4 animate-spin text-info" />
                ) : (
                  <Download className="h-4 w-4 text-info" />
                )}
              </div>
              <span className="text-sm font-medium text-info/80">
                {exporting ? "Exporting..." : "Export Persona"}
              </span>
            </button>

            <button
              onClick={() => {
                setShowDeleteConfirm(true);
              }}
              className="flex w-full items-center gap-3 rounded-xl border border-danger/30 bg-danger/10 px-4 py-3 text-left transition hover:border-danger/50 hover:bg-danger/20"
            >
              <div className="flex h-8 w-8 items-center justify-center rounded-lg border border-danger/30 bg-danger/20">
                <Trash2 className="h-4 w-4 text-danger" />
              </div>
              <span className="text-sm font-medium text-danger/80">Delete Persona</span>
            </button>
          </div>
        )}
      </BottomMenu>

      {/* Delete Confirmation */}
      <BottomMenu
        isOpen={showDeleteConfirm}
        onClose={() => setShowDeleteConfirm(false)}
        title="Delete Persona?"
      >
        <div className="space-y-4">
          <p className="text-sm text-fg/70">
            Are you sure you want to delete "{selectedPersona?.title}"? This action cannot be
            undone.
          </p>
          <div className="flex gap-3">
            <button
              onClick={() => setShowDeleteConfirm(false)}
              disabled={deleting}
              className="flex-1 rounded-xl border border-fg/10 bg-fg/5 py-3 text-sm font-medium text-fg transition hover:border-fg/20 hover:bg-fg/10 disabled:opacity-50"
            >
              Cancel
            </button>
            <button
              onClick={() => void handleDelete()}
              disabled={deleting}
              className="flex-1 rounded-xl border border-danger/30 bg-danger/20 py-3 text-sm font-medium text-danger/80 transition hover:bg-danger/30 disabled:opacity-50"
            >
              {deleting ? "Deleting..." : "Delete"}
            </button>
          </div>
        </div>
      </BottomMenu>
    </div>
  );
}
