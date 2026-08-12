import type { GroupMessage } from "../../../../core/storage/schemas";

export type SendingStatus = "selecting" | "generating" | null;

export type MessageActionState = {
  message: GroupMessage;
  mode: "view" | "edit";
};

export type GroupChatUiState = {
  showParticipation: boolean;
  loading: boolean;
  sending: boolean;
  sendingStatus: SendingStatus;
  selectedCharacterId: string | null;
  selectedCharacterName: string | null;
  selectedCharacterAvatarUrl: string | null;
  regeneratingMessageId: string | null;
  error: string | null;
  draft: string;
  messageAction: MessageActionState | null;
  editDraft: string;
  actionBusy: boolean;
  actionStatus: string | null;
  actionError: string | null;
  heldMessageId: string | null;
};

export type GroupChatUiAction =
  | { type: "PATCH"; patch: Partial<GroupChatUiState> };

export const initialGroupChatUiState: GroupChatUiState = {
  showParticipation: true,
  loading: true,
  sending: false,
  sendingStatus: null,
  selectedCharacterId: null,
  selectedCharacterName: null,
  selectedCharacterAvatarUrl: null,
  regeneratingMessageId: null,
  error: null,
  draft: "",
  messageAction: null,
  editDraft: "",
  actionBusy: false,
  actionStatus: null,
  actionError: null,
  heldMessageId: null,
};

export function groupChatUiReducer(
  state: GroupChatUiState,
  action: GroupChatUiAction,
): GroupChatUiState {
  switch (action.type) {
    case "PATCH":
      return { ...state, ...action.patch };
    default:
      return state;
  }
}
