import { type ReactNode, useEffect, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { Check, ChevronDown, Image, LucideIcon, PenLine, Sparkles } from "lucide-react";

import { ModelSelectionBottomMenu } from "../../components/ModelSelectionBottomMenu";
import {
  resolveImageGenerationOptions,
  resolveSceneWriterOptions,
} from "../../../core/image-generation";
import { readSettings, saveAdvancedSettings } from "../../../core/storage/repo";
import type { Model } from "../../../core/storage/schemas";
import { useI18n } from "../../../core/i18n/context";
import { getProviderIcon } from "../../../core/utils/providerIcons";
import { cn, spacing, typography } from "../../design-tokens";
import { Switch } from "../../components/Switch";

interface ImageGenerationState {
  loading: boolean;
  error: string | null;
  models: Model[];
  writerModels: Model[];
  avatarEnabled: boolean;
  avatarModelId: string | null;
  sceneEnabled: boolean;
  sceneMode: "auto" | "askFirst" | "manual";
  sceneModelId: string | null;
  writerModelId: string | null;
}

type SelectorKey = "avatarModelId" | "sceneModelId" | "writerModelId";
type ToggleKey = "avatarEnabled" | "sceneEnabled";

type SceneMode = ImageGenerationState["sceneMode"];

type SelectorCardProps = {
  title: string;
  description: string;
  enabled?: boolean;
  selectedModel: Model | null;
  fallbackLabel: string;
  icon: LucideIcon;
  accentClassName: string;
  onToggle?: () => void;
  onClick: () => void;
};

type ModeOptionProps = {
  title: string;
  description: string;
  active: boolean;
  onClick: () => void;
};

function SettingsSection({
  title,
  icon,
  children,
}: {
  title: string;
  icon: ReactNode;
  children: ReactNode;
}) {
  return (
    <section className="space-y-3">
      <div className="flex items-center gap-2 px-1">
        <span className="text-fg/30">{icon}</span>
        <h2
          className={cn(
            typography.overline.size,
            typography.overline.weight,
            typography.overline.tracking,
            typography.overline.transform,
            "text-fg/40",
          )}
        >
          {title}
        </h2>
      </div>
      {children}
    </section>
  );
}

function SelectionRow({
  selectedModel,
  fallbackLabel,
  icon: Icon,
  onClick,
}: {
  selectedModel: Model | null;
  fallbackLabel: string;
  icon: LucideIcon;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "flex w-full items-center justify-between rounded-[10px] border border-fg/10",
        "bg-surface/40 px-3.5 py-3 text-left transition-colors duration-150",
        "hover:bg-surface-el/50 focus:border-fg/20 focus:outline-none",
      )}
    >
      <div className="flex min-w-0 items-center gap-3">
        <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-[9px] border border-fg/10 bg-fg/5">
          {selectedModel ? (
            getProviderIcon(selectedModel.providerId)
          ) : (
            <Icon className="h-4 w-4 text-fg/45" />
          )}
        </div>
        <div className="min-w-0">
          <div
            className={cn("truncate text-sm font-medium", selectedModel ? "text-fg" : "text-fg/55")}
          >
            {selectedModel?.displayName || selectedModel?.name || fallbackLabel}
          </div>
          {selectedModel ? (
            <div className="truncate text-xs text-fg/45">{selectedModel.name}</div>
          ) : (
            <div className="truncate text-xs text-fg/38">{fallbackLabel}</div>
          )}
        </div>
      </div>
      <ChevronDown className="h-4 w-4 shrink-0 text-fg/35" />
    </button>
  );
}

function ModeOption({ title, description, active, onClick }: ModeOptionProps) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "rounded-[10px] border px-4 py-3 text-left transition-colors duration-150",
        active
          ? "border-accent/40 bg-accent/10"
          : "border-fg/10 bg-surface/35 hover:bg-surface-el/45",
      )}
    >
      <div className="flex items-start justify-between gap-3">
        <div>
          <div className="text-sm font-semibold text-fg">{title}</div>
          <p className="mt-1 text-xs leading-5 text-fg/52">{description}</p>
        </div>
        <div
          className={cn(
            "mt-0.5 flex h-5 w-5 shrink-0 items-center justify-center rounded-full border transition-colors",
            active ? "border-accent/70 bg-accent/15 text-accent" : "border-fg/18 text-transparent",
          )}
        >
          <Check className="h-3.5 w-3.5" />
        </div>
      </div>
    </button>
  );
}

