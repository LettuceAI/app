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
import { deleteImageRef } from "../../../core/storage";
import type { Character, Persona, Lorebook } from "../../../core/storage/schemas";
import { typography, interactive, cn } from "../../design-tokens";
import { useAvatar } from "../../hooks/useAvatar";
import { useAvatarGradient } from "../../hooks/useAvatarGradient";
import { useRocketEasterEgg } from "../../hooks/useRocketEasterEgg";
import { useLocation, useNavigate } from "react-router-dom";
import { BottomMenu, CharacterExportMenu } from "../../components";
import { LorebookAvatar } from "../../components/LorebookAvatar";
import { ImageLibraryPanel } from "./ImageLibraryPage";
import {
  MessageCircle,
  Edit2,
  Trash2,
  Download,
  Upload,
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
import { useI18n } from "../../../core/i18n/context";
import { isRenderableImageUrl } from "../../../core/utils/image";

type FilterOption = "All" | "Characters" | "Personas" | "Lorebooks" | "Images";
type LibraryItem = (Character | Persona | Lorebook) & {
  itemType: "character" | "persona" | "lorebook";
};

const FILTER_QUERY_VALUES: Record<FilterOption, string | null> = {
  All: null,
  Characters: "characters",
  Personas: "personas",
  Lorebooks: "lorebooks",
  Images: "images",
};

function resolveLibraryFilter(search: string): FilterOption {
  const params = new URLSearchParams(search);
  const view = params.get("view");
  const matched = (
    Object.entries(FILTER_QUERY_VALUES) as Array<[FilterOption, string | null]>
  ).find(([, value]) => value === view);
  return matched?.[0] ?? "All";
}

function getItemName(item: LibraryItem): string {
  if (item.itemType === "character") return (item as Character).name;
  if (item.itemType === "persona") return (item as Persona).title;
  return (item as Lorebook).name;
}

function getItemDisableGradient(item: LibraryItem): boolean | undefined {
  return item.itemType === "character" ? (item as Character).disableAvatarGradient : undefined;
}

export function LibraryPage() {
  const { t } = useI18n();
  const location = useLocation();
  const [characters, setCharacters] = useState<Character[]>([]);
  const [personas, setPersonas] = useState<Persona[]>([]);
  const [lorebooks, setLorebooks] = useState<Lorebook[]>([]);
  const [filter, setFilter] = useState<FilterOption>(() => resolveLibraryFilter(location.search));
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
  const mainRef = useRef<HTMLElement | null>(null);

  useEffect(() => {
    const next = resolveLibraryFilter(location.search);
    setFilter((current) => (current === next ? current : next));
  }, [location.search]);

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

  const handleRenameConfirm = async () => {
    if (!renameItem || !renameName.trim()) return;

    try {
      setRenaming(true);
      // Only lorebooks can be renamed this way for now
      if (renameItem.itemType === "lorebook") {
        await saveLorebook({
          id: renameItem.id,
          name: renameName.trim(),
          avatarPath: renameItem.avatarPath,
        });
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
        (selectedItem as Character).defaultSceneId ?? (selectedItem as Character).scenes?.[0]?.id;
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
        navigate(`/personas/${selectedItem.id}/edit`);
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
        if (selectedItem.avatarPath) {
          await deleteImageRef(selectedItem.avatarPath);
        }
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
      const imported = await importLorebook(raw, file.name);
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

  const setLibraryFilter = (next: FilterOption) => {
    setFilter(next);
    const params = new URLSearchParams(location.search);
    const nextView = FILTER_QUERY_VALUES[next];
    if (nextView) {
      params.set("view", nextView);
    } else {
      params.delete("view");
    }
    const search = params.toString();
    navigate(
      {
        pathname: "/library",
        search: search ? `?${search}` : "",
      },
      { replace: true },
    );
  };

  return (
    <div className="flex h-full flex-col pb-6 text-fg/80">
      <main ref={mainRef} className="flex-1 overflow-y-auto px-4 pt-4">
        <div className="mb-4 flex gap-2 overflow-x-auto pb-1">
          {(["All", "Characters", "Personas", "Lorebooks", "Images"] as FilterOption[]).map(
            (option) => {
              const filterLabels: Record<FilterOption, string> = {
                All: t("library.filters.all"),
                Characters: t("library.filters.characters"),
                Personas: t("library.filters.personas"),
                Lorebooks: t("library.filters.lorebooks"),
                Images: "Images",
              };

              return (
                <button
                  key={option}
                  type="button"
                  onClick={() => setLibraryFilter(option)}
                  className={cn(
                    "shrink-0 rounded-xl border px-4 py-2 text-sm font-medium transition",
                    filter === option
                      ? "border-fg/15 bg-fg/10 text-fg"
                      : "border-fg/10 bg-surface-el/40 text-fg/60 hover:bg-fg/5 hover:text-fg",
                  )}
                >
                  {filterLabels[option]}
                </button>
              );
            },
          )}
        </div>

        {filter === "Images" ? (
          <ImageLibraryPanel embedded scrollContainerRef={mainRef} />
        ) : filteredItems.length === 0 ? (
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
                ? t("library.emptyStates.all.title")
                : filter === "Characters"
                  ? t("library.emptyStates.characters.title")
                  : filter === "Personas"
                    ? t("library.emptyStates.personas.title")
                    : t("library.emptyStates.lorebooks.title")}
            </h3>
            <p className="mb-6 max-w-70 text-center text-sm text-fg/50">
              {filter === "All"
                ? t("library.emptyStates.all.description")
                : filter === "Characters"
                  ? t("library.emptyStates.characters.description")
                  : filter === "Personas"
                    ? t("library.emptyStates.personas.description")
                    : t("library.emptyStates.lorebooks.description")}
            </p>
            {filter === "Lorebooks" && (
              <button
                onClick={() => lorebookImportRef.current?.click()}
                disabled={importingLorebook}
                className="mb-3 flex items-center gap-2 rounded-xl border border-info/40 bg-info/20 px-5 py-2.5 text-sm font-medium text-info transition active:scale-95 active:bg-info/30 disabled:opacity-60 disabled:cursor-not-allowed"
              >
                <Upload className="h-4 w-4" />
                {importingLorebook
                  ? t("common.buttons.importing")
                  : t("library.actions.importLorebook")}
              </button>
            )}
            {filter !== "Lorebooks" && (
              <button
                onClick={() =>
                  navigate(filter === "Personas" ? "/create/persona" : "/characters/create")
                }
                className="flex items-center gap-2 rounded-xl border border-accent/40 bg-accent/20 px-5 py-2.5 text-sm font-medium text-accent/70 transition active:scale-95 active:bg-accent/30"
              >
                <Users className="h-4 w-4" />
                {filter === "Personas"
                  ? t("personas.empty.createButton")
                  : t("characters.empty.createButton")}
              </button>
            )}
            <input
              ref={lorebookImportRef}
              type="file"
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
                <span className="text-sm font-medium text-accent">
                  {t("library.actions.startChat")}
                </span>
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
                  ? t("library.actions.editCharacter")
                  : selectedItem.itemType === "persona"
                    ? t("library.actions.editPersona")
                    : t("library.actions.editLorebook")}
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
                <span className="text-sm font-medium text-fg">
                  {t("library.actions.renameLorebook")}
                </span>
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
                  {exporting ? t("common.buttons.exporting") : t("library.actions.exportCharacter")}
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
                <span className="text-sm font-medium text-secondary">
                  {t("library.actions.chatAppearance")}
                </span>
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
                  {exporting ? t("common.buttons.exporting") : t("library.actions.exportPersona")}
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
                  ? t("library.actions.deleteCharacter")
                  : selectedItem.itemType === "persona"
                    ? t("library.actions.deletePersona")
                    : t("library.actions.deleteLorebook")}
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
        title={t("library.deleteConfirm.title", {
          itemType:
            selectedItem?.itemType === "character"
              ? t("library.itemTypes.character")
              : selectedItem?.itemType === "persona"
                ? t("library.itemTypes.persona")
                : t("library.itemTypes.lorebook"),
        })}
      >
        <div className="space-y-4">
          <p className="text-sm text-fg/70">
            {t("library.deleteConfirm.message")} &quot;
            {selectedItem ? getItemName(selectedItem) : ""}&quot;?
            {selectedItem?.itemType === "character" &&
              ` ${t("library.deleteConfirm.characterWarning")}`}
          </p>
          <div className="flex gap-3">
            <button
              onClick={() => setShowDeleteConfirm(false)}
              disabled={deleting}
              className="flex-1 rounded-xl border border-fg/10 bg-fg/5 py-3 text-sm font-medium text-fg transition hover:border-fg/20 hover:bg-fg/10 disabled:opacity-50"
            >
              {t("common.buttons.cancel")}
            </button>
            <button
              onClick={handleDelete}
              disabled={deleting}
              className="flex-1 rounded-xl border border-danger/30 bg-danger/20 py-3 text-sm font-medium text-danger transition hover:bg-danger/30 disabled:opacity-50"
            >
              {deleting ? t("common.buttons.deleting") : t("common.buttons.delete")}
            </button>
          </div>
        </div>
      </BottomMenu>
      {/* Rename Menu */}
      <BottomMenu
        isOpen={Boolean(renameItem)}
        onClose={() => setRenameItem(null)}
        title={t("library.rename.title")}
      >
        <div className="space-y-4">
          <input
            value={renameName}
            onChange={(e) => setRenameName(e.target.value)}
            placeholder={t("library.rename.placeholder")}
            className="w-full rounded-xl border border-fg/10 bg-surface-el/20 px-4 py-3 text-base text-fg placeholder-fg/40 transition focus:border-fg/25 focus:outline-none"
            autoFocus
          />
          <div className="flex gap-3">
            <button
              onClick={() => setRenameItem(null)}
              className="flex-1 rounded-xl border border-fg/10 bg-fg/5 py-3 text-sm font-medium text-fg transition hover:border-fg/20 hover:bg-fg/10"
            >
              {t("common.buttons.cancel")}
            </button>
            <button
              onClick={handleRenameConfirm}
              disabled={renaming || !renameName.trim()}
              className="flex-1 rounded-xl border border-accent/30 bg-accent/20 py-3 text-sm font-medium text-accent/70 transition hover:border-accent/50 hover:bg-accent/30 disabled:opacity-50"
            >
              {renaming ? t("common.buttons.saving") : t("common.buttons.save")}
            </button>
          </div>
        </div>
      </BottomMenu>
    </div>
  );
}

function isImageLike(s?: string) {
  return isRenderableImageUrl(s);
}

function getItemAvatarPath(item: LibraryItem): string | undefined {
  if (item.itemType === "lorebook") return undefined;
  return (item as Character | Persona).avatarPath;
}

function getItemDescription(item: LibraryItem, t?: (key: any) => string): string {
  if (item.itemType === "lorebook") return t ? t("library.lorebookLabel") : "Lorebook";
  if (item.itemType === "character") {
    const character = item as Character;
    return (
      (character.description || character.definition || "").trim() ||
      (t ? t("library.noDescriptionYet") : "No description yet")
    );
  }
  const persona = item as Persona;
  return persona.description.trim() || (t ? t("library.noDescriptionYet") : "No description yet");
}

const ItemAvatar = memo(({ item, className }: { item: LibraryItem; className?: string }) => {
  if (item.itemType === "lorebook") {
    return (
      <LorebookAvatar
        avatarPath={(item as Lorebook).avatarPath}
        name={getItemName(item)}
        className={className}
      />
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
    const { t } = useI18n();
    const descriptionPreview = getItemDescription(item, t);
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
        ? { label: t("library.itemTypes.character"), dotClass: "bg-info" }
        : item.itemType === "persona"
          ? { label: t("library.itemTypes.persona"), dotClass: "bg-secondary" }
          : { label: t("library.itemTypes.lorebook"), dotClass: "bg-warning" };

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
          <div className="flex items-center gap-1.5 overflow-hidden">
            <h3 className={cn(typography.body.size, "font-bold text-fg truncate leading-tight")}>
              {getItemName(item)}
            </h3>
            {item.itemType === "persona" && (item as Persona).nickname && (
              <span className="shrink-0 rounded-full border border-fg/15 bg-fg/5 px-2 py-0.5 text-[9px] font-medium uppercase tracking-wider text-fg/60">
                {(item as Persona).nickname}
              </span>
            )}
          </div>
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
