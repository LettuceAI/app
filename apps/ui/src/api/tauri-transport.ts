import { Channel } from "@tauri-apps/api/core";
import { commands, events } from "./generated/bindings";
import {
  abortedBeforeSend,
  type CallCommandName,
  type CommandArgs,
  type CommandOutcome,
  type StreamArgs,
  type StreamChannelsAreLast,
  type StreamCommandName,
  type StreamEvent,
  type StreamOptions,
  type Transport,
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
      if (signal?.aborted) return Promise.resolve(abortedBeforeSend(command));
      const channel = new Channel<StreamEvent<K>>(onEvent);
      const detach = () => {
        channel.onmessage = detached;
      };
      signal?.addEventListener("abort", detach, { once: true });
      const release = () => {
        detach();
        signal?.removeEventListener("abort", detach);
      };
      const invoke = commands[command] as AnyCall;
      return (invoke(...args, channel) as Promise<CommandOutcome<K>>).then(
        (outcome) => {
          if (outcome.status === "error") release();
          return outcome;
        },
        (error: unknown) => {
          release();
          throw error;
        },
      );
    },
    async subscribe(listener) {
      return events.appEvent.listen((event) => listener(event.payload));
    },
  };
}
