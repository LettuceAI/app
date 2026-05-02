import { motion } from "framer-motion";
import { Sparkles, ArrowRight, X } from "lucide-react";
import { useNavigate } from "react-router-dom";
import { useI18n } from "../../core/i18n/context";

type LegacyEmbeddingVersion = "v1" | "v2" | "v3";

interface EmbeddingUpgradePromptProps {
  onDismiss: () => void;
  returnTo?: string;
  currentVersion: LegacyEmbeddingVersion;
}

const DISMISS_KEY_PREFIX = "lettuce:embeddingUpgrade:dismissed:";
const TARGET_VERSION = "v4";

/** localStorage key for "user clicked Later on the upgrade-to-X prompt". */
function dismissKey(targetVersion: string): string {
  return `${DISMISS_KEY_PREFIX}${targetVersion}`;
}

/**
 * Has the user already dismissed the upgrade prompt for `targetVersion`?
 * The key is namespaced by target so a future v5 prompt re-prompts users
 * who deferred the v4 one.
 */
export function isEmbeddingUpgradeDismissed(targetVersion: string = TARGET_VERSION): boolean {
  if (typeof window === "undefined") return false;
  try {
    return window.localStorage.getItem(dismissKey(targetVersion)) === "1";
  } catch {
    return false;
  }
}

/** Persist the "Later" preference so we don't pester the user. */
export function markEmbeddingUpgradeDismissed(targetVersion: string = TARGET_VERSION): void {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(dismissKey(targetVersion), "1");
  } catch {
    // localStorage can throw in private mode; ignore.
  }
}

/**
 * Debug-only override: when this localStorage key is `"1"`, callers should
 * render the prompt regardless of installed version or persisted dismissal.
 * Toggle from the browser console:
 *
 *   localStorage.setItem('lettuce:debug:forceUpgradePrompt', '1');
 *   localStorage.removeItem('lettuce:embeddingUpgrade:dismissed:v4');
 *   location.reload();
 *
 * Disable again with:
 *   localStorage.removeItem('lettuce:debug:forceUpgradePrompt');
 */
const FORCE_KEY = "lettuce:debug:forceUpgradePrompt";

export function isEmbeddingUpgradePromptForced(): boolean {
  if (typeof window === "undefined") return false;
  try {
    return window.localStorage.getItem(FORCE_KEY) === "1";
  } catch {
    return false;
  }
}

/**
 * A prompt shown to users with an older embedding model installed,
 * encouraging them to upgrade to v4. Caller is responsible for not
 * rendering the component when `isEmbeddingUpgradeDismissed()` returns
 * true; dismissal here writes that flag.
 */
export function EmbeddingUpgradePrompt({
  onDismiss,
  returnTo,
  currentVersion,
}: EmbeddingUpgradePromptProps) {
  const { t } = useI18n();
  const navigate = useNavigate();

  const handleUpgrade = () => {
    const params = new URLSearchParams();
    params.set("upgrade", "true");
    if (returnTo) {
      params.set("returnTo", returnTo);
    }
    params.set("version", TARGET_VERSION);
    navigate(`/settings/embedding-download?${params.toString()}`);
  };

  const handleDismiss = () => {
    markEmbeddingUpgradeDismissed(TARGET_VERSION);
    onDismiss();
  };

  const title = t("components.embeddingUpgrade.title");
  const body = (() => {
    switch (currentVersion) {
      case "v1":
        return t("components.embeddingUpgrade.v1Message");
      case "v2":
        return t("components.embeddingUpgrade.v2Message");
      case "v3": {
        const translated = t("components.embeddingUpgrade.v3Message");
        // Fall back to a sensible English string if the locale doesn't
        // have a v3-specific entry yet.
        return translated === "components.embeddingUpgrade.v3Message"
          ? "lettuce-emb-v4 is out and dramatically improves roleplay memory recall. Upgrading is recommended."
          : translated;
      }
    }
  })();

  return (
    <motion.div
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, y: 20 }}
      className="rounded-2xl border border-amber-500/30 bg-black/95 backdrop-blur-xl p-4 shadow-2xl shadow-amber-500/10"
    >
      <div className="flex items-start gap-3">
        <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-full bg-amber-500/20">
          <Sparkles className="h-5 w-5 text-amber-400" />
        </div>
        <div className="flex-1 min-w-0">
          <div className="flex items-start justify-between gap-2">
            <div>
              <h3 className="font-semibold text-amber-200 text-base">{title}</h3>
              <p className="mt-1.5 text-sm text-amber-200/70 leading-relaxed">{body}</p>
            </div>
            <button
              onClick={handleDismiss}
              className="shrink-0 p-1.5 text-amber-400/50 hover:text-amber-400 transition-colors rounded-lg hover:bg-amber-400/10"
              aria-label="Dismiss"
            >
              <X className="h-4 w-4" />
            </button>
          </div>
          <div className="mt-4 flex gap-3">
            <button
              onClick={handleUpgrade}
              className="flex items-center gap-1.5 rounded-xl bg-amber-500 px-4 py-2 text-sm font-semibold text-black transition-all hover:bg-amber-400 shadow-lg shadow-amber-500/20"
            >
              {t("components.embeddingUpgrade.button")}
              <ArrowRight className="h-4 w-4" />
            </button>
            <button
              onClick={handleDismiss}
              className="rounded-xl px-4 py-2 text-sm text-amber-200/60 hover:text-amber-200 transition-colors hover:bg-white/5"
            >
              {t("common.buttons.later")}
            </button>
          </div>
        </div>
      </div>
    </motion.div>
  );
}
