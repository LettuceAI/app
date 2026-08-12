import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { localeRegistry } from "../../src/core/i18n/locales/registry";

export type Obj = Record<string, unknown>;
export type FlatMessages = Record<string, string>;

export const LOCALES = Object.keys(localeRegistry).filter((code) => code !== "en");
export const WORK_DIR = new URL(
  `${process.env.I18N_WORKDIR ?? ".i18n-translate"}/`,
  new URL("../../", import.meta.url),
);

export function ensureWorkDir(): void {
  mkdirSync(WORK_DIR, { recursive: true });
}

export function flattenMessages(
  value: unknown,
  prefix = "",
  out: FlatMessages = {},
): FlatMessages {
  if (!value || typeof value !== "object" || Array.isArray(value)) return out;
  for (const [key, child] of Object.entries(value as Obj)) {
    const path = prefix ? `${prefix}.${key}` : key;
    if (child && typeof child === "object" && !Array.isArray(child)) {
      flattenMessages(child, path, out);
    } else if (typeof child === "string") {
      out[path] = child;
    }
  }
  return out;
}

export function readJson<T>(url: URL, fallback: T): T {
  if (!existsSync(url)) return fallback;
  const text = readFileSync(url, "utf8").trim();
  if (!text) return fallback;
  return JSON.parse(text) as T;
}

export function writeJson(url: URL, value: unknown): void {
  ensureWorkDir();
  writeFileSync(url, JSON.stringify(value, null, 1) + "\n");
}

export function workFile(name: string): URL {
  return new URL(name, WORK_DIR);
}

export function missingFile(target: string): URL {
  return workFile(`missing-${target}.json`);
}

export function doneFile(target: string): URL {
  return workFile(`done-${target}.json`);
}

export function suspectFile(target: string): URL {
  return workFile(`suspect-${target}.json`);
}

export function parseTarget(target: string): { code: string; part?: "1" | "2" } {
  const match = target.match(/^(.+?)(?:-([12]))?$/);
  if (!match) throw new Error(`Invalid target: ${target}`);
  return { code: match[1], part: match[2] as "1" | "2" | undefined };
}

export function placeholders(value: string): string[] {
  return Array.from(value.matchAll(/\{\{[^}]+\}\}/g), (match) => match[0]).sort();
}

export function samePlaceholders(source: string, translated: string): boolean {
  return JSON.stringify(placeholders(source)) === JSON.stringify(placeholders(translated));
}

const ALLOWED_IDENTICAL = new Set([
  "API",
  "BYOK",
  "B",
  "KB",
  "MB",
  "GB",
  "TB",
  "Mistral",
  "OpenAI",
  "Anthropic",
  "Cerebras",
  "LettuceAI",
  "Google",
  "AI",
  "Studio",
  "Discord",
  "JSON",
  "URL",
  "HTTP",
  "HTTPS",
]);

const TOKEN_RE = /[A-Za-z][A-Za-z0-9']*/g;

export function isSuspiciousUnchanged(source: string, translated: string): boolean {
  if (source !== translated) return false;
  const tokens = source.match(TOKEN_RE) ?? [];
  return !tokens.length || !tokens.every((token) => ALLOWED_IDENTICAL.has(token));
}

export function englishFlatMessages(): FlatMessages {
  return flattenMessages((localeRegistry as any).en.messages);
}

export function localeFlatMessages(code: string): FlatMessages {
  return flattenMessages((localeRegistry as any)[code].messages);
}

export function chunkTargets(code: string): string[] {
  return [`${code}-1`, `${code}-2`];
}

export function chunkIsComplete(target: string): boolean {
  const missing = readJson<FlatMessages>(missingFile(target), {});
  const done = readJson<FlatMessages>(doneFile(target), {});
  return Object.keys(missing).length > 0 && Object.keys(done).length === Object.keys(missing).length;
}

export function localeIsComplete(code: string): boolean {
  return chunkTargets(code).every(chunkIsComplete);
}

