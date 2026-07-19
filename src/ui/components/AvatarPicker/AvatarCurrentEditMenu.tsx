import { Crop, Sparkles } from "lucide-react";

import { BottomMenu, MenuButton, MenuButtonGroup } from "../BottomMenu";
import { useI18n } from "../../../core/i18n/context";

interface AvatarCurrentEditMenuProps {
  isOpen: boolean;
  onClose: () => void;
  onReposition: () => void;
  onEditWithAI: () => void;
  hasImageGenerationModels: boolean;
  showEditWithAI: boolean;
}

export function AvatarCurrentEditMenu({
  isOpen,
  onClose,
  onReposition,
  onEditWithAI,
  hasImageGenerationModels,
  showEditWithAI,
}: AvatarCurrentEditMenuProps) {
  const { t } = useI18n();

  return (
    <BottomMenu
      isOpen={isOpen}
      onClose={onClose}
      title={t("components.avatarCurrentEdit.title")}
    >
      <MenuButtonGroup>
        <MenuButton
          icon={Crop}
          title={t("components.avatarCurrentEdit.reposition")}
          description={t("components.avatarCurrentEdit.repositionDesc")}
          color="from-amber-500 to-orange-600"
          onClick={() => {
            onReposition();
            onClose();
          }}
        />
        {showEditWithAI && (
          <MenuButton
            icon={Sparkles}
            title={t("components.avatarCurrentEdit.editWithAI")}
            description={
              hasImageGenerationModels
                ? t("components.avatarCurrentEdit.editWithAIDesc")
                : t("components.avatarCurrentEdit.noImageModels")
            }
            color="from-emerald-500 to-emerald-600"
            onClick={() => {
              if (!hasImageGenerationModels) return;
              onEditWithAI();
              onClose();
            }}
            disabled={!hasImageGenerationModels}
          />
        )}
      </MenuButtonGroup>
      {!showEditWithAI && hasImageGenerationModels && (
        <p className="px-1 pt-3 text-center text-xs text-white/40">
          {t("components.avatarGeneration.localEditingUnavailable")}
        </p>
      )}
    </BottomMenu>
  );
}
