import { describeRequirement } from "../../../modelRequirements";
import { MissingModelRequirementsSheet } from "../../../components/MissingModelRequirementsSheet";
import { useI18n } from "../../../../core/i18n/context";

export interface DynamicMemoryEmbeddingPromptProps {
  onDownload: () => void;
  onContinueWithout: () => void;
}

export function DynamicMemoryEmbeddingPrompt({
  onDownload,
  onContinueWithout,
}: DynamicMemoryEmbeddingPromptProps) {
  const { t } = useI18n();
  return (
    <MissingModelRequirementsSheet
      isOpen
      title={t("onboarding.memory.embeddingPrompt.title")}
      description={t("onboarding.memory.embeddingPrompt.description")}
      missing={[describeRequirement("embedding")]}
      onClose={onContinueWithout}
      onDownload={onDownload}
      closeLabel={t("onboarding.memory.embeddingPrompt.continueWithout")}
      downloadLabel={t("onboarding.memory.embeddingPrompt.downloadModel")}
    />
  );
}
