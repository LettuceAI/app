type DebugPayload = unknown;

export type ChatDebugEnvelope = {
  state: string;
  payload?: DebugPayload;
  level?: string;
  message?: string;
};

export type ChatDebugEventRecord = {
  state: string;
  payload?: DebugPayload;
  level?: string;
  message?: string;
  timestamp: number;
};

export type ChatRequestDebugTrace = {
  requestId: string;
  sessionId?: string;
  messageId?: string;
  operation?: string;
  events: ChatDebugEventRecord[];
};

const MAX_TRACE_COUNT = 120;

const tracesByRequestId = new Map<string, ChatRequestDebugTrace>();
const requestIdByMessageKey = new Map<string, string>();
const subscribers = new Set<() => void>();

function notify() {
  subscribers.forEach((subscriber) => subscriber());
}

function getMessageKey(sessionId: string, messageId: string) {
  return `${sessionId}:${messageId}`;
}

function pruneOldestTrace() {
  while (tracesByRequestId.size > MAX_TRACE_COUNT) {
    const oldestKey = tracesByRequestId.keys().next().value;
    if (!oldestKey) return;
    const trace = tracesByRequestId.get(oldestKey);
    tracesByRequestId.delete(oldestKey);
    if (trace?.sessionId && trace.messageId) {
      requestIdByMessageKey.delete(getMessageKey(trace.sessionId, trace.messageId));
    }
  }
}

function ensureTrace(requestId: string): ChatRequestDebugTrace {
  let trace = tracesByRequestId.get(requestId);
  if (!trace) {
    trace = {
      requestId,
      events: [],
    };
    tracesByRequestId.set(requestId, trace);
    pruneOldestTrace();
  }
  return trace;
}

function getString(value: unknown): string | undefined {
  return typeof value === "string" && value.length > 0 ? value : undefined;
}

function getPayloadObject(payload?: DebugPayload): Record<string, unknown> | null {
  if (!payload || typeof payload !== "object") return null;
  return payload as Record<string, unknown>;
}

function getPayloadRequestId(payload?: DebugPayload): string | undefined {
  return getString(getPayloadObject(payload)?.requestId);
}

export function recordChatDebugEvent(envelope: ChatDebugEnvelope) {
  const payload = envelope.payload;
  const payloadObject = getPayloadObject(payload);
  const sessionId = getString(payloadObject?.sessionId);
  const messageId = getString(payloadObject?.messageId);
  const requestId = getPayloadRequestId(payload);

  if (!requestId) {
    return;
  }

  const trace = ensureTrace(requestId);
  if (sessionId) {
    trace.sessionId = sessionId;
  }
  if (messageId) {
    trace.messageId = messageId;
    if (trace.sessionId) {
      requestIdByMessageKey.set(getMessageKey(trace.sessionId, messageId), requestId);
    }
  }
  const operation = getString(payloadObject?.operation);
  if (operation) {
    trace.operation = operation;
  }

  trace.events.push({
    state: envelope.state,
    payload,
    level: envelope.level,
    message: envelope.message,
    timestamp: Date.now(),
  });

  notify();
}

export function getMessageDebugTrace(
  sessionId: string,
  messageId: string,
): ChatRequestDebugTrace | null {
  const requestId = requestIdByMessageKey.get(getMessageKey(sessionId, messageId));
  if (requestId) {
    return tracesByRequestId.get(requestId) ?? null;
  }

  const traces = Array.from(tracesByRequestId.values()).filter(
    (trace) => trace.sessionId === sessionId,
  );
  if (traces.length === 0) return null;
  const exact = traces.find((trace) => trace.messageId === messageId);
  if (exact) return exact;
  return traces[traces.length - 1] ?? null;
}

export function subscribeChatDebugStore(listener: () => void) {
  subscribers.add(listener);
  return () => {
    subscribers.delete(listener);
  };
}
