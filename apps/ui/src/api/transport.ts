import type { AppEvent, commands } from "./generated/bindings";

type Commands = typeof commands;

export type CommandName = keyof Commands;

type ChannelLike = { onmessage: (event: never) => void };

type ChannelEvent<P extends readonly unknown[]> = P extends readonly [infer Head, ...infer Rest]
  ? Head extends { onmessage: (event: infer Event) => void }
    ? Event
    : ChannelEvent<Rest>
  : never;

type WithoutChannel<P extends readonly unknown[]> = P extends readonly [infer Head, ...infer Rest]
  ? Head extends ChannelLike
    ? WithoutChannel<Rest>
    : [Head, ...WithoutChannel<Rest>]
  : [];

type Params<K extends CommandName> = Parameters<Commands[K]>;

/** Commands that take a channel anywhere in their parameters; callers never see the channel. */
export type StreamCommandName = {
  [K in CommandName]: [ChannelEvent<Params<K>>] extends [never] ? never : K;
}[CommandName];

export type CallCommandName = Exclude<CommandName, StreamCommandName>;

export type CommandArgs<K extends CallCommandName> = Params<K>;

export type StreamArgs<K extends StreamCommandName> = WithoutChannel<Params<K>>;

export type StreamEvent<K extends StreamCommandName> = ChannelEvent<Params<K>>;

/** True when every stream command takes its channel as the last parameter. */
export type StreamChannelsAreLast = {
  [K in StreamCommandName]: Params<K> extends readonly [...unknown[], ChannelLike] ? true : false;
}[StreamCommandName];

export type CommandResult<K extends CommandName> = Awaited<ReturnType<Commands[K]>>;

export type CommandData<K extends CommandName> = Extract<CommandResult<K>, { status: "ok" }>["data"];

/**
 * What a transport hands back. The error is whatever the backend or the IPC layer rejected with;
 * the client checks that it is an `ApiError` before trusting it.
 */
export type CommandOutcome<K extends CommandName> =
  | { status: "ok"; data: CommandData<K> }
  | { status: "error"; error: unknown };

export type Unsubscribe = () => void;

export type TransportKind = "tauri" | "mock";

export interface StreamOptions {
  /** Aborting detaches the event handler; no event reaches it afterwards. */
  signal?: AbortSignal;
}

/**
 * The only way the UI reaches the backend. A call resolves to the command's ok/error outcome and
 * rejects only when the transport itself fails.
 *
 * A stream's events are independent of its call promise: they may arrive before or after it
 * settles, and the terminal event (completed, failed, cancelled) lives in the stream, not in the
 * promise. The handler stays attached until the caller aborts `signal`, which it does on unmount or
 * after the terminal event. The Tauri transport then drops the channel's handler; a server transport
 * closes its subscription.
 */
export interface Transport {
  readonly kind: TransportKind;
  call<K extends CallCommandName>(command: K, ...args: CommandArgs<K>): Promise<CommandOutcome<K>>;
  stream<K extends StreamCommandName>(
    command: K,
    args: StreamArgs<K>,
    onEvent: (event: StreamEvent<K>) => void,
    options?: StreamOptions,
  ): Promise<CommandOutcome<K>>;
  subscribe(listener: (event: AppEvent) => void): Promise<Unsubscribe>;
}
