import { useReducer, useCallback, useEffect } from "react";
import { storageBridge } from "../../../../core/storage/files";
import { setTooltipSeen } from "../../../../core/storage/appState";

export interface BackupInfo {
  version: number;
  createdAt: number;
  appVersion: string;
  encrypted: boolean;
  totalFiles: number;
  imageCount: number;
  avatarCount: number;
  attachmentCount: number;
  path: string;
  filename: string;
}

type ModalType = "export" | "import" | "delete" | null;

interface State {
  backups: BackupInfo[];
  loading: boolean;
  exporting: boolean;
  importing: boolean;
  activeModal: ModalType;
  selectedBackup: BackupInfo | null;
  /** True if selectedBackup was picked via file browser (uses byte-based import) */
  isPickedFile: boolean;
  exportPassword: string;
  confirmPassword: string;
  importPassword: string;
  showExportPassword: boolean;
  showImportPassword: boolean;
  exportSuccess: string | null;
  error: string | null;
}

type Action =
  | { type: "SET_BACKUPS"; payload: BackupInfo[] }
  | { type: "SET_LOADING"; payload: boolean }
  | { type: "SET_EXPORTING"; payload: boolean }
  | { type: "SET_IMPORTING"; payload: boolean }
  | { type: "OPEN_EXPORT_MODAL" }
  | { type: "OPEN_IMPORT_MODAL"; payload: BackupInfo }
  | { type: "OPEN_IMPORT_MODAL_PICKED"; payload: BackupInfo }
  | { type: "OPEN_DELETE_MODAL"; payload: BackupInfo }
  | { type: "CLOSE_MODAL" }
  | { type: "SET_EXPORT_PASSWORD"; payload: string }
  | { type: "SET_CONFIRM_PASSWORD"; payload: string }
  | { type: "SET_IMPORT_PASSWORD"; payload: string }
  | { type: "TOGGLE_SHOW_EXPORT_PASSWORD" }
  | { type: "TOGGLE_SHOW_IMPORT_PASSWORD" }
  | { type: "SET_EXPORT_SUCCESS"; payload: string | null }
  | { type: "SET_ERROR"; payload: string | null }
  | { type: "EXPORT_COMPLETE"; payload: string }
  | { type: "IMPORT_COMPLETE" }
  | { type: "DELETE_COMPLETE" };

const initialState: State = {
  backups: [],
  loading: true,
  exporting: false,
  importing: false,
  activeModal: null,
  selectedBackup: null,
  isPickedFile: false,
  exportPassword: "",
  confirmPassword: "",
  importPassword: "",
  showExportPassword: false,
  showImportPassword: false,
  exportSuccess: null,
  error: null,
};

function reducer(state: State, action: Action): State {
  switch (action.type) {
    case "SET_BACKUPS":
      return { ...state, backups: action.payload };
    case "SET_LOADING":
      return { ...state, loading: action.payload };
    case "SET_EXPORTING":
      return { ...state, exporting: action.payload };
    case "SET_IMPORTING":
      return { ...state, importing: action.payload };
    case "OPEN_EXPORT_MODAL":
      return {
        ...state,
        activeModal: "export",
        exportPassword: "",
        confirmPassword: "",
        error: null,
      };
    case "OPEN_IMPORT_MODAL":
      return {
        ...state,
        activeModal: "import",
        selectedBackup: action.payload,
        isPickedFile: false,
        importPassword: "",
        error: null,
      };
    case "OPEN_IMPORT_MODAL_PICKED":
      return {
        ...state,
        activeModal: "import",
        selectedBackup: action.payload,
        isPickedFile: true,
        importPassword: "",
        error: null,
      };
    case "OPEN_DELETE_MODAL":
      return {
        ...state,
        activeModal: "delete",
        selectedBackup: action.payload,
        error: null,
      };
    case "CLOSE_MODAL":
      return { ...state, activeModal: null };
    case "SET_EXPORT_PASSWORD":
      return { ...state, exportPassword: action.payload };
    case "SET_CONFIRM_PASSWORD":
      return { ...state, confirmPassword: action.payload };
    case "SET_IMPORT_PASSWORD":
      return { ...state, importPassword: action.payload };
    case "TOGGLE_SHOW_EXPORT_PASSWORD":
      return { ...state, showExportPassword: !state.showExportPassword };
    case "TOGGLE_SHOW_IMPORT_PASSWORD":
      return { ...state, showImportPassword: !state.showImportPassword };
    case "SET_EXPORT_SUCCESS":
      return { ...state, exportSuccess: action.payload };
    case "SET_ERROR":
      return { ...state, error: action.payload };
    case "EXPORT_COMPLETE":
      return {
        ...state,
        exporting: false,
        activeModal: null,
        exportPassword: "",
        confirmPassword: "",
        exportSuccess: action.payload,
      };
    case "IMPORT_COMPLETE":
      return {
        ...state,
        importing: false,
        activeModal: null,
        importPassword: "",
        selectedBackup: null,
        isPickedFile: false,
      };
    case "DELETE_COMPLETE":
      return {
        ...state,
        activeModal: null,
        selectedBackup: null,
        isPickedFile: false,
      };
    default:
      return state;
  }
}

