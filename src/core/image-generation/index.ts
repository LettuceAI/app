import { invoke } from "@tauri-apps/api/core";
import { convertFileSrc } from "@tauri-apps/api/core";
import type {
  AdvancedModelSettings,
  Model,
  PromptEntryCondition,
  ProviderCredential,
  Settings,
} from "../storage/schemas";
import { convertToImageUrl } from "../storage/images";
import { getPromptTemplate } from "../prompts/service";
import { isRenderableImageUrl } from "../utils/image";
import {
  APP_AVATAR_EDIT_TEMPLATE_ID,
  APP_AVATAR_GENERATION_TEMPLATE_ID,
} from "../prompts/constants";

/**
 * Image generation request parameters
 */
export interface ImageGenerationRequest {
  prompt: string;
  model: string;
  providerId: string;
  credentialId: string;
  advancedModelSettings?: AdvancedModelSettings | null;
  inputImages?: string[];
  size?: string;
  quality?: string;
  style?: string;
  n?: number;
}

/**
 * Generated image result
 */
export interface GeneratedImage {
  assetId: string;
  filePath: string;
  mimeType: string;
  url?: string;
  width?: number;
  height?: number;
  text?: string;
}

/**
 * Image generation response
 */
export interface ImageGenerationResponse {
  images: GeneratedImage[];
  model: string;
  providerId: string;
}

/**
 * Generate images using the specified model and provider
 */
export async function generateImage(
  request: ImageGenerationRequest,
): Promise<ImageGenerationResponse> {
  if (!request.prompt.trim()) {
    throw new Error("Prompt cannot be empty");
  }

  return invoke<ImageGenerationResponse>("generate_image", {
    request: {
      prompt: request.prompt,
      model: request.model,
      providerId: request.providerId,
      credentialId: request.credentialId,
      advancedModelSettings: request.advancedModelSettings ?? null,
      inputImages: request.inputImages ?? null,
      size: request.size ?? null,
      quality: request.quality ?? null,
      style: request.style ?? null,
      n: request.n ?? 1,
    },
  });
}

type PromptTemplateLike = {
  content: string;
  entries?: Array<{
    content: string;
    enabled?: boolean;
    conditions?: PromptEntryCondition | null;
  }>;
};

type AvatarPromptContext = {
  subjectName?: string | null;
  subjectDescription?: string | null;
  avatarRequest?: string | null;
  currentAvatarPrompt?: string | null;
  editRequest?: string | null;
};

type AvatarPromptConditionContext = {
  hasSubjectDescription: boolean;
  hasCurrentDescription: boolean;
};

const FALLBACK_AVATAR_GENERATION_TEMPLATE = [
  "Generate a character avatar image directly from this request.",
  "Subject: {{avatar_subject_name}}",
  "{{avatar_subject_description}}",
  "Avatar request: {{avatar_request}}",
  "Create a centered, profile-friendly image that preserves identity-defining traits.",
  "Prioritize face, hair, clothing, expression, pose, and overall vibe.",
  "Do not add text, logos, watermarks, frames, UI, or split panels unless explicitly requested.",
  "Return only the image.",
].join("\n\n");

const FALLBACK_AVATAR_EDIT_TEMPLATE = [
  "Edit the provided avatar image directly using the source image and the request below.",
  "Subject: {{avatar_subject_name}}",
  "{{avatar_subject_description}}",
  "Current avatar notes: {{current_avatar_prompt}}",
  "Edit request: {{edit_request}}",
  "Preserve identity and keep unchanged traits consistent with the source image.",
  "Change only what the edit request asks for.",
  "Return only the edited image.",
].join("\n\n");

function matchesAvatarPromptCondition(
  condition: PromptEntryCondition,
  context: AvatarPromptConditionContext,
): boolean {
  switch (condition.type) {
    case "hasSubjectDescription":
      return context.hasSubjectDescription === condition.value;
    case "hasCurrentDescription":
      return context.hasCurrentDescription === condition.value;
    case "all":
      return condition.conditions.every((child) => matchesAvatarPromptCondition(child, context));
    case "any":
      return (
        condition.conditions.length > 0 &&
        condition.conditions.some((child) => matchesAvatarPromptCondition(child, context))
      );
    case "not":
      return !matchesAvatarPromptCondition(condition.condition, context);
    default:
      return true;
  }
}

function resolveTemplateContent(
  template: PromptTemplateLike | null,
  fallback: string,
  context: AvatarPromptConditionContext,
): string {
  if (!template) return fallback;

  const mergedEntries =
    template.entries
      ?.filter(
        (entry) =>
          entry.enabled !== false &&
          entry.content.trim().length > 0 &&
          (!entry.conditions || matchesAvatarPromptCondition(entry.conditions, context)),
      )
      .map((entry) => entry.content)
      .join("\n\n") ?? "";

  return mergedEntries.trim() || template.content.trim() || fallback;
}

function applyTemplateVariables(template: string, context: AvatarPromptContext): string {
  const subjectName = context.subjectName?.trim() ?? "";
  const subjectDescription = context.subjectDescription?.trim() ?? "";
  const replacements: Record<string, string> = {
    "{{avatar_subject_name}}": subjectName,
    "{{avatar_subject_description}}": subjectDescription,
    "{{avatar_request}}": context.avatarRequest?.trim() ?? "",
    "{{current_avatar_prompt}}": context.currentAvatarPrompt?.trim() ?? "",
    "{{edit_request}}": context.editRequest?.trim() ?? "",
    // Backward-compatible aliases in case a user customized these templates already.
    "{{char.name}}": subjectName,
    "{{char.desc}}": subjectDescription,
    "{{persona.name}}": subjectName,
    "{{persona.desc}}": subjectDescription,
  };

  let result = template;
  for (const [placeholder, value] of Object.entries(replacements)) {
    result = result.split(placeholder).join(value);
  }

  return result.replace(/\n{3,}/g, "\n\n").trim();
}

