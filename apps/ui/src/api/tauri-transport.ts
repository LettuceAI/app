import { Channel } from "@tauri-apps/api/core";
import { commands, events } from "./generated/bindings";
import type {
  CallCommandName,
  CommandArgs,
  CommandOutcome,
  StreamCommandName,
  StreamEvent,
  StreamRequest,
  Transport,
} from "./transport";

type AnyCall = (...args: unknown[]) => Promise<unknown>;

export function createTauriTransport(): Transport {
  return {
    kind: "tauri",
    call<K extends CallCommandName>(command: K, ...args: CommandArgs<K>) {
      const invoke = commands[command] as AnyCall;
      return invoke(...args) as Promise<CommandOutcome<K>>;
    },
    stream<K extends StreamCommandName>(
      command: K,
      request: StreamRequest<K>,
      onEvent: (event: StreamEvent<K>) => void,
    ) {
      const channel = new Channel<StreamEvent<K>>(onEvent);
      const invoke = commands[command] as AnyCall;
      return invoke(request, channel) as Promise<CommandOutcome<K>>;
    },
    async subscribe(listener) {
      return events.appEvent.listen((event) => listener(event.payload));
    },
  };
}
