import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { localeRegistry } from "../../src/core/i18n/locales/registry";
import { LOCALES, chunkTargets, doneFile, localeIsComplete, readJson } from "./common";

type Obj = Record<string, any>;

const IDENT = /^[A-Za-z_$][A-Za-z0-9_$]*$/;
const completeOnly = process.env.COMPLETE_ONLY === "1";

function isObj(value: unknown): value is Obj {
  return value != null && typeof value === "object" && !Array.isArray(value);
}

function setFlat(target: Obj, dot: string, value: string, log: string[]): void {
  const parts = dot.split(".");
  let current = target;
  for (let i = 0; i < parts.length - 1; i++) {
    const segment = parts[i];
    if (!(segment in current)) current[segment] = {};
    else if (!isObj(current[segment])) {
      log.push(`skip(non-obj) ${dot}`);
      return;
    }
    current = current[segment];
  }
  const leaf = parts[parts.length - 1];
  if (leaf in current) return;
  current[leaf] = value;
}

function serialize(value: unknown, indent: number): string {
  const pad = "  ".repeat(indent);
  const padIn = "  ".repeat(indent + 1);
  if (typeof value === "string") return JSON.stringify(value);
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  if (Array.isArray(value)) {
    return value.length
      ? `[\n${value.map((item) => `${padIn}${serialize(item, indent + 1)}`).join(",\n")},\n${pad}]`
      : "[]";
  }
  if (!isObj(value)) return "{}";
  const keys = Object.keys(value);
  if (!keys.length) return "{}";
  return `{\n${keys
    .map((key) => `${padIn}${IDENT.test(key) ? key : JSON.stringify(key)}: ${serialize(value[key], indent + 1)},`)
    .join("\n")}\n${pad}}`;
}

function varName(code: string): string {
  const [base, ...rest] = code.split("-");
  return base + rest.map((part) => part[0].toUpperCase() + part.slice(1)).join("");
}

const report: string[] = [];

for (const code of LOCALES) {
  const presentTargets = chunkTargets(code).filter((target) => existsSync(doneFile(target)));
  if (!presentTargets.length) {
    report.push(`${code}: NO done files, skipped`);
    continue;
  }
  if (completeOnly && !localeIsComplete(code)) {
    report.push(`${code}: incomplete, skipped`);
    continue;
  }

  const translations: Record<string, string> = {};
  for (const target of presentTargets) {
    Object.assign(translations, readJson<Record<string, string>>(doneFile(target), {}));
  }

  const registryEntry = (localeRegistry as any)[code];
  const merged: Obj = structuredClone(registryEntry.messages as Obj);
  const log: string[] = [];
  let added = 0;

  for (const [path, value] of Object.entries(translations)) {
    if (typeof value !== "string") continue;
    const before = JSON.stringify(merged);
    setFlat(merged, path, value, log);
    if (JSON.stringify(merged) !== before) added++;
  }

  const name = varName(code);
  const output =
    `import type { DeepPartialMessageTree } from "../types";\n` +
    `import { type LocaleMessages } from "./en";\n` +
    `import type { LocaleMetadata } from ".";\n\n` +
    `export const ${name}Metadata: LocaleMetadata = ${serialize(registryEntry.metadata, 0)};\n\n` +
    `export const ${name}Messages: DeepPartialMessageTree<LocaleMessages> = ${serialize(merged, 0)};\n`;

  writeFileSync(new URL(`../../src/core/i18n/locales/${code}.ts`, import.meta.url), output);
  report.push(`${code}: +${added} keys (${Object.keys(translations).length} translated, ${presentTargets.length}/2 halves)`);
}

for (const line of report) console.log(line);

