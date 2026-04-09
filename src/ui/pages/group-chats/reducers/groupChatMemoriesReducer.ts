export type MemoriesTab = "memories" | "tools" | "pinned";
export type RetryStatus = "idle" | "retrying" | "success";
export type MemoryStatus = "idle" | "processing" | "failed";

export type UiState = {
  activeTab: MemoriesTab;
  searchTerm: string;
  editingIndex: number | null;
  editingValue: string;
  newMemory: string;
  isAdding: boolean;
  summaryDraft: string;
  summaryDirty: boolean;
  isSavingSummary: boolean;
  retryStatus: RetryStatus;
  actionError: string | null;
  expandedMemories: Set<number>;
  memoryTempBusy: number | null;
  pendingRefresh: boolean;
  memoryStatus: MemoryStatus;
  memoryProgressStep: number | null;
  selectedMemoryId: string | null;
  memoryActionMode: "actions" | "edit" | null;
};

export type UiAction =
  | { type: "SET_TAB"; tab: MemoriesTab }
  | { type: "SET_SEARCH"; value: string }
  | { type: "CLEAR_SEARCH" }
  | { type: "START_EDIT"; index: number; text: string }
  | { type: "SET_EDIT_VALUE"; value: string }
  | { type: "CANCEL_EDIT" }
  | { type: "SET_NEW_MEMORY"; value: string }
  | { type: "SET_IS_ADDING"; value: boolean }
  | { type: "SET_SUMMARY_DRAFT"; value: string }
  | { type: "SYNC_SUMMARY_FROM_SESSION"; value: string }
  | { type: "SET_IS_SAVING_SUMMARY"; value: boolean }
  | { type: "MARK_SUMMARY_SAVED" }
  | { type: "SET_RETRY_STATUS"; value: RetryStatus }
  | { type: "SET_ACTION_ERROR"; value: string | null }
  | { type: "TOGGLE_EXPANDED"; index: number }
  | { type: "SHIFT_EXPANDED_AFTER_DELETE"; index: number }
  | { type: "SET_MEMORY_TEMP_BUSY"; value: number | null }
  | { type: "SET_PENDING_REFRESH"; value: boolean }
  | { type: "SET_MEMORY_STATUS"; value: MemoryStatus }
  | { type: "SET_MEMORY_PROGRESS_STEP"; value: number | null }
  | { type: "OPEN_MEMORY_ACTIONS"; id: string }
  | { type: "SET_MEMORY_ACTION_MODE"; mode: "actions" | "edit" }
  | { type: "CLOSE_MEMORY_ACTIONS" };

export function initUi(): UiState {
  return {
    activeTab: "memories",
    searchTerm: "",
    editingIndex: null,
    editingValue: "",
    newMemory: "",
    isAdding: false,
    summaryDraft: "",
    summaryDirty: false,
    isSavingSummary: false,
    retryStatus: "idle",
    actionError: null,
    expandedMemories: new Set<number>(),
    memoryTempBusy: null,
    pendingRefresh: false,
    memoryStatus: "idle",
    memoryProgressStep: null,
    selectedMemoryId: null,
    memoryActionMode: null,
  };
}

export function uiReducer(state: UiState, action: UiAction): UiState {
  switch (action.type) {
    case "SET_TAB":
      return { ...state, activeTab: action.tab };
    case "SET_SEARCH":
      return { ...state, searchTerm: action.value };
    case "CLEAR_SEARCH":
      return { ...state, searchTerm: "" };
    case "START_EDIT":
      return { ...state, editingIndex: action.index, editingValue: action.text };
    case "SET_EDIT_VALUE":
      return { ...state, editingValue: action.value };
    case "CANCEL_EDIT":
      return { ...state, editingIndex: null, editingValue: "" };
    case "SET_NEW_MEMORY":
      return { ...state, newMemory: action.value };
    case "SET_IS_ADDING":
      return { ...state, isAdding: action.value };
    case "SET_SUMMARY_DRAFT":
      return { ...state, summaryDraft: action.value, summaryDirty: true };
    case "SYNC_SUMMARY_FROM_SESSION":
      if (state.summaryDirty) return state;
      return { ...state, summaryDraft: action.value };
    case "SET_IS_SAVING_SUMMARY":
      return { ...state, isSavingSummary: action.value };
    case "MARK_SUMMARY_SAVED":
      return { ...state, summaryDirty: false, isSavingSummary: false };
    case "SET_RETRY_STATUS":
      return { ...state, retryStatus: action.value };
    case "SET_ACTION_ERROR":
      return { ...state, actionError: action.value };
    case "TOGGLE_EXPANDED": {
      const next = new Set(state.expandedMemories);
      if (next.has(action.index)) next.delete(action.index);
      else next.add(action.index);
      return { ...state, expandedMemories: next };
    }
    case "SHIFT_EXPANDED_AFTER_DELETE": {
      if (state.expandedMemories.size === 0) return state;
      const next = new Set<number>();
      for (const idx of state.expandedMemories) {
        if (idx === action.index) continue;
        next.add(idx > action.index ? idx - 1 : idx);
      }
      return { ...state, expandedMemories: next };
    }
    case "SET_MEMORY_TEMP_BUSY":
      return { ...state, memoryTempBusy: action.value };
    case "SET_PENDING_REFRESH":
      return { ...state, pendingRefresh: action.value };
    case "SET_MEMORY_STATUS":
      return {
        ...state,
        memoryStatus: action.value,
        memoryProgressStep: action.value === "idle" || action.value === "failed" ? null : state.memoryProgressStep,
      };
    case "SET_MEMORY_PROGRESS_STEP":
      return { ...state, memoryProgressStep: action.value };
    case "OPEN_MEMORY_ACTIONS":
      return { ...state, selectedMemoryId: action.id, memoryActionMode: "actions" };
    case "SET_MEMORY_ACTION_MODE":
      return { ...state, memoryActionMode: action.mode };
    case "CLOSE_MEMORY_ACTIONS":
      return { ...state, selectedMemoryId: null, memoryActionMode: null, editingIndex: null, editingValue: "" };
    default:
      return state;
  }
}
