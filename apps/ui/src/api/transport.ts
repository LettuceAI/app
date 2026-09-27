import type { ApiError, AppEvent, commands } from "./generated/bindings";

type Commands = typeof commands;

export type CommandName = keyof Commands;

type StreamParams<P> = P extends [infer Request, { onmessage: (event: infer Event) => void }] ? [Request, Event] : never;

export type StreamCommandName = {
  [K in CommandName]: [StreamParams<Parameters<Commands[K]>>] extends [never] ? never : K;
}[CommandName];

export type CallCommandName = Exclude<CommandName, StreamCommandName>;

export type CommandArgs<K extends CallCommandName> = Parameters<Commands[K]>;

export type StreamRequest<K extends StreamCommandName> = StreamParams<Parameters<Commands[K]>>[0];

export type StreamEvent<K extends StreamCommandName> = StreamParams<Parameters<Commands[K]>>[1];

export type CommandResult<K extends CommandName> = Awaited<ReturnType<Commands[K]>>;

export type CommandData<K extends CommandName> = Extract<CommandResult<K>, { status: "ok" }>["data"];

export type CommandOutcome<K extends CommandName> =
  | { status: "ok"; data: CommandData<K> }
  | { status: "error"; error: ApiError };

export type Unsubscribe = () => void;

export type TransportKind = "tauri" | "mock";

/**
 * The only way the UI reaches the backend. A call resolves to the command's ok/error outcome and
 * rejects only when the transport itself fails.
 */
export interface Transport {
  readonly kind: TransportKind;
  call<K extends CallCommandName>(command: K, ...args: CommandArgs<K>): Promise<CommandOutcome<K>>;
  stream<K extends StreamCommandName>(
    command: K,
    request: StreamRequest<K>,
    onEvent: (event: StreamEvent<K>) => void,
  ): Promise<CommandOutcome<K>>;
  subscribe(listener: (event: AppEvent) => void): Promise<Unsubscribe>;
}