function SelectorCard({
  title,
  description,
  enabled,
  selectedModel,
  fallbackLabel,
  icon: Icon,
  accentClassName,
  onToggle,
  onClick,
}: SelectorCardProps) {
  const { t } = useI18n();
  const toggleId = `image-generation-toggle-${title.toLowerCase().replace(/\s+/g, "-")}`;
  const hasToggle = typeof enabled === "boolean" && typeof onToggle === "function";
  const isCardActive = enabled ?? true;
  const stateLabel = hasToggle
    ? enabled
      ? t("common.labels.on")
      : t("common.labels.off")
    : t("common.labels.on");

  return (
    <section className="rounded-[12px] border border-fg/10 bg-fg/5">
      <div className="border-b border-fg/8 px-4 py-4">
        <div className="flex items-start gap-3">
          <div className={cn("rounded-[9px] border p-1.5", accentClassName)}>
            <Icon className="h-4 w-4" />
          </div>
          <div className="min-w-0 flex-1">
            <div className="flex items-start justify-between gap-3">
              <div className="min-w-0 flex-1">
                <h3 className="text-sm font-semibold text-fg">{title}</h3>
                <p className="mt-1 text-sm leading-6 text-fg/48">{description}</p>
              </div>
              {hasToggle ? (
                <div className="flex items-center gap-2 pt-0.5">
                  <span className="text-[11px] font-medium text-fg/50">{stateLabel}</span>
                  <Switch
                    id={toggleId}
                    checked={enabled}
                    onChange={() => onToggle()}
                  />
                </div>
              ) : null}
            </div>
          </div>
        </div>
      </div>

      <div className="space-y-3 px-4 py-4">
        <div className="flex items-center justify-between text-[11px] font-medium uppercase tracking-[0.08em] text-fg/36">
          <span>{t("imageGeneration.labels.model")}</span>
          <span>{isCardActive ? t("common.labels.on") : t("common.labels.off")}</span>
        </div>
        {isCardActive ? (
          <SelectionRow
            selectedModel={selectedModel}
            fallbackLabel={fallbackLabel}
            icon={Icon}
            onClick={onClick}
          />
        ) : (
          <div className="rounded-[10px] border border-dashed border-fg/12 bg-surface/30 px-3.5 py-3 text-sm text-fg/45">
            Model selection stays saved, but this flow will not run until you turn it back on.
          </div>
        )}
      </div>
    </section>
  );
}

