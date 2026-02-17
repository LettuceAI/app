export type EngineChatMessage = {
  id: string;
  role: "user" | "assistant";
  content: string;
  emotion?: string;
  emotionIntensity?: number;
  timestamp?: string;
};

export type EngineChatState = {
  loading: boolean;
  sending: boolean;
  error: string | null;
  messages: EngineChatMessage[];
  draft: string;
};

export const initialEngineChatState: EngineChatState = {
  loading: true,
  sending: false,
  error: null,
  messages: [],
  draft: "",
};

export type EngineChatAction =
  | { type: "set_loading"; payload: boolean }
  | { type: "set_sending"; payload: boolean }
  | { type: "set_error"; payload: string | null }
  | { type: "set_messages"; payload: EngineChatMessage[] }
  | { type: "append_message"; payload: EngineChatMessage }
  | { type: "set_draft"; payload: string };

export function engineChatReducer(
  state: EngineChatState,
  action: EngineChatAction,
): EngineChatState {
  switch (action.type) {
    case "set_loading":
      return { ...state, loading: action.payload };
    case "set_sending":
      return { ...state, sending: action.payload };
    case "set_error":
      return { ...state, error: action.payload };
    case "set_messages":
      return { ...state, messages: action.payload };
    case "append_message":
      return { ...state, messages: [...state.messages, action.payload] };
    case "set_draft":
      return { ...state, draft: action.payload };
    default:
      return state;
  }
}
