export interface CustomProviderDraft {
  chatPath: string;
  fetchModels: boolean;
  modelsPath: string;
  listPath: string;
  idPath: string;
  namePath: string;
  descriptionPath: string;
  contextPath: string;
  auth: "bearer" | "header" | "query" | "none";
  headerName: string;
  queryName: string;
  systemRole: string;
  userRole: string;
  assistantRole: string;
  supportsStreaming: boolean;
  mergeMessages: boolean;
  templateKwargs: boolean;
  toolChoice: "auto" | "required" | "none" | "omit" | "passthrough";
}

function record(value: unknown): Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value) ? value as Record<string, unknown> : {};
}
function text(value: unknown, fallback = ""): string {
  return typeof value === "string" ? value : fallback;
}
function optional(value: unknown, fallback: string): string {
  return value === null ? "" : text(value, fallback);
}
export function customProviderDraft(kind: string, config: Record<string, unknown> = {}): CustomProviderDraft {
  const auth = record(config.auth);
  const list = record(config.model_list);
  const roles = record(config.roles);
  const mode = config.auth === "bearer" || config.auth === "none" ? config.auth : "header" in auth ? "header" : "query" in auth ? "query" : kind === "custom-anthropic" ? "header" : "bearer";
  const tool = config.tool_choice_mode;
  return {
    chatPath: text(config.chat_path, kind === "custom-anthropic" ? "/v1/messages" : "/v1/chat/completions"),
    fetchModels: typeof config.models_path === "string",
    modelsPath: text(config.models_path, "/v1/models"),
    listPath: text(list.list_path, "data"),
    idPath: text(list.id_path, "id"),
    namePath: optional(list.display_name_path, "name"),
    descriptionPath: optional(list.description_path, "description"),
    contextPath: text(list.context_length_path),
    auth: mode,
    headerName: text(record(auth.header).name, "x-api-key"),
    queryName: text(record(auth.query).name, "api_key"),
    systemRole: text(roles.system),
    userRole: text(roles.user),
    assistantRole: text(roles.assistant),
    supportsStreaming: config.streaming !== false,
    mergeMessages: config.merge_same_role_messages !== false,
    templateKwargs: config.send_chat_template_kwargs === true,
    toolChoice: tool === "required" || tool === "none" || tool === "omit" || tool === "passthrough" ? tool : "auto",
  };
}
export function customProviderConfig(draft: CustomProviderDraft): Record<string, unknown> {
  return {
    kind: "custom",
    chat_path: draft.chatPath.trim(),
    models_path: draft.fetchModels ? draft.modelsPath.trim() : null,
    model_list: {
      list_path: draft.listPath.trim(), id_path: draft.idPath.trim(),
      display_name_path: draft.namePath.trim() || null,
      description_path: draft.descriptionPath.trim() || null,
      context_length_path: draft.contextPath.trim() || null,
    },
    auth: draft.auth === "header" ? { header: { name: draft.headerName.trim() } } : draft.auth === "query" ? { query: { name: draft.queryName.trim() } } : draft.auth,
    roles: { system: draft.systemRole.trim() || null, user: draft.userRole.trim() || null, assistant: draft.assistantRole.trim() || null },
    streaming: draft.supportsStreaming,
    merge_same_role_messages: draft.mergeMessages,
    send_chat_template_kwargs: draft.templateKwargs,
    tool_choice_mode: draft.toolChoice,
  };
}
