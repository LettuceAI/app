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

function findings(fixture: string): string[] {
  return lint(fixture).map((diagnostic) =>
    diagnostic.code === "eslint(no-restricted-imports)" ? `imports: ${diagnostic.help ?? ""}` : diagnostic.code,
  );
}

const tauriOnlyInApi = "imports: Only src/api/ may talk to Tauri; use the API client.";
const noFeaturesBelow = "imports: shared/ and entities/ may not import features.";
const featureIndexOnly = "imports: Import another feature only through its public index: @/features/<name>.";
const typesOnlyFromBindings =
  "imports: Only types may come from the generated bindings outside src/api; call the backend through @/api/client.";
const transportsInApi = "imports: Transports are chosen by @/api/client; nothing outside src/api imports one.";

describe("lint boundaries", () => {
  it("rejects a @tauri-apps import outside src/api", () => {
    expect(findings("src/shared/tauri-import.ts")).toEqual([tauriOnlyInApi]);
  });

  it("rejects shared/ importing a feature", () => {
    expect(findings("src/shared/feature-import.ts")).toEqual([noFeaturesBelow]);
  });

  it("rejects entities/ importing a feature", () => {
    expect(findings("src/entities/feature-import.ts")).toEqual([noFeaturesBelow]);
  });

  it("lets a shared test use the mock transport but still not a feature", () => {
    expect(findings("src/shared/helper.test.ts")).toEqual([noFeaturesBelow]);
  });

  it("rejects a feature reaching into another feature's internals", () => {
    expect(findings("src/features/alpha/deep-import.ts")).toEqual([featureIndexOnly]);
  });

  it("rejects value imports of the generated bindings outside src/api", () => {
    expect(findings("src/features/alpha/generated-value-import.ts")).toEqual([typesOnlyFromBindings]);
  });

  it("allows type imports of the generated contract types", () => {
    expect(findings("src/features/alpha/generated-type-import.ts")).toEqual([]);
  });

  it("rejects importing a transport outside src/api", () => {
    expect(findings("src/features/alpha/transport-import.ts")).toEqual([transportsInApi, transportsInApi]);
  });

  it("rejects touching __TAURI_INTERNALS__ outside src/api", () => {
    expect(findings("src/features/alpha/shell-detection.ts").sort()).toEqual([
      "eslint(no-restricted-globals)",
      "eslint(no-restricted-properties)",
    ]);
  });

  it("exempts src/api from the Tauri, transport and runtime rules", () => {
    expect(findings("src/api/allowed.ts")).toEqual([]);
  });

  it("rejects literal text and literal text props in JSX, but not class names, data attributes or a space expression", () => {
    expect(findings("src/features/alpha/Literal.tsx")).toEqual([
      "react(jsx-no-literals)",
      "react(jsx-no-literals)",
      "react(jsx-no-literals)",
    ]);
  });
});
