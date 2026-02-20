import { useEffect, useState, memo, useRef } from "react";
import { motion } from "framer-motion";
import {
  listCharacters,
  listPersonas,
  deleteCharacter,
  deletePersona,
  createSession,
  listLorebooks,
  deleteLorebook,
  saveLorebook,
} from "../../../core/storage/repo";
import type { Character, Persona, Lorebook } from "../../../core/storage/schemas";
import { typography, interactive, cn } from "../../design-tokens";
import { useAvatar } from "../../hooks/useAvatar";
import { useAvatarGradient } from "../../hooks/useAvatarGradient";
import { useRocketEasterEgg } from "../../hooks/useRocketEasterEgg";
import { useNavigate } from "react-router-dom";
import { BottomMenu, CharacterExportMenu } from "../../components";
import {
  MessageCircle,
  Edit2,
  Trash2,
  Download,
  Upload,
  Check,
  BookOpen,
  Users,
  Pencil,
  Paintbrush,
  Rocket,
} from "lucide-react";
import {
  exportCharacterWithFormat,
  downloadJson,
  generateExportFilenameWithFormat,
  type CharacterFileFormat,
} from "../../../core/storage/characterTransfer";
import { exportPersona, generateExportFilename } from "../../../core/storage/personaTransfer";
import { importLorebook, readFileAsText } from "../../../core/storage/lorebookTransfer";
import { listen } from "@tauri-apps/api/event";

type FilterOption = "All" | "Characters" | "Personas" | "Lorebooks";
type LibraryItem = (Character | Persona | Lorebook) & {
  itemType: "character" | "persona" | "lorebook";
};

function getItemName(item: LibraryItem): string {
  if (item.itemType === "character") return (item as Character).name;
  if (item.itemType === "persona") return (item as Persona).title;
  return (item as Lorebook).name;
}

function getItemDisableGradient(item: LibraryItem): boolean | undefined {
  return item.itemType === "character" ? (item as Character).disableAvatarGradient : undefined;
}

