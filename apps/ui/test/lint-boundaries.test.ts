import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const root = fileURLToPath(new URL("..", import.meta.url));
const oxlint = fileURLToPath(new URL("../node_modules/.bin/oxlint", import.meta.url));

interface Diagnostic {
  code: string;
  filename: string;
  help?: string;
}

function lint(fixture: string): Diagnostic[] {
  let output: string;
  try {
    output = execFileSync(oxlint, ["-f", "json", `test/lint-fixtures/${fixture}`], { cwd: root, encoding: "utf8" });
  } catch (error) {
    output = (error as { stdout: string }).stdout;
  }
  return (JSON.parse(output) as { diagnostics: Diagnostic[] }).diagnostics;
}

function restrictedImports(fixture: string): string[] {
  return lint(fixture)
    .filter((diagnostic) => diagnostic.code === "eslint(no-restricted-imports)")
    .map((diagnostic) => diagnostic.help ?? "");
}

describe("import boundaries", () => {
  it("rejects a @tauri-apps import outside src/api", () => {
    expect(restrictedImports("src/shared/tauri-import.ts")).toEqual([
      "Only src/api/ may talk to Tauri; use the API client.",
    ]);
  });

  it("rejects shared/ importing a feature", () => {
    expect(restrictedImports("src/shared/feature-import.ts")).toEqual([
      "shared/ and entities/ may not import features.",
    ]);
  });

  it("rejects a feature reaching into another feature's internals", () => {
    expect(restrictedImports("src/features/alpha/deep-import.ts")).toEqual([
      "Import another feature only through its public index: @/features/<name>.",
    ]);
  });

  it("rejects value imports of the generated bindings outside src/api", () => {
    expect(restrictedImports("src/features/alpha/generated-value-import.ts")).toHaveLength(1);
  });

  it("allows type imports of the generated contract types", () => {
    expect(restrictedImports("src/features/alpha/generated-type-import.ts")).toEqual([]);
  });
});
