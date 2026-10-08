import type { ApiError, ApiErrorCode, ApiErrorDetails, AppEvent } from "./generated/bindings";
import type {
  CallCommandName,
  CommandArgs,
  CommandData,
  CommandName,
  CommandOutcome,
  StreamArgs,
  StreamCommandName,
  StreamEvent,
  StreamOptions,
  Transport,
  TransportKind,
  Unsubscribe,
} from "./transport";

/** `transport` means the call never reached the backend or its reply could not be read. */
export type ApiFailureCode = ApiErrorCode | "transport";

const apiErrorCodes = {
  not_found: true,
  in_use: true,
  malformed: true,
  conflict: true,
  invalid_input: true,
  unsupported: true,
  unavailable: true,
  cancelled: true,
  busy: true,
  internal: true,
  model_required: true,
  model_unavailable: true,
} satisfies Record<ApiErrorCode, true>;

/** An `ApiError` the backend produced, as opposed to an IPC rejection such as an unknown command. */
export function isApiError(value: unknown): value is ApiError {
  if (typeof value !== "object" || value === null) return false;
  const candidate = value as Record<string, unknown>;
  return (
    typeof candidate.code === "string" &&
    Object.hasOwn(apiErrorCodes, candidate.code) &&
    typeof candidate.message === "string" &&
    (candidate.details === null || typeof candidate.details === "object")
  );
}

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

  /** A backend `ApiError` keeps its code and details; anything else is a transport failure. */
  static fromRejection(rejection: unknown): ApiFailure {
    if (rejection instanceof ApiFailure) return rejection;
    if (isApiError(rejection)) return new ApiFailure(rejection.code, rejection.message, rejection.details);
    const message = rejection instanceof Error ? rejection.message : String(rejection);
    return new ApiFailure("transport", message, null, { cause: rejection });
  }
}

export function isApiFailure(value: unknown): value is ApiFailure {
  return value instanceof ApiFailure;
}

async function settle<K extends CommandName>(pending: () => Promise<CommandOutcome<K>>): Promise<CommandData<K>> {
  let outcome: CommandOutcome<K>;
  try {
    outcome = await pending();
  } catch (thrown) {
    throw ApiFailure.fromRejection(thrown);
  }
  if (outcome.status === "ok") return outcome.data;
  throw ApiFailure.fromRejection(outcome.error);
}

export interface ApiClient {
  readonly transport: TransportKind;
  call<K extends CallCommandName>(command: K, ...args: CommandArgs<K>): Promise<CommandData<K>>;
  /** See `Transport.stream`: events may outlive the promise; abort `signal` to detach. */
  stream<K extends StreamCommandName>(
    command: K,
    args: StreamArgs<K>,
    onEvent: (event: StreamEvent<K>) => void,
    options?: StreamOptions,
  ): Promise<CommandData<K>>;
  subscribe(listener: (event: AppEvent) => void): Promise<Unsubscribe>;
}

export function createApiClient(transport: Transport): ApiClient {
  return {
    transport: transport.kind,
    call: (command, ...args) => settle(() => transport.call(command, ...args)),
    stream: (command, args, onEvent, options) => settle(() => transport.stream(command, args, onEvent, options)),
    subscribe: async (listener) => {
      try {
        return await transport.subscribe(listener);
      } catch (thrown) {
        throw ApiFailure.fromRejection(thrown);
      }
    },
  };
}

export function isInsideShell(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/**
 * Tauri when the page runs inside the shell; the in-memory mock only in development builds, since
 * `import.meta.env.DEV` is a build-time constant that removes the mock from production bundles. A
 * production build outside the shell has no backend and fails typed.
 */
export async function selectTransport(insideShell: boolean = isInsideShell()): Promise<Transport> {
  if (insideShell) {
    const { createTauriTransport } = await import("./tauri-transport");
    return createTauriTransport();
  }
  if (import.meta.env.DEV) {
    const { createMockTransport } = await import("./mock-transport");
    return createMockTransport();
  }
  throw new ApiFailure("transport", "no backend: the page is not running inside the app shell", null);
}
