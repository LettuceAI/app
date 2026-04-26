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
        setValidationError("Choose a model variant");
        return;
      }
      if (!formData.assetRoot?.trim()) {
        setValidationError("Asset root is required");
        return;
      }
      setIsSaving(true);
      setValidationError(null);
      try {
        await onSave(formData);
      } catch (e) {
        setValidationError(e instanceof Error ? e.message : "Save failed");
      } finally {
        setIsSaving(false);
      }
      return;
    }

    if (!formData.apiKey?.trim()) {
      setValidationError("API key is required");
      return;
    }

    if (formData.providerType === "gemini_tts" && !formData.projectId?.trim()) {
      setValidationError("Project ID is required for Gemini TTS");
      return;
    }

    if (formData.providerType === "openai_tts" && !formData.baseUrl?.trim()) {
      setValidationError("Base URL is required for OpenAI-compatible TTS");
      return;
    }

    setIsSaving(true);
    setValidationError(null);

    try {
      const isValid = await verifyAudioProvider(
        formData.providerType,
        formData.apiKey,
        formData.projectId,
      );

      if (!isValid) {
        setValidationError("Invalid API key or credentials");
        return;
      }

      await onSave(formData);
    } catch (e) {
      setValidationError(e instanceof Error ? e.message : "Verification failed");
    } finally {
      setIsSaving(false);
    }
  };

  if (!formData) return null;

  return (
    <BottomMenu
      isOpen={isOpen}
      onClose={onClose}
      title={formData.id ? "Edit Provider" : "Add Audio Provider"}
    >
      <div className="space-y-4 pb-2">
        <div>
          <label className="mb-1 block text-[11px] font-medium text-fg/70">Provider Type</label>
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
                requestPath: next === "openai_tts" ? "/v1/audio/speech" : undefined,
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
            <option value="gemini_tts" className="bg-surface-el">
              Gemini TTS (Google)
            </option>
            <option value="openai_tts" className="bg-surface-el">
              OpenAI-Compatible TTS
            </option>
            {isKokoro && (
              <option value="kokoro" className="bg-surface-el">
                Kokoro (Local)
              </option>
            )}
          </select>
        </div>

        <div>
          <label className="mb-1 block text-[11px] font-medium text-fg/70">Label</label>
          <input
            type="text"
            value={formData.label}
            onChange={(e) => setFormData({ ...formData, label: e.target.value })}
            placeholder={
              formData.providerType === "gemini_tts"
                ? "My Gemini TTS"
                : formData.providerType === "openai_tts"
                  ? "My Compatible TTS"
                  : formData.providerType === "kokoro"
                    ? "Kokoro Local"
                    : "My ElevenLabs"
            }
            className="w-full rounded-lg border border-fg/10 bg-surface-el/20 px-3 py-2 text-sm text-fg placeholder-fg/40 focus:border-fg/30 focus:outline-none"
          />
        </div>

        {!isKokoro && (
          <div>
            <label className="mb-1 block text-[11px] font-medium text-fg/70">API Key</label>
            <input
              type="password"
              value={formData.apiKey ?? ""}
              onChange={(e) => {
                setFormData({ ...formData, apiKey: e.target.value });
                if (validationError) setValidationError(null);
              }}
              placeholder="Enter your API key"
              className="w-full rounded-lg border border-fg/10 bg-surface-el/20 px-3 py-2 text-sm text-fg placeholder-fg/40 focus:border-fg/30 focus:outline-none"
            />
          </div>
        )}

        {isKokoro && (
          <>
            <div>
              <label className="mb-1 block text-[11px] font-medium text-fg/70">Model Variant</label>
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
                    Loading variants...
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
                Mobile builds only support int8. Install the model from the Kokoro Studio after
                saving.
              </p>
            </div>
            <div>
              <label className="mb-1 block text-[11px] font-medium text-fg/70">Asset Root</label>
              <div className="flex gap-2">
                <input
                  type="text"
                  value={formData.assetRoot ?? ""}
                  onChange={(e) => {
                    setFormData({ ...formData, assetRoot: e.target.value });
                    if (validationError) setValidationError(null);
                  }}
                  placeholder={managedRoot || "/path/to/kokoro"}
                  className="min-w-0 flex-1 rounded-lg border border-fg/10 bg-surface-el/20 px-3 py-2 text-sm text-fg placeholder-fg/40 focus:border-fg/30 focus:outline-none"
                />
                <button
                  type="button"
                  onClick={() => void browseAssetRoot()}
                  className="rounded-lg border border-fg/10 bg-fg/5 px-3 py-2 text-xs text-fg/70 transition hover:border-fg/20 hover:bg-fg/10"
                >
                  Browse
                </button>
                {managedRoot && (
                  <button
                    type="button"
                    onClick={() => setFormData({ ...formData, assetRoot: managedRoot })}
                    className="rounded-lg border border-info/25 bg-info/10 px-3 py-2 text-xs text-info/85 transition hover:border-info/40 hover:bg-info/15"
                  >
                    Managed
                  </button>
                )}
              </div>
              {managedRoot && (
                <p className="mt-1 break-all text-[10px] text-fg/40">Managed: {managedRoot}</p>
              )}
            </div>
          </>
        )}

        {formData.providerType === "gemini_tts" && (
          <>
            <div>
              <label className="mb-1 block text-[11px] font-medium text-fg/70">
                Google Cloud Project ID
              </label>
              <input
                type="text"
                value={formData.projectId ?? ""}
                onChange={(e) => {
                  setFormData({ ...formData, projectId: e.target.value });
                  if (validationError) setValidationError(null);
                }}
                placeholder="your-project-id"
                className="w-full rounded-lg border border-fg/10 bg-surface-el/20 px-3 py-2 text-sm text-fg placeholder-fg/40 focus:border-fg/30 focus:outline-none"
              />
            </div>
            <div>
              <label className="mb-1 block text-[11px] font-medium text-fg/70">
                Region (optional)
              </label>
              <input
                type="text"
                value={formData.location ?? ""}
                onChange={(e) => setFormData({ ...formData, location: e.target.value })}
                placeholder="us-central1"
                className="w-full rounded-lg border border-fg/10 bg-surface-el/20 px-3 py-2 text-sm text-fg placeholder-fg/40 focus:border-fg/30 focus:outline-none"
              />
            </div>
          </>
        )}

        {formData.providerType === "openai_tts" && (
          <>
            <div>
              <label className="mb-1 block text-[11px] font-medium text-fg/70">Base URL</label>
              <input
                type="text"
                value={formData.baseUrl ?? ""}
                onChange={(e) => {
                  setFormData({ ...formData, baseUrl: e.target.value });
                  if (validationError) setValidationError(null);
                }}
                placeholder="https://api.example.com"
                className="w-full rounded-lg border border-fg/10 bg-surface-el/20 px-3 py-2 text-sm text-fg placeholder-fg/40 focus:border-fg/30 focus:outline-none"
              />
            </div>
            <div>
              <label className="mb-1 block text-[11px] font-medium text-fg/70">Request Path</label>
              <input
                type="text"
                value={formData.requestPath ?? "/v1/audio/speech"}
                onChange={(e) => setFormData({ ...formData, requestPath: e.target.value })}
                placeholder="/v1/audio/speech"
                className="w-full rounded-lg border border-fg/10 bg-surface-el/20 px-3 py-2 text-sm text-fg placeholder-fg/40 focus:border-fg/30 focus:outline-none"
              />
              <p className="mt-1 text-[10px] text-fg/40">
                Use the provider path if it differs from the OpenAI default
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
            Cancel
          </button>
          <button
            onClick={() => void handleSave()}
            disabled={isSaving || !formData.label.trim()}
            className="flex-1 rounded-lg border border-accent/40 bg-accent/20 px-4 py-2 text-sm font-semibold text-accent/90 transition hover:border-accent/60 hover:bg-accent/30 disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {isSaving ? "Verifying..." : "Save"}
          </button>
        </div>
      </div>
    </BottomMenu>
  );
}
