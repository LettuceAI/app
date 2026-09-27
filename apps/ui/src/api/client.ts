import type { ApiError, ApiErrorCode, ApiErrorDetails, AppEvent } from "./generated/bindings";
import type {
  CallCommandName,
  CommandArgs,
  CommandData,
  CommandName,
  CommandOutcome,
  StreamCommandName,
  StreamEvent,
  StreamRequest,
  Transport,
  TransportKind,
  Unsubscribe,
} from "./transport";

/** `transport` means the call never reached the backend or its reply could not be read. */
export type ApiFailureCode = ApiErrorCode | "transport";

/** Every failed backend call rejects with this. `message` is diagnostic text for logs, never UI copy. */
export class ApiFailure extends Error {
  readonly code: ApiFailureCode;
  readonly details: ApiErrorDetails | null;

  constructor(code: ApiFailureCode, message: string, details: ApiErrorDetails | null, options?: ErrorOptions) {
    super(message, options);
    this.name = "ApiFailure";
    this.code = code;
    this.details = details;
  }

  static fromApiError(error: ApiError): ApiFailure {
    return new ApiFailure(error.code, error.message, error.details);
  }

  static fromThrown(thrown: unknown): ApiFailure {
    if (thrown instanceof ApiFailure) return thrown;
    const message = thrown instanceof Error ? thrown.message : String(thrown);
    return new ApiFailure("transport", message, null, { cause: thrown });
  }
}

export function isApiFailure(value: unknown): value is ApiFailure {
  return value instanceof ApiFailure;
}

function unwrap<K extends CommandName>(outcome: CommandOutcome<K>): CommandData<K> {
  if (outcome.status === "ok") return outcome.data;
  throw ApiFailure.fromApiError(outcome.error);
}

async function settle<K extends CommandName>(pending: () => Promise<CommandOutcome<K>>): Promise<CommandData<K>> {
  let outcome: CommandOutcome<K>;
  try {
    outcome = await pending();
  } catch (thrown) {
    throw ApiFailure.fromThrown(thrown);
  }
  return unwrap(outcome);
}

export interface ApiClient {
  readonly transport: TransportKind;
  call<K extends CallCommandName>(command: K, ...args: CommandArgs<K>): Promise<CommandData<K>>;
  stream<K extends StreamCommandName>(
    command: K,
    request: StreamRequest<K>,
    onEvent: (event: StreamEvent<K>) => void,
  ): Promise<CommandData<K>>;
  subscribe(listener: (event: AppEvent) => void): Promise<Unsubscribe>;
}

export function createApiClient(transport: Transport): ApiClient {
  return {
    transport: transport.kind,
    call: (command, ...args) => settle(() => transport.call(command, ...args)),
    stream: (command, request, onEvent) => settle(() => transport.stream(command, request, onEvent)),
    subscribe: async (listener) => {
      try {
        return await transport.subscribe(listener);
      } catch (thrown) {
        throw ApiFailure.fromThrown(thrown);
      }
    },
  };
}

export interface TransportEnvironment {
  hasTauri: boolean;
  allowMock: boolean;
}

export function detectEnvironment(): TransportEnvironment {
  return {
    hasTauri: typeof window !== "undefined" && "__TAURI_INTERNALS__" in window,
    allowMock: import.meta.env.DEV,
  };
}

/**
 * Tauri when the page runs inside the shell; the in-memory mock only for development in a plain
 * browser. A production build outside the shell has no backend and fails typed.
 */
export async function selectTransport(environment: TransportEnvironment = detectEnvironment()): Promise<Transport> {
  if (environment.hasTauri) {
    const { createTauriTransport } = await import("./tauri-transport");
    return createTauriTransport();
  }
  if (environment.allowMock) {
    const { createMockTransport } = await import("./mock-transport");
    return createMockTransport();
  }
  throw new ApiFailure("transport", "no backend: the page is not running inside the app shell", null);
}
