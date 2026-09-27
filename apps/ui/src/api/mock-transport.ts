import type { ApiError, AppEvent, AppStatus } from "./generated/bindings";
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
} from "./transport";

type Awaitable<T> = T | Promise<T>;

export type MockOutcome<K extends CommandName> =
  | { status: "ok"; data: CommandData<K> }
  | { status: "error"; error: ApiError };

export type MockCallHandlers = {
  [K in CallCommandName]?: (...args: CommandArgs<K>) => Awaitable<MockOutcome<K>>;
};

/** `emit` keeps delivering after the handler settles, until the caller aborts its signal. */
export type MockStreamHandlers = {
  [K in StreamCommandName]?: (
    args: StreamArgs<K>,
    emit: (event: StreamEvent<K>) => void,
  ) => Awaitable<MockOutcome<K>>;
};

export interface MockTransportOptions {
  calls?: MockCallHandlers;
  streams?: MockStreamHandlers;
}

export interface MockTransport extends Transport {
  emitAppEvent(event: AppEvent): void;
}

export const mockAppStatus: AppStatus = {
  version: "0.0.0-mock",
  build_variant: "normal",
  platform: "other",
  ui_state: {},
  legacy_database_detected: false,
  unresolved_sync_conflicts: 0,
  purge_notices: 0,
};

const defaultCalls: MockCallHandlers = {
  appStatus: () => ({ status: "ok", data: mockAppStatus }),
};

function unhandled<K extends CommandName>(command: K): CommandOutcome<K> {
  return {
    status: "error",
    error: { code: "unsupported", message: `mock transport has no handler for ${command}`, details: null },
  };
}

type AnyCallHandler = (...args: unknown[]) => Awaitable<unknown>;
type AnyStreamHandler = (args: unknown, emit: (event: unknown) => void) => Awaitable<unknown>;

/** An in-memory transport for tests and for running the UI in a plain browser. */
export function createMockTransport(options: MockTransportOptions = {}): MockTransport {
  const calls: MockCallHandlers = { ...defaultCalls, ...options.calls };
  const streams: MockStreamHandlers = { ...options.streams };
  const listeners = new Set<(event: AppEvent) => void>();

  return {
    kind: "mock",
    async call<K extends CallCommandName>(command: K, ...args: CommandArgs<K>) {
      const handler = calls[command] as AnyCallHandler | undefined;
      if (!handler) return unhandled(command);
      return (await handler(...args)) as CommandOutcome<K>;
    },
    async stream<K extends StreamCommandName>(
      command: K,
      args: StreamArgs<K>,
      onEvent: (event: StreamEvent<K>) => void,
      streamOptions: StreamOptions = {},
    ) {
      const handler = streams[command] as AnyStreamHandler | undefined;
      if (!handler) return unhandled(command);
      const { signal } = streamOptions;
      const emit = (event: unknown) => {
        if (!signal?.aborted) onEvent(event as StreamEvent<K>);
      };
      return (await handler(args, emit)) as CommandOutcome<K>;
    },
    async subscribe(listener) {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    emitAppEvent(event) {
      for (const listener of listeners) listener(event);
    },
  };
}
