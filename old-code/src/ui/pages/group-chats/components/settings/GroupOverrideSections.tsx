import { useCallback, useEffect, useMemo, useState } from "react";
import { Cpu, Loader2 } from "lucide-react";

import { cn, radius, typography } from "../../../../design-tokens";
import { ModelSelectionBottomMenu } from "../../../../components/ModelSelectionBottomMenu";
import { readSettings } from "../../../../../core/storage/repo";
import { listPromptTemplates } from "../../../../../core/prompts";
import {
  APP_GROUP_CHAT_ROLEPLAY_TEMPLATE_ID,
  APP_GROUP_CHAT_TEMPLATE_ID,
} from "../../../../../core/prompts/constants";
import { useI18n } from "../../../../../core/i18n/context";
import type {
  Character,
  Model,
  SystemPromptTemplate,
} from "../../../../../core/storage/schemas";
import { SectionHeader } from "./SectionHeader";
import { CharacterAvatar } from "./CharacterAvatar";

export function GroupCharacterModelsSection({
  characters,
  overrides,
  onChange,
  disabled = false,
  footer,
}: {
  characters: Character[];
  overrides: Record<string, string>;
  onChange: (characterId: string, modelId: string | null) => void;
  disabled?: boolean;
  footer?: React.ReactNode;
}) {
  const { t } = useI18n();
  const [models, setModels] = useState<Model[]>([]);
  const [globalDefaultModelId, setGlobalDefaultModelId] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [pickerCharacterId, setPickerCharacterId] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void readSettings()
      .then((settings) => {
        if (cancelled) return;
        setModels(settings.models);
        setGlobalDefaultModelId(settings.defaultModelId ?? null);
      })
      .catch((error: unknown) => {
        console.error("Failed to load models:", error);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const modelName = useCallback(
    (modelId: string | null | undefined) =>
      models.find((model) => model.id === modelId)?.displayName ?? null,
    [models],
  );

  const describe = useCallback(
    (character: Character) => {
      const override = overrides[character.id];
      if (override) {
        return modelName(override) ?? t("groupChats.modelOverrides.missingModel");
      }
      const inherited = modelName(character.defaultModelId) ?? modelName(globalDefaultModelId);
      return inherited
        ? t("groupChats.modelOverrides.inheritedValue", { model: inherited })
        : t("groupChats.modelOverrides.noModel");
    },
    [globalDefaultModelId, modelName, overrides, t],
  );

  const pickerCharacter = characters.find((character) => character.id === pickerCharacterId);

  return (
    <>
      <SectionHeader
        title={t("groupChats.modelOverrides.title")}
        subtitle={t("groupChats.modelOverrides.subtitle")}
      />
      {loading ? (
        <div className="flex items-center gap-2 rounded-xl border border-fg/10 bg-surface-el/85 px-4 py-3">
          <Loader2 className="h-4 w-4 animate-spin text-fg/50" />
          <span className="text-sm text-fg/50">{t("groupChats.modelOverrides.loading")}</span>
        </div>
      ) : (
        <div className="space-y-2">
          {characters.map((character) => (
            <button
              key={character.id}
              type="button"
              onClick={() => setPickerCharacterId(character.id)}
              disabled={disabled}
              className={cn(
                "flex w-full items-center gap-3 p-3 text-left",
                radius.lg,
                "border border-fg/10 bg-surface-el/85 transition",
                disabled ? "opacity-50" : "hover:border-fg/20 hover:bg-fg/10",
              )}
            >
              <CharacterAvatar character={character} size="md" />
              <div className="min-w-0 flex-1">
                <p className="truncate text-sm font-medium text-fg">{character.name}</p>
                <p className={cn(typography.caption.size, "truncate text-fg/50")}>
                  {describe(character)}
                </p>
              </div>
              {overrides[character.id] ? (
                <span
                  className={cn(
                    typography.caption.size,
                    "shrink-0 rounded-full border border-accent/30 bg-accent/10 px-2 py-0.5 text-accent/80",
                  )}
                >
                  {t("groupChats.modelOverrides.overrideBadge")}
                </span>
              ) : null}
              <Cpu className="h-4 w-4 shrink-0 text-fg/40" />
            </button>
          ))}
        </div>
      )}
      {footer}

      <ModelSelectionBottomMenu
        isOpen={Boolean(pickerCharacter)}
        onClose={() => setPickerCharacterId(null)}
        title={
          pickerCharacter
            ? t("groupChats.modelOverrides.selectFor", { name: pickerCharacter.name })
            : t("groupChats.modelOverrides.title")
        }
        models={models}
        selectedModelIds={
          pickerCharacter && overrides[pickerCharacter.id] ? [overrides[pickerCharacter.id]] : []
        }
        searchPlaceholder={t("chats.settings.searchModels")}
        theme="dark"
        tone="emerald"
        location="bottom"
        onSelectModel={(modelId) => {
          if (pickerCharacter) onChange(pickerCharacter.id, modelId);
          setPickerCharacterId(null);
        }}
        clearOption={{
          label: t("groupChats.modelOverrides.useCharacterDefault"),
          icon: Cpu,
          selected: Boolean(pickerCharacter && !overrides[pickerCharacter.id]),
          onClick: () => {
            if (pickerCharacter) onChange(pickerCharacter.id, null);
            setPickerCharacterId(null);
          },
        }}
      />
    </>
  );
}

export function GroupPromptTemplateSection({
  chatType,
  templateId,
  onChange,
  disabled = false,
  footer,
}: {
  chatType: "conversation" | "roleplay";
  templateId: string | null;
  onChange: (templateId: string | null) => void;
  disabled?: boolean;
  footer?: React.ReactNode;
}) {
  const { t } = useI18n();
  const [templates, setTemplates] = useState<SystemPromptTemplate[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    void listPromptTemplates()
      .then((items) => {
        if (!cancelled) setTemplates(items);
      })
      .catch((error: unknown) => {
        console.error("Failed to load prompt templates:", error);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const isRoleplay = chatType === "roleplay";
  const available = useMemo(
    () =>
      templates.filter((template) =>
        isRoleplay
          ? template.promptType === "groupChatRoleplay" &&
            template.id !== APP_GROUP_CHAT_ROLEPLAY_TEMPLATE_ID
          : template.promptType === "groupChatConversational" &&
            template.id !== APP_GROUP_CHAT_TEMPLATE_ID,
      ),
    [isRoleplay, templates],
  );

  return (
    <>
      <SectionHeader
        title={t("groupChats.promptOverride.title")}
        subtitle={
          isRoleplay
            ? t("groupChats.promptOverride.roleplaySubtitle")
            : t("groupChats.promptOverride.conversationSubtitle")
        }
      />
      {loading ? (
        <div className="flex items-center gap-2 rounded-xl border border-fg/10 bg-surface-el/85 px-4 py-3">
          <Loader2 className="h-4 w-4 animate-spin text-fg/50" />
          <span className="text-sm text-fg/50">{t("characters.edit.loadingTemplates")}</span>
        </div>
      ) : available.length > 0 ? (
        <select
          value={templateId ?? ""}
          disabled={disabled}
          onChange={(event) => onChange(event.target.value || null)}
          className="w-full appearance-none rounded-xl border border-fg/10 bg-surface-el/85 px-3.5 py-3 text-sm text-fg transition focus:border-fg/25 focus:outline-none disabled:opacity-50"
        >
          <option value="">{t("groupChats.promptOverride.useCharacterDefault")}</option>
          {available.map((template) => (
            <option key={template.id} value={template.id}>
              {template.name}
            </option>
          ))}
        </select>
      ) : (
        <div className="rounded-xl border border-fg/10 bg-surface-el/85 px-4 py-3">
          <p className="text-sm text-fg/50">{t("characters.edit.usingAppDefault")}</p>
          <p className="mt-1 text-xs text-fg/40">
            {isRoleplay
              ? t("characters.edit.noGroupRoleplayTemplatesHint")
              : t("characters.edit.noGroupConversationTemplatesHint")}
          </p>
        </div>
      )}
      <p className={cn(typography.caption.size, "mt-2 px-1 text-fg/45")}>
        {t("groupChats.promptOverride.hint")}
      </p>
      {footer}
    </>
  );
}
