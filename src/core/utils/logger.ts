import { isDevelopmentMode } from "./env";
import { invoke } from "@tauri-apps/api/core";

type LogLevel = "debug" | "info" | "warn" | "error" | "log";

export type LoggerOptions = {
  component: string; // e.g. useChatController, Chat.tsx, SettingsPage
  fn?: string; // optional function scope
};

export interface Logger {
  debug: (message?: any, ...optionalParams: any[]) => void;
  info: (message?: any, ...optionalParams: any[]) => void;
  warn: (message?: any, ...optionalParams: any[]) => void;
  error: (message?: any, ...optionalParams: any[]) => void;
  log: (message?: any, ...optionalParams: any[]) => void;
  with: (ctx: Partial<LoggerOptions>) => Logger;
}

let _enabled = isDevelopmentMode();
let _minLevel: LogLevel = isDevelopmentMode() ? "debug" : "info";

export function setLoggingEnabled(enabled: boolean) {
  _enabled = enabled;
}

export function isLoggingEnabled() {
  return _enabled;
}

export function setMinLogLevel(level: LogLevel) {
  _minLevel = level;
}

function fmtPrefix(level: LogLevel, opts: LoggerOptions): string {
  const ts = new Date();
  const hh = `${ts.getHours()}`.padStart(2, "0");
  const mm = `${ts.getMinutes()}`.padStart(2, "0");
  const ss = `${ts.getSeconds()}`.padStart(2, "0");
  return `[${hh}:${mm}:${ss}] ${level.toUpperCase()} ${opts.component}`;
}

function serializeError(err: Error): Record<string, unknown> {
  const out: Record<string, unknown> = {
    name: err.name,
    message: err.message,
  };
  if (err.stack) out.stack = err.stack;
  const cause = (err as any).cause;
  if (cause instanceof Error) {
    out.cause = serializeError(cause);
  } else if (cause !== undefined) {
    out.cause = String(cause);
  }
  return out;
}

function serializeArg(arg: any): string {
  const clamp = (value: string): string => value;

  if (arg instanceof Error) {
    return clamp(JSON.stringify(serializeError(arg)));
  }
  if (typeof arg === "string") return clamp(arg);
  if (typeof arg === "number" || typeof arg === "boolean" || arg == null) return clamp(String(arg));
  try {
    const seen = new WeakSet<object>();
    const serialized = JSON.stringify(arg, (_key, value) => {
      if (value instanceof Error) return serializeError(value);
      if (typeof value === "bigint") return value.toString();
      if (typeof value === "object" && value !== null) {
        if (seen.has(value)) return "[Circular]";
        seen.add(value);
      }
      return value;
    });
    return clamp(serialized);
  } catch {
    return clamp(String(arg));
  }
}

function write(level: LogLevel, opts: LoggerOptions, args: any[]) {
  if (!_enabled) return;
  const levelRank: Record<LogLevel, number> = {
    debug: 10,
    info: 20,
    warn: 30,
    error: 40,
    log: 20,
  };
  if (levelRank[level] < levelRank[_minLevel]) return;
  const method: (...data: any[]) => void = (console as any)[level] || console.log;
  const prefix = fmtPrefix(level, opts);
  if (opts.fn) {
    method(`${prefix} at=${opts.fn} |`, ...args);
  } else {
    method(`${prefix} |`, ...args);
  }

  // Also log to file
  try {
    const message = args.map((arg) => serializeArg(arg)).join(" | ");

    invoke("log_to_file", {
      timestamp: new Date().toISOString(),
      level: level.toUpperCase(),
      component: opts.component,
      function: opts.fn || null,
      message: message,
    }).catch((err) => {
      // Silently fail if logging to file doesn't work
      console.warn("Failed to log to file:", err);
    });
  } catch (err) {
    // Silently fail
  }
}

const disabledLogger: Logger = {
  debug: () => {},
  info: () => {},
  warn: () => {},
  error: () => {},
  log: () => {},
  with: () => disabledLogger,
};

export function logManager(options: LoggerOptions): Logger {
  const base: LoggerOptions = { component: options.component, fn: options.fn };

  const logger: Logger = {
    debug: (message?: any, ...rest: any[]) => write("debug", base, [message, ...rest]),
    info: (message?: any, ...rest: any[]) => write("info", base, [message, ...rest]),
    warn: (message?: any, ...rest: any[]) => write("warn", base, [message, ...rest]),
    error: (message?: any, ...rest: any[]) => write("error", base, [message, ...rest]),
    log: (message?: any, ...rest: any[]) => write("log", base, [message, ...rest]),
    with: (ctx: Partial<LoggerOptions>) =>
      logManager({ component: ctx.component ?? base.component, fn: ctx.fn ?? base.fn }),
  };

  return logger;
}

// Optional: allow runtime toggling via devtools
// @ts-ignore
if (typeof window !== "undefined") (window as any).__setDevLogs = setLoggingEnabled;
