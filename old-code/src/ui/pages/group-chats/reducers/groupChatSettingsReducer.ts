export type GroupChatSettingsUiState = {
  loading: boolean;
  error: string | null;
  editingName: boolean;
  nameDraft: string;
  showPersonaSelector: boolean;
  showAddCharacter: boolean;
  showRemoveConfirm: string | null;
  saving: boolean;
};

export type GroupChatSettingsUiAction =
  | { type: "PATCH"; patch: Partial<GroupChatSettingsUiState> };

export const initialGroupChatSettingsUiState: GroupChatSettingsUiState = {
  loading: true,
  error: null,
  editingName: false,
  nameDraft: "",
  showPersonaSelector: false,
  showAddCharacter: false,
  showRemoveConfirm: null,
  saving: false,
};

export function groupChatSettingsUiReducer(
  state: GroupChatSettingsUiState,
  action: GroupChatSettingsUiAction,
): GroupChatSettingsUiState {
  switch (action.type) {
    case "PATCH":
      return { ...state, ...action.patch };
    default:
      return state;
  }
}
