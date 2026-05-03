import { useMemo, useState } from "react";
import { AlertCircle, ArrowLeft, Loader2, Save } from "lucide-react";
import { useSearchParams } from "react-router-dom";
import {
  WindowControlButtons,
  hasCustomWindowControls,
  useDragRegionProps,
} from "../../components/App/TopNav";
import { saveCharacter } from "../../../core/storage/repo";
import type { CompanionConfig } from "../../../core/storage/schemas";
import {
  generateCompanionSoulDraft,
  mergeCompanionSoulDraft,
} from "../../../core/companion/soul";
import { CompanionSoulEditor } from "../characters/components/CompanionSoulEditor";
import { normalizeCompanionConfig } from "../characters/utils/companionDefaults";
import { cn, components, radius } from "../../design-tokens";
import { Routes, useNavigationManager } from "../../navigation";
import { useI18n } from "../../../core/i18n/context";
import { useChatLayoutContext } from "./ChatLayout";
import { isCompanionChat } from "./companionUi";

function PageHeader({
  title,
  subtitle,
  onBack,
  right,
}: {
  title: string;
  subtitle?: string;
  onBack: () => void;
  right?: React.ReactNode;
}) {
  const dragRegionProps = useDragRegionProps();
  const { t } = useI18n();

  return (
    <header
      className={cn(
        "z-20 shrink-0 border-b border-fg/8 pl-3 lg:pl-8",
        hasCustomWindowControls ? "pr-0" : "pr-3 lg:pr-8",
        "bg-surface/95 backdrop-blur-xl",
      )}
      style={{
        paddingTop: "calc(env(safe-area-inset-top) + 12px)",
        paddingBottom: "12px",
      }}
      {...dragRegionProps}
    >
      <div className="flex h-10 items-center justify-between" {...dragRegionProps}>
        <div className="flex min-w-0 items-center gap-2.5">
          <button
            onClick={onBack}
            className="-ml-2 flex shrink-0 items-center justify-center px-[0.6em] py-[0.3em] text-fg/80 transition hover:text-fg"
            aria-label={t("chats.companionSoul.back")}
          >
            <ArrowLeft size={18} strokeWidth={2.5} />
          </button>
          <div className="min-w-0">
            <p className="truncate text-[15px] font-semibold text-fg">{title}</p>
            {subtitle ? <p className="truncate text-[11px] text-fg/45">{subtitle}</p> : null}
          </div>
        </div>
        <div className="flex items-center gap-2">
          {right}
          <WindowControlButtons />
        </div>
      </div>
    </header>
  );
}

function openingContextFromCharacter(companion: CompanionConfig, sceneText: string): string {
  const soul = companion.soul;
  return [
    sceneText,
    soul.essence ? `Current essence: ${soul.essence}` : "",
    soul.relationalStyle ? `Current relational style: ${soul.relationalStyle}` : "",
  ]
    .filter(Boolean)
    .join("\n\n");
}

export function CompanionSoulPage() {
  const [searchParams] = useSearchParams();
  const sessionId = searchParams.get("sessionId");
  const { backOrReplace } = useNavigationManager();
  const { t } = useI18n();
  const { character, characterLoading, chatController, reloadCharacter } = useChatLayoutContext();
  const [draft, setDraft] = useState<CompanionConfig | null>(null);
  const [saving, setSaving] = useState(false);
  const [generating, setGenerating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const companion = useMemo(
    () => normalizeCompanionConfig(draft ?? character?.companion),
    [character?.companion, draft],
  );
  const selectedSceneText = useMemo(() => {
    const sceneId = chatController.session?.selectedSceneId ?? character?.defaultSceneId;
    const scene = character?.scenes?.find((item) => item.id === sceneId);
    return [scene?.content, scene?.direction].filter(Boolean).join("\n");
  }, [character?.defaultSceneId, character?.scenes, chatController.session?.selectedSceneId]);

  const handleBack = () => {
    if (!character) {
      backOrReplace(Routes.chat);
      return;
    }
    backOrReplace(Routes.chatSettingsSession(character.id, sessionId));
  };

  const handleGenerate = async () => {
    if (!character) return;
    setGenerating(true);
    setError(null);
    try {
      const generated = await generateCompanionSoulDraft({
        characterName: character.name,
        characterDefinition: character.definition ?? character.description ?? "",
        characterDescription: character.description ?? "",
        openingContext: openingContextFromCharacter(companion, selectedSceneText),
        currentSoul: companion,
        modelId: character.defaultModelId,
      });
      setDraft(mergeCompanionSoulDraft(companion, generated));
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setGenerating(false);
    }
  };

  const handleSave = async () => {
    if (!character) return;
    setSaving(true);
    setError(null);
    try {
      await saveCharacter({
        ...character,
        mode: "companion",
        companion,
      });
      setDraft(null);
      reloadCharacter();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  if (characterLoading) {
    return (
      <div className="flex min-h-screen items-center justify-center bg-base text-fg">
        <div className="flex items-center gap-3 text-sm text-fg/60">
          <Loader2 className="h-4 w-4 animate-spin" />
          {t("chats.companionSoul.loading")}
        </div>
      </div>
    );
  }

  if (!character || !isCompanionChat(character, chatController.session)) {
    return (
      <div className="flex min-h-screen items-center justify-center bg-base px-6">
        <div className={cn("w-full max-w-lg border border-fg/10 bg-surface p-5", radius.lg)}>
          <p className="text-base font-semibold text-fg">{t("chats.companionSoul.unavailable")}</p>
          <p className="mt-2 text-sm text-fg/60">
            {t("chats.companionSoul.unavailableDesc")}
          </p>
          <button
            onClick={handleBack}
            className={cn("mt-4 px-4 py-2 text-sm text-fg", components.button.primary)}
          >
            {t("chats.companionSoul.back")}
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="relative z-10 flex min-h-screen flex-col bg-base text-fg">
      <PageHeader
        title={t("chats.companionSoul.pageTitle")}
        subtitle={character.name}
        onBack={handleBack}
        right={
          <button
            type="button"
            onClick={handleSave}
            disabled={saving || generating}
            className={cn(
              "inline-flex items-center gap-2 rounded-lg border border-accent/25 bg-accent/10 px-3 py-2 text-xs font-medium text-accent transition hover:border-accent/40 hover:bg-accent/15 disabled:opacity-50",
            )}
          >
            {saving ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Save className="h-3.5 w-3.5" />}
            {t("chats.companionSoul.save")}
          </button>
        }
      />

      <main className="mx-auto flex w-full max-w-5xl flex-1 flex-col gap-4 overflow-y-auto px-4 py-4 pb-20 lg:px-8">
        <CompanionSoulEditor
          companion={companion}
          onChange={setDraft}
          onGenerate={handleGenerate}
          generating={generating}
          disabled={saving}
        />

        {error ? (
          <div className="flex items-start gap-2 rounded-lg border border-danger/25 bg-danger/10 px-3 py-2 text-sm text-danger">
            <AlertCircle className="mt-0.5 h-4 w-4 shrink-0" />
            <span>{error}</span>
          </div>
        ) : null}
      </main>
    </div>
  );
}