export function useBackupRestore() {
  const [state, dispatch] = useReducer(reducer, initialState);

  const loadBackups = useCallback(async () => {
    try {
      dispatch({ type: "SET_LOADING", payload: true });
      const list = await storageBridge.backupList();
      dispatch({ type: "SET_BACKUPS", payload: list });
    } catch (e) {
      console.error("Failed to load backups:", e);
    } finally {
      dispatch({ type: "SET_LOADING", payload: false });
    }
  }, []);

  useEffect(() => {
    loadBackups();
  }, [loadBackups]);

  const handleExport = useCallback(async () => {
    if (state.exportPassword.length < 6) {
      dispatch({ type: "SET_ERROR", payload: "Password must be at least 6 characters" });
      return;
    }
    if (state.exportPassword !== state.confirmPassword) {
      dispatch({ type: "SET_ERROR", payload: "Passwords do not match" });
      return;
    }

    try {
      dispatch({ type: "SET_ERROR", payload: null });
      dispatch({ type: "SET_EXPORTING", payload: true });
      const path = await storageBridge.backupExport(state.exportPassword);
      dispatch({ type: "EXPORT_COMPLETE", payload: path });
      await loadBackups();
    } catch (e) {
      dispatch({ type: "SET_ERROR", payload: e instanceof Error ? e.message : "Export failed" });
      dispatch({ type: "SET_EXPORTING", payload: false });
    }
  }, [state.exportPassword, state.confirmPassword, loadBackups]);

  const handleImport = useCallback(
    async (skipDynamicMemoryCheck = false) => {
      const { selectedBackup, importPassword } = state;
      if (!selectedBackup) return;

      if (selectedBackup.encrypted && importPassword.length < 1) {
        dispatch({ type: "SET_ERROR", payload: "Password is required for this backup" });
        return;
      }

      try {
        dispatch({ type: "SET_ERROR", payload: null });

        if (selectedBackup.encrypted) {
          const valid = await storageBridge.backupVerifyPassword(
            selectedBackup.path,
            importPassword,
          );
          if (!valid) {
            dispatch({ type: "SET_ERROR", payload: "Incorrect password" });
            return;
          }
        }

        dispatch({ type: "SET_IMPORTING", payload: true });

        // Import first
        await storageBridge.backupImport(
          selectedBackup.path,
          selectedBackup.encrypted ? importPassword : undefined,
        );

        dispatch({ type: "IMPORT_COMPLETE" });

        await setTooltipSeen("app_tour_v1");
        await setTooltipSeen("chat_detail_tour_v1");
        await setTooltipSeen("post_first_message_tour_v1");


        if (!skipDynamicMemoryCheck) {
          console.log("[useBackupRestore] Checking for dynamic memory in backup...");
          const hasDynamicMemory = await storageBridge.backupCheckDynamicMemory(
            selectedBackup.path,
            selectedBackup.encrypted ? importPassword : undefined,
          );

          console.log("[useBackupRestore] hasDynamicMemory:", hasDynamicMemory);

          if (hasDynamicMemory) {
            console.log("[useBackupRestore] Checking if embedding model exists...");
            const hasEmbeddingModel = await storageBridge.checkEmbeddingModel();
            console.log("[useBackupRestore] hasEmbeddingModel:", hasEmbeddingModel);

            if (!hasEmbeddingModel) {
              console.log("[useBackupRestore] Returning needsEmbeddingModel=true");
              return { success: true, needsEmbeddingModel: true };
            }
          }
        }

        return { success: true };
      } catch (e) {
        dispatch({ type: "SET_ERROR", payload: e instanceof Error ? e.message : "Import failed" });
        dispatch({ type: "SET_IMPORTING", payload: false });
        return { error: true };
      }
    },
    [state],
  );

  const handleDelete = useCallback(async () => {
    if (!state.selectedBackup) return;

    try {
      dispatch({ type: "SET_ERROR", payload: null });
      await storageBridge.backupDelete(state.selectedBackup.path);
      dispatch({ type: "DELETE_COMPLETE" });
      await loadBackups();
    } catch (e) {
      dispatch({ type: "SET_ERROR", payload: e instanceof Error ? e.message : "Delete failed" });
    }
  }, [state.selectedBackup, loadBackups]);

  const handleBrowseForBackup = useCallback(async () => {
    try {
      const result = await storageBridge.backupPickFile();
      if (!result) return;

      const { path, filename } = result;

      const info = await storageBridge.backupGetInfo(path);

      const backupInfo: BackupInfo = {
        ...info,
        path,
        filename,
      };

      dispatch({ type: "OPEN_IMPORT_MODAL", payload: backupInfo });
    } catch (e) {
      console.error("Failed to browse for backup:", e);
      dispatch({
        type: "SET_ERROR",
        payload: e instanceof Error ? e.message : "Failed to open file",
      });
    }
  }, []);

  const actions = {
    openExportModal: () => dispatch({ type: "OPEN_EXPORT_MODAL" }),
    openImportModal: (backup: BackupInfo) =>
      dispatch({ type: "OPEN_IMPORT_MODAL", payload: backup }),
    openDeleteModal: (backup: BackupInfo) =>
      dispatch({ type: "OPEN_DELETE_MODAL", payload: backup }),
    closeModal: () => {
      dispatch({ type: "CLOSE_MODAL" });
    },
    setExportPassword: (value: string) => dispatch({ type: "SET_EXPORT_PASSWORD", payload: value }),
    setConfirmPassword: (value: string) =>
      dispatch({ type: "SET_CONFIRM_PASSWORD", payload: value }),
    setImportPassword: (value: string) => dispatch({ type: "SET_IMPORT_PASSWORD", payload: value }),
    toggleShowExportPassword: () => dispatch({ type: "TOGGLE_SHOW_EXPORT_PASSWORD" }),
    toggleShowImportPassword: () => dispatch({ type: "TOGGLE_SHOW_IMPORT_PASSWORD" }),
    clearExportSuccess: () => dispatch({ type: "SET_EXPORT_SUCCESS", payload: null }),
    handleExport,
    handleImport,
    handleDelete,
    handleBrowseForBackup,
  };

  return { state, actions };
}
