import type {
  Character,
  Persona,
  Session,
  StoredMessage,
  ImageAttachment,
} from "../../../../core/storage/schemas";

const MAX_STREAMING_REASONING_CHARS = 200_000;

function pruneStreamingReasoning(
  streamingReasoning: Record<string, string>,
  messages: StoredMessage[],
): Record<string, string> {
  if (Object.keys(streamingReasoning).length === 0) return streamingReasoning;
  const validIds = new Set(messages.map((message) => message.id));
  const next: Record<string, string> = {};
  for (const [messageId, reasoning] of Object.entries(streamingReasoning)) {
    if (validIds.has(messageId)) {
      next[messageId] = reasoning;
    }
  }
  return next;
}

export interface MessageActionState {
  message: StoredMessage;
  mode: "view" | "edit";
}

export interface ChatState {
  // Core data
  character: Character | null;
  persona: Persona | null;
  session: Session | null;
  messages: StoredMessage[];

  // UI state
  draft: string;
  loading: boolean;
  sending: boolean;
  error: string | null;

  // Message actions
  messageAction: MessageActionState | null;
  actionError: string | null;
  actionStatus: string | null;
  actionBusy: boolean;
  editDraft: string;

  // Interaction state
  heldMessageId: string | null;
  regeneratingMessageId: string | null;
  activeRequestId: string | null;

  // Attachments
  pendingAttachments: ImageAttachment[];

  // Streaming reasoning (for thinking models)
  streamingReasoning: Record<string, string>;
}

export type ChatAction =
  | { type: "BATCH"; actions: ChatAction[] }
  | { type: "SET_CHARACTER"; payload: Character | null }
  | { type: "SET_PERSONA"; payload: Persona | null }
  | { type: "SET_SESSION"; payload: Session | null }
  | { type: "SET_MESSAGES"; payload: StoredMessage[] }
  | { type: "SET_DRAFT"; payload: string }
  | { type: "SET_LOADING"; payload: boolean }
  | { type: "SET_SENDING"; payload: boolean }
  | { type: "SET_ERROR"; payload: string | null }
  | { type: "SET_STREAMING_REASONING"; payload: Record<string, string> }
  | { type: "SET_MESSAGE_ACTION"; payload: MessageActionState | null }
  | { type: "SET_ACTION_ERROR"; payload: string | null }
  | { type: "SET_ACTION_STATUS"; payload: string | null }
  | { type: "SET_ACTION_BUSY"; payload: boolean }
  | { type: "SET_EDIT_DRAFT"; payload: string }
  | { type: "SET_HELD_MESSAGE_ID"; payload: string | null }
  | { type: "SET_REGENERATING_MESSAGE_ID"; payload: string | null }
  | { type: "SET_ACTIVE_REQUEST_ID"; payload: string | null }
  | { type: "RESET_MESSAGE_ACTIONS" }
  | { type: "UPDATE_MESSAGE_CONTENT"; payload: { messageId: string; content: string } }
  | {
      type: "REPLACE_PLACEHOLDER_MESSAGES";
      payload: {
        userPlaceholder: StoredMessage;
        assistantPlaceholder: StoredMessage;
        userMessage: StoredMessage;
        assistantMessage: StoredMessage;
      };
    }
  | { type: "REWIND_TO_MESSAGE"; payload: { messageId: string; messages: StoredMessage[] } }
  | { type: "SET_PENDING_ATTACHMENTS"; payload: ImageAttachment[] }
  | { type: "ADD_PENDING_ATTACHMENT"; payload: ImageAttachment }
  | { type: "REMOVE_PENDING_ATTACHMENT"; payload: string }
  | { type: "CLEAR_PENDING_ATTACHMENTS" }
  | { type: "CLEAR_DRAFT" }
  | { type: "UPDATE_MESSAGE_REASONING"; payload: { messageId: string; reasoning: string } }
  | { type: "CLEAR_STREAMING_REASONING"; payload: string }
  | { type: "TRANSFER_REASONING"; payload: { fromId: string; toId: string } };

export const initialChatState: ChatState = {
  character: null,
  persona: null,
  session: null,
  messages: [],
  draft: "",
  loading: true,
  sending: false,
  error: null,
  messageAction: null,
  actionError: null,
  actionStatus: null,
  actionBusy: false,
  editDraft: "",
  heldMessageId: null,
  regeneratingMessageId: null,
  activeRequestId: null,
  pendingAttachments: [],
  streamingReasoning: {},
};

