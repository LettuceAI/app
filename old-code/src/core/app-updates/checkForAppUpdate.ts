import { invoke } from "@tauri-apps/api/core";

import { DOWNLOADS_PAGE_LINK } from "../utils/links";
import type { AppPlatform } from "../utils/platform";

const RELEASES_API_URL = "https://api.github.com/repos/LettuceAI/app/releases?per_page=40";

export type AppUpdateInfo = {
  currentVersion: string;
  latestVersion: string;
  releaseUrl: string;
  downloadUrl: string;
  releaseTag: string;
  channel: "dev" | "release";
};

type GitHubAsset = {
  name?: string;
};

type GitHubRelease = {
  tag_name?: string;
  html_url?: string;
  draft?: boolean;
  prerelease?: boolean;
  assets?: GitHubAsset[];
};

type AppChannel = "dev" | "release";

type NumericVersionInfo = {
  kind: "release";
  version: string;
  parts: number[];
};

type DevVersionInfo = {
  kind: "dev";
  version: string;
  runNumber: number;
  buildAttempt: number;
};

type ParsedVersionInfo = NumericVersionInfo | DevVersionInfo;

type CandidateRelease = {
  tag: string;
  url: string;
  version: ParsedVersionInfo;
};

function isDevBuildVersion(currentVersion: string): boolean {
  const normalized = currentVersion.trim().toLowerCase();
  return (
    normalized.includes("-dev.") ||
    normalized.includes("-dev-") ||
    /^0\.1\.\d+(?:[+-].*)?$/.test(normalized)
  );
}

export function detectUpdateChannel(currentVersion: string): AppChannel {
  return isDevBuildVersion(currentVersion) ? "dev" : "release";
}

function getTagPrefix(platform: AppPlatform, channel: AppChannel): string | null {
  if (platform.os === "android") {
    return channel === "dev" ? "android-dev-" : "android-release-";
  }
  if (platform.type === "desktop") {
    return channel === "dev" ? "desktop-dev-" : "desktop-release-";
  }
  return null;
}

function parseReleaseVersion(input: string): NumericVersionInfo | null {
  const match = input.match(/\d+(?:\.\d+){2,3}/);
  if (!match) return null;

  const parts = match[0].split(".").map((segment) => Number.parseInt(segment, 10));
  if (!parts.every((part) => Number.isFinite(part))) return null;

  return {
    kind: "release",
    version: match[0],
    parts,
  };
}

function parseCurrentDevVersion(input: string): DevVersionInfo | null {
  const normalized = input.trim();
  const runMatch = normalized.match(/^0\.1\.(\d+)/i);
  if (!runMatch) return null;

  const runNumber = Number.parseInt(runMatch[1], 10);
  if (!Number.isFinite(runNumber)) return null;

  const attemptMatch = normalized.match(/-dev(?:\.|-)(\d+)/i);
  const buildAttempt = attemptMatch ? Number.parseInt(attemptMatch[1], 10) : 1;

  return {
    kind: "dev",
    version: normalized,
    runNumber,
    buildAttempt: Number.isFinite(buildAttempt) ? buildAttempt : 1,
  };
}

function parseReleaseTagVersion(
  tagName: string,
  prefix: string,
  channel: AppChannel,
): ParsedVersionInfo | null {
  if (!tagName.startsWith(prefix)) return null;

  const payload = tagName.slice(prefix.length).trim();
  if (!payload) return null;

  if (channel === "dev") {
    const match = payload.match(/^(\d+)-(\d+)-[a-z0-9]+$/i);
    if (!match) return null;

    const runNumber = Number.parseInt(match[1], 10);
    const buildAttempt = Number.parseInt(match[2], 10);
    if (!Number.isFinite(runNumber) || !Number.isFinite(buildAttempt)) return null;

    return {
      kind: "dev",
      version: payload,
      runNumber,
      buildAttempt,
    };
  }

  const parsed = parseReleaseVersion(payload);
  if (!parsed) return null;
  if (parsed.version !== payload) return null;
  return parsed;
}

