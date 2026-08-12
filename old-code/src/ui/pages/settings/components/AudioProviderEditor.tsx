import { useEffect, useMemo, useState } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";

import {
  kokoroDefaultAssetRoot,
  kokoroSupportedVariants,
  verifyAudioProvider,
  type AudioProvider,
  type AudioProviderType,
  type KokoroModelVariant,
  type KokoroSupportedVariant,
} from "../../../../core/storage/audioProviders";
import { BottomMenu } from "../../../components/BottomMenu";
import { useI18n } from "../../../../core/i18n/context";

interface AudioProviderEditorProps {
  isOpen: boolean;
  provider: AudioProvider | null;
  onClose: () => void;
  onSave: (provider: AudioProvider) => Promise<void>;
}

export function AudioProviderEditor({
  isOpen,
  provider,
  onClose,
  onSave,
}: AudioProviderEditorProps) {
  const { t } = useI18n();
  const [formData, setFormData] = useState<AudioProvider | null>(null);
  const [isSaving, setIsSaving] = useState(false);
  const [validationError, setValidationError] = useState<string | null>(null);
  const [kokoroVariants, setKokoroVariants] = useState<KokoroSupportedVariant[]>([]);
  const [managedRoot, setManagedRoot] = useState<string>("");

  useEffect(() => {
    if (provider) {
      setFormData({ ...provider });
      setValidationError(null);
    }
  }, [provider]);

  useEffect(() => {
    if (formData?.providerType !== "kokoro") return;
    void (async () => {
      try {
        const [variants, defaultRoot] = await Promise.all([
          kokoroSupportedVariants(),
          kokoroDefaultAssetRoot(),
        ]);
        setKokoroVariants(variants);
        setManagedRoot(defaultRoot);
        setFormData((current) => {
          if (!current || current.providerType !== "kokoro") return current;
          const next = { ...current };
          if (!next.kokoroVariant && variants.length > 0) {
            next.kokoroVariant = variants[0].id;
          }
          if (!next.assetRoot) {
            next.assetRoot = defaultRoot;
          }
          return next;
        });
      } catch (e) {
        console.error("Failed to load kokoro metadata:", e);
      }
    })();
  }, [formData?.providerType]);

  const isKokoro = formData?.providerType === "kokoro";
  const isOpenAiCompatible = formData?.providerType === "openai_tts";
  const isFishSpeechLocal = formData?.providerType === "fish_speech";

  const browseAssetRoot = async () => {
    try {
      const selected = await openDialog({ multiple: false, directory: true });
      if (typeof selected === "string" && formData) {
        setFormData({ ...formData, assetRoot: selected });
      }
    } catch (e) {
      console.error("Browse failed:", e);
    }
  };

  const variantOptions = useMemo(() => kokoroVariants, [kokoroVariants]);

  const handleSave = async () => {
    if (!formData || !formData.label.trim()) return;

    if (formData.providerType === "kokoro") {
      if (!formData.kokoroVariant) {
        setValidationError(t("providers.audioEditor.errors.chooseModelVariant"));
        return;
      }
      if (!formData.assetRoot?.trim()) {
        setValidationError(t("providers.audioEditor.errors.assetRootRequired"));
        return;
      }
      setIsSaving(true);
      setValidationError(null);
      try {
        await onSave(formData);
      } catch (e) {
        setValidationError(e instanceof Error ? e.message : t("providers.audioEditor.errors.saveFailed"));
      } finally {
        setIsSaving(false);
      }
      return;
    }

    if (formData.providerType !== "fish_speech" && !formData.apiKey?.trim()) {
      setValidationError(t("providers.audioEditor.errors.apiKeyRequired"));
      return;
    }

    if (formData.providerType === "gemini_tts" && !formData.projectId?.trim()) {
      setValidationError(t("providers.audioEditor.errors.projectIdRequired"));
      return;
    }

    if (
      (formData.providerType === "openai_tts" || formData.providerType === "fish_speech") &&
      !formData.baseUrl?.trim()
    ) {
      setValidationError(t("providers.audioEditor.errors.baseUrlRequired"));
      return;
    }

    setIsSaving(true);
    setValidationError(null);

    try {
      const isValid = await verifyAudioProvider(
        formData.providerType,
        formData.apiKey,
        formData.projectId,
        formData.baseUrl,
      );

      if (!isValid) {
        setValidationError(t("providers.audioEditor.errors.invalidCredentials"));
        return;
      }

      await onSave(formData);
    } catch (e) {
      setValidationError(
        e instanceof Error ? e.message : t("providers.audioEditor.errors.verificationFailed"),
      );
    } finally {
      setIsSaving(false);
    }
  };

  if (!formData) return null;

  return (
    <BottomMenu
      isOpen={isOpen}
      onClose={onClose}
      title={
        formData.id
          ? t("providers.audioEditor.titleEdit")
          : t("providers.audioEditor.titleCreate")
      }
    >
      <div className="space-y-4 pb-2">
        <div>
          <label className="mb-1 block text-[11px] font-medium text-fg/70">
            {t("providers.audioEditor.fields.providerType")}
          </label>
          <select
            value={formData.providerType}
            onChange={(e) => {
              const next = e.target.value as AudioProviderType;
              setFormData({
                ...formData,
                providerType: next,
                projectId: undefined,
                location: undefined,
                baseUrl: undefined,
                requestPath:
                  next === "openai_tts"
                    ? "/v1/audio/speech"
                    : next === "fish_speech"
                      ? "/v1/tts"
                      : undefined,
                apiKey: next === "kokoro" ? undefined : formData.apiKey,
                kokoroVariant: next === "kokoro" ? formData.kokoroVariant : undefined,
                assetRoot: next === "kokoro" ? formData.assetRoot : undefined,
              });
            }}
            disabled={isKokoro}
            className="w-full rounded-lg border border-fg/10 bg-surface-el/20 px-3 py-2 text-sm text-fg focus:border-fg/30 focus:outline-none disabled:opacity-70"
          >
            <option value="elevenlabs" className="bg-surface-el">
              ElevenLabs
            </option>
            <option value="fish_tts" className="bg-surface-el">
              {t("providers.audioEditor.types.fish")}
            </option>
            <option value="fish_speech" className="bg-surface-el">
              {t("providers.audioEditor.types.fishSpeech")}
            </option>
            <option value="gemini_tts" className="bg-surface-el">
              {t("providers.audioEditor.types.gemini")}
            </option>
            <option value="openai_tts" className="bg-surface-el">
              {t("providers.audioEditor.types.openai")}
            </option>
            {isKokoro && (
              <option value="kokoro" className="bg-surface-el">
                {t("providers.audioEditor.types.kokoro")}
              </option>
            )}
          </select>
        </div>

        <div>
          <label className="mb-1 block text-[11px] font-medium text-fg/70">
            {t("providers.audioEditor.fields.label")}
          </label>
          <input
            type="text"
            value={formData.label}
            onChange={(e) => setFormData({ ...formData, label: e.target.value })}
            placeholder={
              formData.providerType === "gemini_tts"
                ? t("providers.audioEditor.placeholders.labelGemini")
                : formData.providerType === "fish_tts"
                  ? t("providers.audioEditor.placeholders.labelFish")
                : formData.providerType === "fish_speech"
                  ? t("providers.audioEditor.placeholders.labelFishSpeech")
                : formData.providerType === "openai_tts"
                  ? t("providers.audioEditor.placeholders.labelOpenai")
                  : formData.providerType === "kokoro"
                    ? t("providers.audioEditor.placeholders.labelKokoro")
                    : t("providers.audioEditor.placeholders.labelElevenlabs")
            }
            className="w-full rounded-lg border border-fg/10 bg-surface-el/20 px-3 py-2 text-sm text-fg placeholder-fg/40 focus:border-fg/30 focus:outline-none"
          />
        </div>

        {!isKokoro && (
          <div>
            <label className="mb-1 block text-[11px] font-medium text-fg/70">
              {isFishSpeechLocal
                ? t("providers.audioEditor.fields.apiKeyOptional")
                : t("providers.audioEditor.fields.apiKey")}
            </label>
            <input
              type="password"
              value={formData.apiKey ?? ""}
              onChange={(e) => {
                setFormData({ ...formData, apiKey: e.target.value });
                if (validationError) setValidationError(null);
              }}
              placeholder={t("providers.audioEditor.placeholders.apiKey")}
              className="w-full rounded-lg border border-fg/10 bg-surface-el/20 px-3 py-2 text-sm text-fg placeholder-fg/40 focus:border-fg/30 focus:outline-none"
            />
          </div>
        )}

        {isKokoro && (
          <>
            <div>
              <label className="mb-1 block text-[11px] font-medium text-fg/70">
                {t("providers.audioEditor.fields.modelVariant")}
              </label>
              <select
                value={formData.kokoroVariant ?? ""}
                onChange={(e) => {
                  setFormData({
                    ...formData,
                    kokoroVariant: (e.target.value || undefined) as KokoroModelVariant | undefined,
                  });
                  if (validationError) setValidationError(null);
                }}
                className="w-full rounded-lg border border-fg/10 bg-surface-el/20 px-3 py-2 text-sm text-fg focus:border-fg/30 focus:outline-none"
                disabled={variantOptions.length === 0}
              >
                {variantOptions.length === 0 ? (
                  <option value="" className="bg-surface-el">
                    {t("providers.audioEditor.loadingVariants")}
                  </option>
                ) : (
                  variantOptions.map((variant) => (
                    <option key={variant.id} value={variant.id} className="bg-surface-el">
                      {variant.label} ({variant.sizeMb} MB)
                    </option>
                  ))
                )}
              </select>
              <p className="mt-1 text-[10px] text-fg/40">
                {t("providers.audioEditor.kokoroVariantHint")}
              </p>
            </div>
            <div>
              <label className="mb-1 block text-[11px] font-medium text-fg/70">
                {t("providers.audioEditor.fields.assetRoot")}
              </label>
              <div className="flex gap-2">
                <input
                  type="text"
                  value={formData.assetRoot ?? ""}
                  onChange={(e) => {
                    setFormData({ ...formData, assetRoot: e.target.value });
                    if (validationError) setValidationError(null);
                  }}
                  placeholder={managedRoot || t("providers.audioEditor.placeholders.assetRoot")}
                  className="min-w-0 flex-1 rounded-lg border border-fg/10 bg-surface-el/20 px-3 py-2 text-sm text-fg placeholder-fg/40 focus:border-fg/30 focus:outline-none"
                />
                <button
                  type="button"
                  onClick={() => void browseAssetRoot()}
                  className="rounded-lg border border-fg/10 bg-fg/5 px-3 py-2 text-xs text-fg/70 transition hover:border-fg/20 hover:bg-fg/10"
                >
                  {t("common.buttons.browseFiles")}
                </button>
                {managedRoot && (
                  <button
                    type="button"
                    onClick={() => setFormData({ ...formData, assetRoot: managedRoot })}
                    className="rounded-lg border border-info/25 bg-info/10 px-3 py-2 text-xs text-info/85 transition hover:border-info/40 hover:bg-info/15"
                  >
                    {t("providers.audioEditor.managed")}
                  </button>
                )}
              </div>
              {managedRoot && (
                <p className="mt-1 break-all text-[10px] text-fg/40">
                  {t("providers.audioEditor.managedPath", { path: managedRoot })}
                </p>
              )}
            </div>
          </>
        )}

        {formData.providerType === "gemini_tts" && (
          <>
            <div>
              <label className="mb-1 block text-[11px] font-medium text-fg/70">
                {t("providers.audioEditor.fields.projectId")}
              </label>
              <input
                type="text"
                value={formData.projectId ?? ""}
                onChange={(e) => {
                  setFormData({ ...formData, projectId: e.target.value });
                  if (validationError) setValidationError(null);
                }}
                placeholder={t("providers.audioEditor.placeholders.projectId")}
                className="w-full rounded-lg border border-fg/10 bg-surface-el/20 px-3 py-2 text-sm text-fg placeholder-fg/40 focus:border-fg/30 focus:outline-none"
              />
            </div>
            <div>
              <label className="mb-1 block text-[11px] font-medium text-fg/70">
                {t("providers.audioEditor.fields.region")}
              </label>
              <input
                type="text"
                value={formData.location ?? ""}
                onChange={(e) => setFormData({ ...formData, location: e.target.value })}
                placeholder={t("providers.audioEditor.placeholders.region")}
                className="w-full rounded-lg border border-fg/10 bg-surface-el/20 px-3 py-2 text-sm text-fg placeholder-fg/40 focus:border-fg/30 focus:outline-none"
              />
            </div>
          </>
        )}

        {(isOpenAiCompatible || isFishSpeechLocal) && (
          <>
            <div>
              <label className="mb-1 block text-[11px] font-medium text-fg/70">
                {t("providers.audioEditor.fields.baseUrl")}
              </label>
              <input
                type="text"
                value={formData.baseUrl ?? ""}
                onChange={(e) => {
                  setFormData({ ...formData, baseUrl: e.target.value });
                  if (validationError) setValidationError(null);
                }}
                placeholder={t("providers.audioEditor.placeholders.baseUrl")}
                className="w-full rounded-lg border border-fg/10 bg-surface-el/20 px-3 py-2 text-sm text-fg placeholder-fg/40 focus:border-fg/30 focus:outline-none"
              />
            </div>
            <div>
              <label className="mb-1 block text-[11px] font-medium text-fg/70">
                {t("providers.audioEditor.fields.requestPath")}
              </label>
              <input
                type="text"
                value={formData.requestPath ?? (isFishSpeechLocal ? "/v1/tts" : "/v1/audio/speech")}
                onChange={(e) => setFormData({ ...formData, requestPath: e.target.value })}
                placeholder={isFishSpeechLocal ? "/v1/tts" : "/v1/audio/speech"}
                className="w-full rounded-lg border border-fg/10 bg-surface-el/20 px-3 py-2 text-sm text-fg placeholder-fg/40 focus:border-fg/30 focus:outline-none"
              />
              <p className="mt-1 text-[10px] text-fg/40">
                {t("providers.audioEditor.requestPathHint")}
              </p>
            </div>
          </>
        )}

        {validationError && <p className="text-xs font-medium text-danger/80">{validationError}</p>}

        <div className="flex gap-3 pt-1">
          <button
            onClick={onClose}
            className="flex-1 rounded-lg border border-fg/10 bg-fg/5 px-4 py-2 text-sm font-medium text-fg/70 transition hover:border-fg/20 hover:bg-fg/10 hover:text-fg"
          >
            {t("common.buttons.cancel")}
          </button>
          <button
            onClick={() => void handleSave()}
            disabled={isSaving || !formData.label.trim()}
            className="flex-1 rounded-lg border border-accent/40 bg-accent/20 px-4 py-2 text-sm font-semibold text-accent/90 transition hover:border-accent/60 hover:bg-accent/30 disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {isSaving ? t("providers.audioEditor.verifying") : t("common.buttons.save")}
          </button>
        </div>
      </div>
    </BottomMenu>
  );
}
