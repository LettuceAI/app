import { readdirSync, readFileSync } from "node:fs";
import { dirname, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const src = fileURLToPath(new URL("../src", import.meta.url));

function sourceFiles(dir: string): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) return sourceFiles(path);
    return /\.(ts|tsx)$/.test(entry.name) ? [path] : [];
  });
}

const specifierPattern = /(?:\bfrom\s+|\bimport\s*\(\s*|\bimport\s+)["']([^"']+)["']/g;

function relativeImports(file: string): string[] {
  const text = readFileSync(file, "utf8");
  return [...text.matchAll(specifierPattern)]
    .map((match) => match[1] ?? "")
    .filter((specifier) => specifier.startsWith("."))
    .map((specifier) => resolve(dirname(file), specifier));
}

function segments(path: string): string[] {
  return relative(src, path).split(sep);
}

/**
 * The lint rules cover `@/` imports; this covers relative paths, which a pattern cannot judge
 * without knowing where the importing file sits.
 */
describe("relative imports respect layer boundaries", () => {
  const files = sourceFiles(src).filter((file) => !file.includes(`${sep}api${sep}generated${sep}`));
  const edges = files.flatMap((file) => relativeImports(file).map((target) => ({ file, target })));

  it("never cross from one feature into another", () => {
    const crossings = edges.filter(({ file, target }) => {
      const [fromLayer, fromFeature] = segments(file);
      const [toLayer, toFeature] = segments(target);
      return fromLayer === "features" && toLayer === "features" && fromFeature !== toFeature;
    });
    expect(crossings.map(({ file, target }) => `${relative(src, file)} -> ${relative(src, target)}`)).toEqual([]);
  });

  it("never reach features from shared/ or entities/", () => {
    const leaks = edges.filter(({ file, target }) => {
      const [fromLayer] = segments(file);
      const [toLayer] = segments(target);
      return (fromLayer === "shared" || fromLayer === "entities") && toLayer === "features";
    });
    expect(leaks.map(({ file, target }) => `${relative(src, file)} -> ${relative(src, target)}`)).toEqual([]);
  });

  it("keep the generated bindings and transports behind src/api", () => {
    const leaks = edges.filter(({ file, target }) => {
      const [fromLayer] = segments(file);
      const [toLayer, toModule] = segments(target);
      return fromLayer !== "api" && toLayer === "api" && (toModule === "generated" || /^(tauri|mock)-transport/.test(toModule ?? ""));
    });
    expect(leaks.map(({ file, target }) => `${relative(src, file)} -> ${relative(src, target)}`)).toEqual([]);
  });

  it("scans the source tree", () => {
    expect(edges.length).toBeGreaterThan(0);
  });
});