export function chatReducer(state: ChatState, action: ChatAction): ChatState {
  if (action.type === "BATCH") {
    return action.actions.reduce(chatReducer, state);
  }

  switch (action.type) {
    case "SET_CHARACTER":
      return { ...state, character: action.payload };

    case "SET_PERSONA":
      return { ...state, persona: action.payload };

    case "SET_SESSION":
      return { ...state, session: action.payload };

    case "SET_MESSAGES":
      return {
        ...state,
        messages: action.payload,
        streamingReasoning: pruneStreamingReasoning(state.streamingReasoning, action.payload),
      };

    case "SET_DRAFT":
      return { ...state, draft: action.payload };

    case "SET_LOADING":
      return { ...state, loading: action.payload };

    case "SET_SENDING":
      return { ...state, sending: action.payload };

    case "SET_ERROR":
      return { ...state, error: action.payload };

    case "SET_STREAMING_REASONING":
      return {
        ...state,
        streamingReasoning: pruneStreamingReasoning(action.payload, state.messages),
      };

    case "SET_MESSAGE_ACTION":
      return { ...state, messageAction: action.payload };

    case "SET_ACTION_ERROR":
      return { ...state, actionError: action.payload };

    case "SET_ACTION_STATUS":
      return { ...state, actionStatus: action.payload };

    case "SET_ACTION_BUSY":
      return { ...state, actionBusy: action.payload };

    case "SET_EDIT_DRAFT":
      return { ...state, editDraft: action.payload };

    case "SET_HELD_MESSAGE_ID":
      return { ...state, heldMessageId: action.payload };

    case "SET_REGENERATING_MESSAGE_ID":
      return { ...state, regeneratingMessageId: action.payload };

    case "SET_ACTIVE_REQUEST_ID":
      return { ...state, activeRequestId: action.payload };

    case "RESET_MESSAGE_ACTIONS":
      return {
        ...state,
        messageAction: null,
        editDraft: "",
        actionError: null,
        actionStatus: null,
      };

    case "UPDATE_MESSAGE_CONTENT":
      return {
        ...state,
        messages: state.messages.map((msg) =>
          msg.id === action.payload.messageId
            ? { ...msg, content: msg.content + action.payload.content }
            : msg,
        ),
      };

    case "REPLACE_PLACEHOLDER_MESSAGES":
      const { userPlaceholder, assistantPlaceholder, userMessage, assistantMessage } =
        action.payload;
      return {
        ...state,
        messages: state.messages.map((msg) => {
          if (msg.id === userPlaceholder.id) return userMessage;
          if (msg.id === assistantPlaceholder.id) return assistantMessage;
          return msg;
        }),
      };

    case "REWIND_TO_MESSAGE":
      return {
        ...state,
        messages: action.payload.messages,
        streamingReasoning: pruneStreamingReasoning(
          state.streamingReasoning,
          action.payload.messages,
        ),
      };

    case "SET_PENDING_ATTACHMENTS":
      return { ...state, pendingAttachments: action.payload };

    case "ADD_PENDING_ATTACHMENT":
      return { ...state, pendingAttachments: [...state.pendingAttachments, action.payload] };

    case "REMOVE_PENDING_ATTACHMENT":
      return {
        ...state,
        pendingAttachments: state.pendingAttachments.filter((a) => a.id !== action.payload),
      };

    case "CLEAR_PENDING_ATTACHMENTS":
      return { ...state, pendingAttachments: [] };

    case "CLEAR_DRAFT":
      return { ...state, draft: "" };

    case "UPDATE_MESSAGE_REASONING":
      const existingReasoning = state.streamingReasoning[action.payload.messageId] || "";
      const combinedReasoning = (existingReasoning + action.payload.reasoning).slice(
        0,
        MAX_STREAMING_REASONING_CHARS,
      );
      return {
        ...state,
        streamingReasoning: {
          ...state.streamingReasoning,
          [action.payload.messageId]: combinedReasoning,
        },
      };

    case "CLEAR_STREAMING_REASONING":
      const { [action.payload]: _, ...remainingReasoning } = state.streamingReasoning;
      return { ...state, streamingReasoning: remainingReasoning };

    case "TRANSFER_REASONING": {
      const reasoning = state.streamingReasoning[action.payload.fromId];
      if (!reasoning) return state;
      const { [action.payload.fromId]: removed, ...rest } = state.streamingReasoning;
      return {
        ...state,
        streamingReasoning: {
          ...rest,
          [action.payload.toId]: reasoning,
        },
      };
    }

    default:
      return state;
  }
}