function compareVersions(left: ParsedVersionInfo, right: ParsedVersionInfo): number {
  if (left.kind !== right.kind) return 0;

  if (left.kind === "dev" && right.kind === "dev") {
    if (left.runNumber !== right.runNumber) return left.runNumber > right.runNumber ? 1 : -1;
    if (left.buildAttempt !== right.buildAttempt) {
      return left.buildAttempt > right.buildAttempt ? 1 : -1;
    }
    return 0;
  }

  if (left.kind === "release" && right.kind === "release") {
    const size = Math.max(left.parts.length, right.parts.length);
    for (let index = 0; index < size; index += 1) {
      const leftPart = left.parts[index] ?? 0;
      const rightPart = right.parts[index] ?? 0;
      if (leftPart > rightPart) return 1;
      if (leftPart < rightPart) return -1;
    }
  }

  return 0;
}

function hasUsableAsset(release: GitHubRelease, platform: AppPlatform): boolean {
  const assetNames = (release.assets ?? [])
    .map((asset) => asset.name)
    .filter((name): name is string => typeof name === "string");

  if (platform.os === "android") {
    return assetNames.some((name) => name.toLowerCase().endsWith(".apk"));
  }

  if (platform.os === "windows") {
    return assetNames.some((name) => /^windows-.*\.zip$/i.test(name));
  }

  if (platform.os === "linux") {
    return assetNames.some((name) => /^linux-.*\.zip$/i.test(name));
  }

  if (platform.os === "macos") {
    return assetNames.some((name) => /^macos-.*\.(dmg|zip)$/i.test(name));
  }

  return false;
}

function buildDownloadUrl(
  platform: AppPlatform,
  channel: AppChannel,
  version: string,
  releaseTag: string,
  releaseUrl: string,
): string {
  const url = new URL(DOWNLOADS_PAGE_LINK);
  url.searchParams.set("platform", platform.os);
  url.searchParams.set("channel", channel);
  url.searchParams.set("version", version);
  url.searchParams.set("release", releaseTag);
  url.searchParams.set("source", "in-app-update");
  url.searchParams.set("fallback", releaseUrl);
  return url.toString();
}

async function fetchMatchingRelease(
  platform: AppPlatform,
  channel: AppChannel,
  prefix: string,
): Promise<CandidateRelease | null> {
  const controller = new AbortController();
  const timeoutId = window.setTimeout(() => controller.abort(), 5000);

  try {
    const response = await fetch(RELEASES_API_URL, {
      headers: {
        Accept: "application/vnd.github+json",
      },
      signal: controller.signal,
    });
    if (!response.ok) return null;

    const releases = (await response.json()) as GitHubRelease[];
    let best: CandidateRelease | null = null;

    for (const release of releases) {
      if (release.draft) continue;
      if (channel === "dev" ? !release.prerelease : release.prerelease) continue;
      if (typeof release.tag_name !== "string" || typeof release.html_url !== "string") continue;
      if (!hasUsableAsset(release, platform)) continue;

      const version = parseReleaseTagVersion(release.tag_name, prefix, channel);
      if (!version) continue;

      if (!best || compareVersions(version, best.version) > 0) {
        best = {
          tag: release.tag_name,
          url: release.html_url,
          version,
        };
      }
    }

    return best;
  } finally {
    window.clearTimeout(timeoutId);
  }
}

export async function checkForAppUpdate(platform: AppPlatform): Promise<AppUpdateInfo | null> {
  if (platform.os === "ios") return null;

  const currentVersion = await invoke<string>("get_app_version");
  const channel = detectUpdateChannel(currentVersion);
  const prefix = getTagPrefix(platform, channel);
  if (!prefix) return null;

  const currentParsed =
    channel === "dev"
      ? parseCurrentDevVersion(currentVersion)
      : parseReleaseVersion(currentVersion);
  if (!currentParsed) return null;

  const latest = await fetchMatchingRelease(platform, channel, prefix);
  if (!latest) return null;
  if (compareVersions(latest.version, currentParsed) <= 0) return null;

  return {
    currentVersion,
    latestVersion: latest.version.version,
    releaseUrl: latest.url,
    downloadUrl: buildDownloadUrl(
      platform,
      channel,
      latest.version.version,
      latest.tag,
      latest.url,
    ),
    releaseTag: latest.tag,
    channel,
  };
}