export function ImageGenerationPage() {
  const { t } = useI18n();
  const [state, setState] = useState<ImageGenerationState>({
    loading: true,
    error: null,
    models: [],
    writerModels: [],
    avatarEnabled: true,
    avatarModelId: null,
    sceneEnabled: true,
    sceneMode: "auto",
    sceneModelId: null,
    writerModelId: null,
  });
  const [showAvatarMenu, setShowAvatarMenu] = useState(false);
  const [showSceneMenu, setShowSceneMenu] = useState(false);
  const [showWriterMenu, setShowWriterMenu] = useState(false);

  useEffect(() => {
    const load = async () => {
      try {
        const settings = await readSettings();
        const options = resolveImageGenerationOptions(settings);
        const writerOptions = resolveSceneWriterOptions(settings);
        setState({
          loading: false,
          error: null,
          models: options.models,
          writerModels: writerOptions.models,
          avatarEnabled: settings.advancedSettings?.avatarGenerationEnabled ?? true,
          avatarModelId: settings.advancedSettings?.avatarGenerationModelId ?? null,
          sceneEnabled: settings.advancedSettings?.sceneGenerationEnabled ?? true,
          sceneMode:
            settings.advancedSettings?.sceneGenerationMode === "manual"
              ? "manual"
              : settings.advancedSettings?.sceneGenerationMode === "askFirst"
                ? "askFirst"
                : "auto",
          sceneModelId: settings.advancedSettings?.sceneGenerationModelId ?? null,
          writerModelId: settings.advancedSettings?.sceneWriterModelId ?? null,
        });
      } catch (err) {
        console.error("Failed to load image generation settings:", err);
        setState((prev) => ({
          ...prev,
          loading: false,
          error: err instanceof Error ? err.message : "Failed to load settings",
        }));
      }
    };

    void load();
  }, []);

  const persistSelection = async (key: SelectorKey, modelId: string | null) => {
    setState((prev) => ({
      ...prev,
      [key]: modelId,
      error: null,
    }));

    try {
      const settings = await readSettings();
      await saveAdvancedSettings({
        ...(settings.advancedSettings ?? {}),
        avatarGenerationEnabled: settings.advancedSettings?.avatarGenerationEnabled ?? true,
        avatarGenerationModelId:
          key === "avatarModelId"
            ? (modelId ?? undefined)
            : settings.advancedSettings?.avatarGenerationModelId,
        sceneGenerationEnabled: settings.advancedSettings?.sceneGenerationEnabled ?? true,
        sceneGenerationMode:
          settings.advancedSettings?.sceneGenerationMode === "manual"
            ? "manual"
            : settings.advancedSettings?.sceneGenerationMode === "askFirst"
              ? "askFirst"
              : "auto",
        sceneGenerationModelId:
          key === "sceneModelId"
            ? (modelId ?? undefined)
            : settings.advancedSettings?.sceneGenerationModelId,
        sceneWriterModelId:
          key === "writerModelId"
            ? (modelId ?? undefined)
            : settings.advancedSettings?.sceneWriterModelId,
      });
    } catch (err) {
      console.error("Failed to save image generation settings:", err);
      setState((prev) => ({
        ...prev,
        error: err instanceof Error ? err.message : "Failed to save image generation settings",
      }));
    }
  };

  const persistToggle = async (key: ToggleKey, enabled: boolean) => {
    setState((prev) => ({
      ...prev,
      [key]: enabled,
      error: null,
    }));

    if (!enabled) {
      if (key === "avatarEnabled") {
        setShowAvatarMenu(false);
      } else {
        setShowSceneMenu(false);
      }
    }

    try {
      const settings = await readSettings();
      await saveAdvancedSettings({
        ...(settings.advancedSettings ?? {}),
        avatarGenerationEnabled:
          key === "avatarEnabled"
            ? enabled
            : (settings.advancedSettings?.avatarGenerationEnabled ?? true),
        avatarGenerationModelId: settings.advancedSettings?.avatarGenerationModelId,
        sceneGenerationEnabled:
          key === "sceneEnabled"
            ? enabled
            : (settings.advancedSettings?.sceneGenerationEnabled ?? true),
        sceneGenerationMode:
          settings.advancedSettings?.sceneGenerationMode === "manual"
            ? "manual"
            : settings.advancedSettings?.sceneGenerationMode === "askFirst"
              ? "askFirst"
              : "auto",
        sceneGenerationModelId: settings.advancedSettings?.sceneGenerationModelId,
        sceneWriterModelId: settings.advancedSettings?.sceneWriterModelId,
      });
    } catch (err) {
      console.error("Failed to save image generation settings:", err);
      setState((prev) => ({
        ...prev,
        error: err instanceof Error ? err.message : "Failed to save image generation settings",
      }));
    }
  };

  const persistSceneMode = async (mode: SceneMode) => {
    setState((prev) => ({
      ...prev,
      sceneMode: mode,
      error: null,
    }));

    try {
      const settings = await readSettings();
      await saveAdvancedSettings({
        ...(settings.advancedSettings ?? {}),
        avatarGenerationEnabled: settings.advancedSettings?.avatarGenerationEnabled ?? true,
        avatarGenerationModelId: settings.advancedSettings?.avatarGenerationModelId,
        sceneGenerationEnabled: settings.advancedSettings?.sceneGenerationEnabled ?? true,
        sceneGenerationMode: mode,
        sceneGenerationModelId: settings.advancedSettings?.sceneGenerationModelId,
        sceneWriterModelId: settings.advancedSettings?.sceneWriterModelId,
      });
    } catch (err) {
      console.error("Failed to save scene generation mode:", err);
      setState((prev) => ({
        ...prev,
        error: err instanceof Error ? err.message : "Failed to save scene generation mode",
      }));
    }
  };

  const selectedAvatarModel = state.avatarModelId
    ? (state.models.find((model) => model.id === state.avatarModelId) ?? null)
    : null;
  const selectedSceneModel = state.sceneModelId
    ? (state.models.find((model) => model.id === state.sceneModelId) ?? null)
    : null;
  const selectedWriterModel = state.writerModelId
    ? (state.writerModels.find((model) => model.id === state.writerModelId) ?? null)
    : null;

  if (state.loading) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="h-8 w-8 animate-spin rounded-full border-2 border-fg/10 border-t-fg/60" />
      </div>
    );
  }

  if (state.models.length === 0 && state.writerModels.length === 0) {
    return (
      <div className="flex h-full flex-col items-center justify-center px-6">
        <div className="mb-4 flex h-16 w-16 items-center justify-center rounded-2xl border border-fg/10 bg-fg/5">
          <Image className="h-8 w-8 text-fg/40" />
        </div>
        <h2 className="mb-2 text-lg font-semibold text-fg">{t("imageGeneration.empty.title")}</h2>
        <p className="text-center text-sm text-fg/50">{t("imageGeneration.empty.description")}</p>
      </div>
    );
  }

  return (
    <div className="flex min-h-screen flex-col">
      <main className="flex-1 px-4 pb-24 pt-5">
        <div className={cn("mx-auto w-full max-w-5xl space-y-7", spacing.section)}>
          {state.error && (
            <motion.div
              initial={{ opacity: 0, y: -6 }}
              animate={{ opacity: 1, y: 0 }}
              className="rounded-[12px] border border-danger/20 bg-danger/5 p-3"
            >
              <p className="text-xs leading-relaxed text-danger/80">{state.error}</p>
            </motion.div>
          )}

          <div className="space-y-7">
            <motion.div
              initial={{ opacity: 0, y: 12 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ duration: 0.22, ease: "easeOut" }}
            >
              <SettingsSection title="Generation" icon={<Sparkles size={12} />}>
                <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
                  <SelectorCard
                    title={t("imageGeneration.sections.avatar.title")}
                    description={t("imageGeneration.sections.avatar.description")}
                    enabled={state.avatarEnabled}
                    selectedModel={selectedAvatarModel}
                    fallbackLabel={t("imageGeneration.labels.useFirstAvailable")}
                    icon={Image}
                    accentClassName="border-warning/30 bg-warning/10 text-warning/80"
                    onToggle={() => void persistToggle("avatarEnabled", !state.avatarEnabled)}
                    onClick={() => setShowAvatarMenu(true)}
                  />

                  <SelectorCard
                    title={t("imageGeneration.sections.scene.title")}
                    description={t("imageGeneration.sections.scene.description")}
                    enabled={state.sceneEnabled}
                    selectedModel={selectedSceneModel}
                    fallbackLabel={t("imageGeneration.labels.useFirstAvailable")}
                    icon={Sparkles}
                    accentClassName="border-accent/30 bg-accent/10 text-accent/80"
                    onToggle={() => void persistToggle("sceneEnabled", !state.sceneEnabled)}
                    onClick={() => setShowSceneMenu(true)}
                  />
                </div>
              </SettingsSection>
            </motion.div>

            <motion.div
              initial={{ opacity: 0, y: 12 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ duration: 0.22, delay: 0.06, ease: "easeOut" }}
            >
              <SettingsSection title="Prompting" icon={<PenLine size={12} />}>
                <div className="space-y-4">
                  <SelectorCard
                    title={t("imageGeneration.sections.writer.title")}
                    description={t("imageGeneration.sections.writer.description")}
                    selectedModel={selectedWriterModel}
                    fallbackLabel={t("imageGeneration.labels.useFirstCompatible")}
                    icon={PenLine}
                    accentClassName="border-info/30 bg-info/10 text-info/80"
                    onClick={() => setShowWriterMenu(true)}
                  />

                  <AnimatePresence initial={false}>
                    {state.sceneEnabled ? (
                      <motion.section
                        key="scene-mode"
                        initial={{ opacity: 0, y: 8 }}
                        animate={{ opacity: 1, y: 0 }}
                        exit={{ opacity: 0, y: -6 }}
                        transition={{ duration: 0.16, ease: "easeOut" }}
                        className="rounded-[12px] border border-fg/10 bg-fg/5"
                      >
                        <div className="border-b border-fg/8 px-4 py-4">
                          <div className="space-y-1">
                            <h3 className="text-sm font-semibold text-fg">
                              {t("imageGeneration.mode.title")}
                            </h3>
                            <p className="text-sm leading-6 text-fg/52">
                              {t("imageGeneration.mode.description")}
                            </p>
                          </div>
                        </div>
                        <div className="grid grid-cols-1 gap-3 px-4 py-4">
                          <ModeOption
                            title={t("imageGeneration.mode.auto")}
                            description={t("imageGeneration.mode.autoDescription")}
                            active={state.sceneMode === "auto"}
                            onClick={() => void persistSceneMode("auto")}
                          />
                          <ModeOption
                            title={t("imageGeneration.mode.askFirst")}
                            description={t("imageGeneration.mode.askFirstDescription")}
                            active={state.sceneMode === "askFirst"}
                            onClick={() => void persistSceneMode("askFirst")}
                          />
                          <ModeOption
                            title={t("imageGeneration.mode.manual")}
                            description={t("imageGeneration.mode.manualDescription")}
                            active={state.sceneMode === "manual"}
                            onClick={() => void persistSceneMode("manual")}
                          />
                        </div>
                      </motion.section>
                    ) : null}
                  </AnimatePresence>
                </div>
              </SettingsSection>
            </motion.div>
          </div>
        </div>
      </main>

      <ModelSelectionBottomMenu
        isOpen={showAvatarMenu}
        onClose={() => setShowAvatarMenu(false)}
        title={t("imageGeneration.labels.selectAvatarModel")}
        models={state.models}
        selectedModelIds={state.avatarModelId ? [state.avatarModelId] : []}
        searchPlaceholder={t("imageGeneration.labels.searchModels")}
        onSelectModel={(modelId) => {
          void persistSelection("avatarModelId", modelId);
          setShowAvatarMenu(false);
        }}
        clearOption={{
          label: t("imageGeneration.labels.useFirstAvailable"),
          icon: Image,
          selected: !state.avatarModelId,
          onClick: () => {
            void persistSelection("avatarModelId", null);
            setShowAvatarMenu(false);
          },
        }}
      />

      <ModelSelectionBottomMenu
        isOpen={showSceneMenu}
        onClose={() => setShowSceneMenu(false)}
        title={t("imageGeneration.labels.selectSceneModel")}
        models={state.models}
        selectedModelIds={state.sceneModelId ? [state.sceneModelId] : []}
        searchPlaceholder={t("imageGeneration.labels.searchModels")}
        onSelectModel={(modelId) => {
          void persistSelection("sceneModelId", modelId);
          setShowSceneMenu(false);
        }}
        clearOption={{
          label: t("imageGeneration.labels.useFirstAvailable"),
          icon: Image,
          selected: !state.sceneModelId,
          onClick: () => {
            void persistSelection("sceneModelId", null);
            setShowSceneMenu(false);
          },
        }}
      />

      <ModelSelectionBottomMenu
        isOpen={showWriterMenu}
        onClose={() => setShowWriterMenu(false)}
        title={t("imageGeneration.labels.selectWriterModel")}
        models={state.writerModels}
        selectedModelIds={state.writerModelId ? [state.writerModelId] : []}
        searchPlaceholder={t("imageGeneration.labels.searchModels")}
        onSelectModel={(modelId) => {
          void persistSelection("writerModelId", modelId);
          setShowWriterMenu(false);
        }}
        clearOption={{
          label: t("imageGeneration.labels.useFirstCompatible"),
          icon: PenLine,
          selected: !state.writerModelId,
          onClick: () => {
            void persistSelection("writerModelId", null);
            setShowWriterMenu(false);
          },
        }}
      />
    </div>
  );
}