export function LibraryPage() {
  const [characters, setCharacters] = useState<Character[]>([]);
  const [personas, setPersonas] = useState<Persona[]>([]);
  const [lorebooks, setLorebooks] = useState<Lorebook[]>([]);
  const [filter, setFilter] = useState<FilterOption>("All");
  const [showFilterMenu, setShowFilterMenu] = useState(false);
  const [selectedItem, setSelectedItem] = useState<LibraryItem | null>(null);
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [exportMenuOpen, setExportMenuOpen] = useState(false);
  const [exportTarget, setExportTarget] = useState<LibraryItem | null>(null);
  const [importingLorebook, setImportingLorebook] = useState(false);
  const lorebookImportRef = useRef<HTMLInputElement | null>(null);
  const rocket = useRocketEasterEgg();

  // Rename state
  const [renameItem, setRenameItem] = useState<LibraryItem | null>(null);
  const [renameName, setRenameName] = useState("");
  const [renaming, setRenaming] = useState(false);

  const navigate = useNavigate();

  const loadData = async () => {
    try {
      const [chars, pers, lbs] = await Promise.all([
        listCharacters(),
        listPersonas(),
        listLorebooks(),
      ]);
      setCharacters(chars);
      setPersonas(pers);
      setLorebooks(lbs);
    } catch (error) {
      console.error("Failed to load library data:", error);
    }
  };

  useEffect(() => {
    loadData();
    const unlisten = listen("database-reloaded", () => {
      console.log("Database reloaded, refreshing library data...");
      loadData();
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  useEffect(() => {
    const handleOpenFilter = () => setShowFilterMenu(true);
    window.addEventListener("library:openFilter", handleOpenFilter);
    return () => window.removeEventListener("library:openFilter", handleOpenFilter);
  }, []);

  const handleRenameConfirm = async () => {
    if (!renameItem || !renameName.trim()) return;

    try {
      setRenaming(true);
      // Only lorebooks can be renamed this way for now
      if (renameItem.itemType === "lorebook") {
        await saveLorebook({ id: renameItem.id, name: renameName.trim() });
      }
      setRenameItem(null);
      setRenameName("");
      await loadData(); // Reload
    } catch (error) {
      console.error("Failed to rename:", error);
    } finally {
      setRenaming(false);
    }
  };

  const handleSelect = (item: LibraryItem) => {
    setSelectedItem(item);
  };

  const handleStartChat = async () => {
    if (selectedItem && selectedItem.itemType === "character") {
      const sceneId =
        (selectedItem as Character).defaultSceneId || (selectedItem as Character).scenes?.[0]?.id;
      const session = await createSession(
        selectedItem.id,
        `Chat with ${getItemName(selectedItem)}`,
        sceneId,
      );

      navigate(`/chat/${selectedItem.id}?sessionId=${session.id}`);
      setSelectedItem(null);
    }
  };

  const handleEdit = () => {
    if (selectedItem) {
      if (selectedItem.itemType === "character") {
        navigate(`/settings/characters/${selectedItem.id}/edit`);
      } else if (selectedItem.itemType === "persona") {
        navigate(`/settings/personas/${selectedItem.id}/edit`);
      } else {
        navigate(`/library/lorebooks/${selectedItem.id}`);
      }
      setSelectedItem(null);
    }
  };

  const handleDelete = async () => {
    if (!selectedItem) return;
    try {
      setDeleting(true);
      if (selectedItem.itemType === "character") {
        await deleteCharacter(selectedItem.id);
        const list = await listCharacters();
        setCharacters(list);
      } else if (selectedItem.itemType === "persona") {
        await deletePersona(selectedItem.id);
        const list = await listPersonas();
        setPersonas(list);
      } else {
        await deleteLorebook(selectedItem.id);
        const list = await listLorebooks();
        setLorebooks(list);
      }
      setShowDeleteConfirm(false);
      setSelectedItem(null);
    } catch (err) {
      console.error("Failed to delete:", err);
    } finally {
      setDeleting(false);
    }
  };

  const handleExport = () => {
    if (!selectedItem || selectedItem.itemType !== "character") return;
    setExportTarget(selectedItem);
    setSelectedItem(null);
    setExportMenuOpen(true);
  };

  const handlePersonaExport = async () => {
    if (!selectedItem || selectedItem.itemType !== "persona") return;
    try {
      setExporting(true);
      const exportJson = await exportPersona(selectedItem.id);
      const filename = generateExportFilename(getItemName(selectedItem));
      await downloadJson(exportJson, filename);
      setSelectedItem(null);
    } catch (err) {
      console.error("Failed to export persona:", err);
    } finally {
      setExporting(false);
    }
  };

  const handleExportFormat = async (format: CharacterFileFormat) => {
    if (!exportTarget || exportTarget.itemType !== "character") return;
    try {
      setExporting(true);
      const exportJson = await exportCharacterWithFormat(exportTarget.id, format);
      const filename = generateExportFilenameWithFormat(getItemName(exportTarget), format);
      await downloadJson(exportJson, filename);
    } catch (err) {
      console.error("Failed to export character:", err);
    } finally {
      setExporting(false);
      setExportMenuOpen(false);
      setExportTarget(null);
    }
  };

  const handleImportLorebook = async (file: File) => {
    if (importingLorebook) return;
    try {
      setImportingLorebook(true);
      const raw = await readFileAsText(file);
      const imported = await importLorebook(raw);
      await loadData();
      navigate(`/library/lorebooks/${imported.id}`);
    } catch (err) {
      console.error("Failed to import lorebook:", err);
      alert("Failed to import lorebook. " + String(err));
    } finally {
      setImportingLorebook(false);
      if (lorebookImportRef.current) {
        lorebookImportRef.current.value = "";
      }
    }
  };

  const allItems: LibraryItem[] = [
    ...characters.map((c) => ({ ...c, itemType: "character" as const })),
    ...personas.map((p) => ({ ...p, itemType: "persona" as const })),
    ...lorebooks.map((l) => ({ ...l, itemType: "lorebook" as const })),
  ];

  const filteredItems = allItems.filter((item) => {
    if (filter === "All") return true;
    if (filter === "Characters") return item.itemType === "character";
    if (filter === "Personas") return item.itemType === "persona";
    if (filter === "Lorebooks") return item.itemType === "lorebook";
    return false;
  });

  return (
    <div className="flex h-full flex-col pb-6 text-fg/80">
      <main className="flex-1 overflow-y-auto px-4 pt-4">
        {filteredItems.length === 0 ? (
          <motion.div
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.3 }}
            className="relative flex flex-1 flex-col items-center justify-center px-6 py-20 overflow-hidden"
            {...rocket.bind}
          >
            {rocket.isLaunched && (
              <div className="pointer-events-none absolute bottom-8 left-1/2 -translate-x-1/2 rocket-launch">
                <div className="flex h-10 w-10 items-center justify-center rounded-full border border-fg/10 bg-fg/10">
                  <Rocket className="h-5 w-5 text-fg/80" />
                </div>
              </div>
            )}
            <div className="relative mb-6">
              <div className="flex h-20 w-20 items-center justify-center rounded-2xl border border-fg/10 bg-fg/5">
                <BookOpen className="h-10 w-10 text-fg/30" />
              </div>
            </div>
            <h3
              className={cn(
                typography.heading.size,
                typography.heading.weight,
                typography.heading.lineHeight,
                "mb-2 text-center text-fg/80",
              )}
            >
              {filter === "All"
                ? "Your library is empty"
                : filter === "Characters"
                  ? "No characters yet"
                  : filter === "Personas"
                    ? "No personas yet"
                    : "No lorebooks yet"}
            </h3>
            <p className="mb-6 max-w-70 text-center text-sm text-fg/50">
              {filter === "All"
                ? "Create characters, personas, and lorebooks to see them here"
                : filter === "Characters"
                  ? "Create your first character to start chatting"
                  : filter === "Personas"
                    ? "Create a persona to customize your chat identity"
                    : "Lorebooks are created from within a character's settings"}
            </p>
            {filter === "Lorebooks" && (
              <button
                onClick={() => lorebookImportRef.current?.click()}
                disabled={importingLorebook}
                className="mb-3 flex items-center gap-2 rounded-xl border border-info/40 bg-info/20 px-5 py-2.5 text-sm font-medium text-info transition active:scale-95 active:bg-info/30 disabled:opacity-60 disabled:cursor-not-allowed"
              >
                <Upload className="h-4 w-4" />
                {importingLorebook ? "Importing..." : "Import Lorebook"}
              </button>
            )}
            {filter !== "Lorebooks" && (
              <button
                onClick={() =>
                  navigate(filter === "Personas" ? "/personas/create" : "/characters/create")
                }
                className="flex items-center gap-2 rounded-xl border border-accent/40 bg-accent/20 px-5 py-2.5 text-sm font-medium text-accent/70 transition active:scale-95 active:bg-accent/30"
              >
                <Users className="h-4 w-4" />
                {filter === "Personas" ? "Create Persona" : "Create Character"}
              </button>
            )}
            <input
              ref={lorebookImportRef}
              type="file"
              accept=".json,application/json"
              className="hidden"
              onChange={(e) => {
                const file = e.target.files?.[0];
                if (file) {
                  handleImportLorebook(file);
                }
              }}
            />
          </motion.div>
        ) : (
          <div className="grid grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-3 pb-24">
            {filteredItems.map((item) => (
              <LibraryCard
                key={`${item.itemType}-${item.id}`}
                item={item}
                onSelect={handleSelect}
              />
            ))}
          </div>
        )}
      </main>

      {/* Filter Menu */}
      <BottomMenu
        isOpen={showFilterMenu}
        onClose={() => setShowFilterMenu(false)}
        title="Filter Library"
      >
        <div className="space-y-2">
          {(["All", "Characters", "Personas", "Lorebooks"] as FilterOption[]).map((option) => (
            <button
              key={option}
              onClick={() => {
                setFilter(option);
                setShowFilterMenu(false);
              }}
              className={cn(
                "flex w-full items-center justify-between rounded-xl px-4 py-3 text-left transition",
                filter === option
                  ? "bg-fg/10 text-fg"
                  : "text-fg/60 hover:bg-fg/5 hover:text-fg",
              )}
            >
              <span className="text-sm font-medium">{option}</span>
              {filter === option && <Check className="h-4 w-4 text-accent" />}
            </button>
          ))}
        </div>
      </BottomMenu>

      {/* Item Actions Menu */}
      <BottomMenu
        isOpen={Boolean(selectedItem)}
        onClose={() => setSelectedItem(null)}
        title={selectedItem ? getItemName(selectedItem) : ""}
      >
        {selectedItem && (
          <div className="space-y-2">
            {selectedItem.itemType === "character" && (
              <button
                onClick={handleStartChat}
                className="flex w-full items-center gap-3 rounded-xl border border-accent/30 bg-accent/10 px-4 py-3 text-left transition hover:border-accent/50 hover:bg-accent/20"
              >
                <div className="flex h-8 w-8 items-center justify-center rounded-full border border-accent/30 bg-accent/20">
                  <MessageCircle className="h-4 w-4 text-accent" />
                </div>
                <span className="text-sm font-medium text-accent">Start Chat</span>
              </button>
            )}

            <button
              onClick={handleEdit}
              className="flex w-full items-center gap-3 rounded-xl border border-fg/10 bg-fg/5 px-4 py-3 text-left transition hover:border-fg/20 hover:bg-fg/10"
            >
              <div className="flex h-8 w-8 items-center justify-center rounded-full border border-fg/10 bg-fg/10">
                <Edit2 className="h-4 w-4 text-fg/70" />
              </div>
              <span className="text-sm font-medium text-fg">
                {selectedItem.itemType === "character"
                  ? "Edit Character"
                  : selectedItem.itemType === "persona"
                    ? "Edit Persona"
                    : "Edit Lorebook"}
              </span>
            </button>

            {selectedItem.itemType === "lorebook" && (
              <button
                onClick={() => {
                  setRenameItem(selectedItem);
                  setRenameName(getItemName(selectedItem));
                  setSelectedItem(null); // Close main menu
                }}
                className="flex w-full items-center gap-3 rounded-xl border border-fg/10 bg-fg/5 px-4 py-3 text-left transition hover:border-fg/20 hover:bg-fg/10"
              >
                <div className="flex h-8 w-8 items-center justify-center rounded-full border border-fg/10 bg-fg/10">
                  <Pencil className="h-4 w-4 text-fg/70" />
                </div>
                <span className="text-sm font-medium text-fg">Rename Lorebook</span>
              </button>
            )}

            {selectedItem.itemType === "character" && (
              <button
                onClick={handleExport}
                disabled={exporting}
                className="flex w-full items-center gap-3 rounded-xl border border-info/30 bg-info/10 px-4 py-3 text-left transition hover:border-info/50 hover:bg-info/20 disabled:opacity-50"
              >
                <div className="flex h-8 w-8 items-center justify-center rounded-full border border-info/30 bg-info/20">
                  <Download className="h-4 w-4 text-info" />
                </div>
                <span className="text-sm font-medium text-info">
                  {exporting ? "Exporting..." : "Export Character"}
                </span>
              </button>
            )}

            {selectedItem.itemType === "character" && (
              <button
                onClick={() => {
                  const charId = selectedItem.id;
                  setSelectedItem(null);
                  navigate(`/settings/accessibility/chat?characterId=${charId}`);
                }}
                className="flex w-full items-center gap-3 rounded-xl border border-secondary/30 bg-secondary/10 px-4 py-3 text-left transition hover:border-secondary/50 hover:bg-secondary/20"
              >
                <div className="flex h-8 w-8 items-center justify-center rounded-full border border-secondary/30 bg-secondary/20">
                  <Paintbrush className="h-4 w-4 text-secondary" />
                </div>
                <span className="text-sm font-medium text-secondary">Chat Appearance</span>
              </button>
            )}

            {selectedItem.itemType === "persona" && (
              <button
                onClick={handlePersonaExport}
                disabled={exporting}
                className="flex w-full items-center gap-3 rounded-xl border border-info/30 bg-info/10 px-4 py-3 text-left transition hover:border-info/50 hover:bg-info/20 disabled:opacity-50"
              >
                <div className="flex h-8 w-8 items-center justify-center rounded-full border border-info/30 bg-info/20">
                  <Download className="h-4 w-4 text-info" />
                </div>
                <span className="text-sm font-medium text-info">
                  {exporting ? "Exporting..." : "Export Persona"}
                </span>
              </button>
            )}

            <button
              onClick={() => setShowDeleteConfirm(true)}
              className="flex w-full items-center gap-3 rounded-xl border border-danger/30 bg-danger/10 px-4 py-3 text-left transition hover:border-danger/50 hover:bg-danger/20"
            >
              <div className="flex h-8 w-8 items-center justify-center rounded-full border border-danger/30 bg-danger/20">
                <Trash2 className="h-4 w-4 text-danger" />
              </div>
              <span className="text-sm font-medium text-danger">
                {selectedItem.itemType === "character"
                  ? "Delete Character"
                  : selectedItem.itemType === "persona"
                    ? "Delete Persona"
                    : "Delete Lorebook"}
              </span>
            </button>
          </div>
        )}
      </BottomMenu>

      <CharacterExportMenu
        isOpen={exportMenuOpen}
        onClose={() => {
          setExportMenuOpen(false);
          setExportTarget(null);
        }}
        onSelect={handleExportFormat}
        exporting={exporting}
      />

      {/* Delete Confirmation */}
      <BottomMenu
        isOpen={showDeleteConfirm}
        onClose={() => setShowDeleteConfirm(false)}
        title={`Delete ${selectedItem?.itemType === "character" ? "Character" : selectedItem?.itemType === "persona" ? "Persona" : "Lorebook"}?`}
      >
        <div className="space-y-4">
          <p className="text-sm text-fg/70">
            Are you sure you want to delete \"{selectedItem ? getItemName(selectedItem) : ""}\"?
            {selectedItem?.itemType === "character" &&
              " This will also delete all chat sessions with this character."}
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
              onClick={handleDelete}
              disabled={deleting}
              className="flex-1 rounded-xl border border-danger/30 bg-danger/20 py-3 text-sm font-medium text-danger transition hover:bg-danger/30 disabled:opacity-50"
            >
              {deleting ? "Deleting..." : "Delete"}
            </button>
          </div>
        </div>
      </BottomMenu>
      {/* Rename Menu */}
      <BottomMenu
        isOpen={Boolean(renameItem)}
        onClose={() => setRenameItem(null)}
        title="Rename Lorebook"
      >
        <div className="space-y-4">
          <input
            value={renameName}
            onChange={(e) => setRenameName(e.target.value)}
            placeholder="Enter new name..."
            className="w-full rounded-xl border border-fg/10 bg-surface-el/20 px-4 py-3 text-base text-fg placeholder-fg/40 transition focus:border-fg/25 focus:outline-none"
            autoFocus
          />
          <div className="flex gap-3">
            <button
              onClick={() => setRenameItem(null)}
              className="flex-1 rounded-xl border border-fg/10 bg-fg/5 py-3 text-sm font-medium text-fg transition hover:border-fg/20 hover:bg-fg/10"
            >
              Cancel
            </button>
            <button
              onClick={handleRenameConfirm}
              disabled={renaming || !renameName.trim()}
              className="flex-1 rounded-xl border border-accent/30 bg-accent/20 py-3 text-sm font-medium text-accent/70 transition hover:border-accent/50 hover:bg-accent/30 disabled:opacity-50"
            >
              {renaming ? "Saving..." : "Save"}
            </button>
          </div>
        </div>
      </BottomMenu>
    </div>
  );
}

function isImageLike(s?: string) {
  if (!s) return false;
  const lower = s.toLowerCase();
  return (
    lower.startsWith("http://") || lower.startsWith("https://") || lower.startsWith("data:image")
  );
}

function getItemAvatarPath(item: LibraryItem): string | undefined {
  if (item.itemType === "lorebook") return undefined;
  return (item as Character | Persona).avatarPath;
}

function getItemDescription(item: LibraryItem): string {
  if (item.itemType === "lorebook") return "Lorebook";
  if (item.itemType === "character") {
    const character = item as Character;
    return (character.description || character.definition || "").trim() || "No description yet";
  }
  const persona = item as Persona;
  return persona.description.trim() || "No description yet";
}

const ItemAvatar = memo(({ item, className }: { item: LibraryItem; className?: string }) => {
  if (item.itemType === "lorebook") {
    return (
      <div
        className={cn(
          "flex h-full w-full items-center justify-center bg-linear-to-br from-warning/20 to-warning/80/30",
          className,
        )}
      >
        <BookOpen className="h-12 w-12 text-warning/80" />
      </div>
    );
  }

  const avatarPath = getItemAvatarPath(item);
  const avatarUrl = useAvatar(item.itemType as "character" | "persona", item.id, avatarPath);

  if (avatarUrl && isImageLike(avatarUrl)) {
    return (
      <img
        src={avatarUrl}
        alt={`${getItemName(item)} avatar`}
        className={cn("h-full w-full object-cover", className)}
      />
    );
  }

  const initials = getItemName(item).slice(0, 2).toUpperCase();
  return (
    <span
      className={cn("flex h-full w-full items-center justify-center text-4xl font-bold", className)}
    >
      {initials}
    </span>
  );
});

ItemAvatar.displayName = "ItemAvatar";

const LibraryCard = memo(
  ({ item, onSelect }: { item: LibraryItem; onSelect: (item: LibraryItem) => void }) => {
    const descriptionPreview = getItemDescription(item);
    const avatarPath = getItemAvatarPath(item);

    // Only use gradient for non-lorebook items
    const { gradientCss, hasGradient } = useAvatarGradient(
      item.itemType === "lorebook" ? "character" : (item.itemType as "character" | "persona"),
      item.id,
      avatarPath,
      getItemDisableGradient(item),
    );

    const badge =
      item.itemType === "character"
        ? { label: "Character", dotClass: "bg-info" }
        : item.itemType === "persona"
          ? { label: "Persona", dotClass: "bg-secondary" }
          : { label: "Lorebook", dotClass: "bg-warning" };

    return (
      <motion.button
        layoutId={`library-${item.itemType}-${item.id}`}
        onClick={() => onSelect(item)}
        className={cn(
          "group relative flex aspect-3/4 w-full flex-col justify-end overflow-hidden rounded-2xl text-left",
          "border border-fg/10",
          interactive.active.scale,
        )}
        style={hasGradient && item.itemType !== "lorebook" ? { background: gradientCss } : {}}
      >
        {/* Background Image / Avatar */}
        <div className="absolute inset-0 z-0">
          <ItemAvatar
            item={item}
            className="transition-transform duration-500 group-hover:scale-110"
          />
        </div>

        {/* Gradient Overlay */}
        <div className="absolute inset-0 z-10 bg-linear-to-t from-black/90 via-black/40 to-transparent" />

        {/* Type Badge */}
        <div className="absolute left-2 top-2 z-20">
          <span className="flex items-center gap-1.5 rounded-full border border-fg/15 bg-surface-el/60 px-2.5 py-1 text-[10px] font-semibold uppercase tracking-[0.08em] text-fg/80 backdrop-blur-md shadow-sm shadow-black/30">
            <span className={cn("h-2 w-2 rounded-full", badge.dotClass)} />
            {badge.label}
          </span>
        </div>

        {/* Glass Content Area */}
        <div className="relative z-20 flex w-full flex-col gap-1 p-3">
          <h3 className={cn(typography.body.size, "font-bold text-fg truncate leading-tight")}>
            {getItemName(item)}
          </h3>
          <p
            className={cn(
              typography.bodySmall.size,
              "text-fg/70 line-clamp-2 text-xs leading-relaxed",
            )}
          >
            {descriptionPreview}
          </p>
        </div>

        {/* Hover Highlight */}
        <div className="absolute inset-0 z-30 bg-fg/0 transition-colors group-hover:bg-fg/5" />
      </motion.button>
    );
  },
);

LibraryCard.displayName = "LibraryCard";
