import { Channel } from "@tauri-apps/api/core";
import { commands, events } from "./generated/bindings";
import type {
  CallCommandName,
  CommandArgs,
  CommandOutcome,
  StreamArgs,
  StreamChannelsAreLast,
  StreamCommandName,
  StreamEvent,
  StreamOptions,
  Transport,
} from "./transport";

type AnyCall = (...args: unknown[]) => Promise<unknown>;

type Assert<T extends true> = T;

/** The channel is appended after the arguments; this fails to compile if a command puts it elsewhere. */
export type ChannelGoesLast = Assert<StreamChannelsAreLast>;

const detached = () => {};

export function createTauriTransport(): Transport {
  return {
    kind: "tauri",
    call<K extends CallCommandName>(command: K, ...args: CommandArgs<K>) {
      const invoke = commands[command] as AnyCall;
      return invoke(...args) as Promise<CommandOutcome<K>>;
    },
    stream<K extends StreamCommandName>(
      command: K,
      args: StreamArgs<K>,
      onEvent: (event: StreamEvent<K>) => void,
      options: StreamOptions = {},
    ) {
      const { signal } = options;
      const channel = new Channel<StreamEvent<K>>(signal?.aborted ? detached : onEvent);
      signal?.addEventListener("abort", () => {
        channel.onmessage = detached;
      }, { once: true });
      const invoke = commands[command] as AnyCall;
      return invoke(...args, channel) as Promise<CommandOutcome<K>>;
    },
    async subscribe(listener) {
      return events.appEvent.listen((event) => listener(event.payload));
    },
  };
}
