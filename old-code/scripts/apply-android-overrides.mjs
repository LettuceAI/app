import { mkdir, readFile, readdir, rm, writeFile } from "node:fs/promises";
import path from "node:path";

const repoRoot = process.cwd();
const androidAppDir = path.join(repoRoot, "src-tauri", "gen", "android", "app");
const androidOverridesDir = path.join(repoRoot, "src-tauri", "android-overrides");
const tauriConfigPath = path.join(repoRoot, "src-tauri", "tauri.conf.json");
const javaRootDir = path.join(androidAppDir, "src", "main", "java");
const manifestPath = path.join(androidAppDir, "src", "main", "AndroidManifest.xml");
const buildGradlePath = path.join(androidAppDir, "build.gradle.kts");

async function main() {
  const packageName = await readBasePackageName(tauriConfigPath);
  const packagePath = packageName.split(".").join(path.sep);
  const targetJavaDir = path.join(javaRootDir, packagePath);

  await mkdir(targetJavaDir, { recursive: true });
  await deleteStaleClassFiles(javaRootDir, targetJavaDir, [
    "MainActivity.kt",
    "CrashMonitorService.kt",
    "KokoroPhonemizerBridge.kt",
  ]);

  await applyTemplate("MainActivity.kt.template", path.join(targetJavaDir, "MainActivity.kt"), {
    __PACKAGE__: packageName,
  });
  await applyTemplate(
    "CrashMonitorService.kt.template",
    path.join(targetJavaDir, "CrashMonitorService.kt"),
    {
      __PACKAGE__: packageName,
    },
  );
  await applyTemplate(
    "KokoroPhonemizerBridge.kt.template",
    path.join(targetJavaDir, "KokoroPhonemizerBridge.kt"),
    {
      __PACKAGE__: packageName,
    },
  );
  await applyTemplate("AndroidManifest.xml", manifestPath, {
    __PACKAGE__: packageName,
  });
  await applyTemplate("app.build.gradle.kts.template", buildGradlePath, {
    __PACKAGE__: packageName,
  });

  console.log(
    `Applied Android overrides for package ${packageName} (applicationId override via LETTUCE_ANDROID_APPLICATION_ID)`,
  );
}

async function readBasePackageName(filePath) {
  const content = await readFile(filePath, "utf8");
  const config = JSON.parse(content);
  const packageName = config?.identifier;

  if (typeof packageName !== "string" || packageName.length === 0) {
    throw new Error(`Failed to resolve Android package name from ${filePath}`);
  }

  return packageName;
}

async function applyTemplate(templateName, targetPath, replacements) {
  const templatePath = path.join(androidOverridesDir, templateName);
  let content = await readFile(templatePath, "utf8");
  for (const [needle, value] of Object.entries(replacements)) {
    content = content.replaceAll(needle, value);
  }
  await writeFile(targetPath, content);
}

async function deleteStaleClassFiles(javaDir, targetJavaDir, fileNames) {
  const entries = await walk(javaDir);
  const targetFiles = new Set(fileNames.map((name) => path.join(targetJavaDir, name)));

  await Promise.all(
    entries
      .filter((entry) => fileNames.includes(path.basename(entry)))
      .filter((entry) => !targetFiles.has(entry))
      .map((entry) => rm(entry, { force: true })),
  );
}

async function walk(dir) {
  const entries = await readdir(dir, { withFileTypes: true });
  const paths = await Promise.all(
    entries.map(async (entry) => {
      const fullPath = path.join(dir, entry.name);
      if (entry.isDirectory()) {
        return walk(fullPath);
      }
      return [fullPath];
    }),
  );
  return paths.flat();
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