async function buildAvatarTemplatePrompt(
  templateId: string,
  context: AvatarPromptContext,
  fallback: string,
): Promise<string> {
  try {
    const template = await getPromptTemplate(templateId);
    const content = resolveTemplateContent(template, fallback, {
      hasSubjectDescription: Boolean(context.subjectDescription?.trim()),
      hasCurrentDescription: Boolean(context.currentAvatarPrompt?.trim()),
    });
    return applyTemplateVariables(content, context);
  } catch (error) {
    console.warn("Failed to load avatar prompt template, using fallback:", error);
    return applyTemplateVariables(fallback, context);
  }
}

export async function buildAvatarGenerationPrompt(context: AvatarPromptContext): Promise<string> {
  return buildAvatarTemplatePrompt(
    APP_AVATAR_GENERATION_TEMPLATE_ID,
    context,
    FALLBACK_AVATAR_GENERATION_TEMPLATE,
  );
}

export async function buildAvatarEditPrompt(context: AvatarPromptContext): Promise<string> {
  return buildAvatarTemplatePrompt(
    APP_AVATAR_EDIT_TEMPLATE_ID,
    context,
    FALLBACK_AVATAR_EDIT_TEMPLATE,
  );
}

export interface ImageGenerationOptions {
  models: Model[];
  providers: ProviderCredential[];
  defaultModel: Model | null;
  defaultProvider: ProviderCredential | null;
  enabled: boolean;
}

export function isImageTextToTextModel(model: Model): boolean {
  const inputScopes = model.inputScopes ?? [];
  const outputScopes = model.outputScopes ?? [];
  return (
    inputScopes.includes("text") && inputScopes.includes("image") && outputScopes.includes("text")
  );
}

function resolveImageGenerationOptionsWithPreference(
  settings: Settings,
  preferredModelId?: string | null,
  enabled = true,
): ImageGenerationOptions {
  const models = settings.models.filter((model) => model.outputScopes?.includes("image"));
  const providers = settings.providerCredentials;
  if (!enabled) {
    return {
      models,
      providers,
      defaultModel: null,
      defaultProvider: null,
      enabled: false,
    };
  }
  const defaultModel =
    (preferredModelId ? models.find((model) => model.id === preferredModelId) : null) ??
    models[0] ??
    null;
  const defaultProvider = defaultModel
    ? resolveProviderCredential(providers, defaultModel.providerId, defaultModel.providerLabel)
    : null;

  return {
    models,
    providers,
    defaultModel,
    defaultProvider,
    enabled: true,
  };
}

export function resolveImageGenerationOptions(settings: Settings): ImageGenerationOptions {
  return resolveImageGenerationOptionsWithPreference(settings);
}

export function resolveAvatarGenerationOptions(settings: Settings): ImageGenerationOptions {
  return resolveImageGenerationOptionsWithPreference(
    settings,
    settings.advancedSettings?.avatarGenerationModelId,
    settings.advancedSettings?.avatarGenerationEnabled ?? true,
  );
}

export function resolveSceneGenerationOptions(settings: Settings): ImageGenerationOptions {
  return resolveImageGenerationOptionsWithPreference(
    settings,
    settings.advancedSettings?.sceneGenerationModelId,
    settings.advancedSettings?.sceneGenerationEnabled ?? true,
  );
}

export function resolveSceneWriterOptions(settings: Settings): ImageGenerationOptions {
  const models = settings.models.filter(isImageTextToTextModel);
  const providers = settings.providerCredentials;
  const preferredModelId = settings.advancedSettings?.sceneWriterModelId;
  const defaultModel =
    (preferredModelId ? models.find((model) => model.id === preferredModelId) : null) ??
    models[0] ??
    null;
  const defaultProvider = defaultModel
    ? resolveProviderCredential(providers, defaultModel.providerId, defaultModel.providerLabel)
    : null;

  return {
    models,
    providers,
    defaultModel,
    defaultProvider,
    enabled: models.length > 0,
  };
}

export function resolveProviderCredential(
  providers: ProviderCredential[],
  providerId: string,
  providerLabel?: string | null,
): ProviderCredential | null {
  return (
    providers.find(
      (provider) => provider.providerId === providerId && provider.label === providerLabel,
    ) ??
    providers.find((provider) => provider.providerId === providerId) ??
    null
  );
}

export async function resolveGeneratedImageUrl(image: GeneratedImage): Promise<string | undefined> {
  if (isRenderableImageUrl(image.url)) {
    return image.url;
  }

  if (image.assetId) {
    return convertToImageUrl(image.assetId);
  }

  if (image.filePath) {
    return convertFileSrc(image.filePath);
  }

  return undefined;
}

/**
 * Get available sizes for a model
 */
export function getModelSizes(providerId: string, modelId: string): readonly string[] {
  if (providerId === "openai") {
    if (modelId === "dall-e-3") {
      return ["1024x1024", "1024x1792", "1792x1024"];
    }

    if (modelId === "dall-e-2") {
      return ["256x256", "512x512", "1024x1024"];
    }

    if (modelId.startsWith("gpt-image-1")) {
      return ["1024x1024", "1024x1536", "1536x1024", "auto"];
    }
  }

  if (providerId === "automatic1111") {
    return ["512x512", "768x768", "1024x1024", "1152x896", "896x1152"];
  }

  if (providerId === "stability") {
    return ["512x512", "768x768", "1024x1024", "1152x896", "896x1152"];
  }

  return ["1024x1024"];
}
