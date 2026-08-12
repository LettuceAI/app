import { Rocket } from "lucide-react";
import { useRocketEasterEgg } from "../../../hooks/useRocketEasterEgg";
import { useNavigationManager, Routes } from "../../../navigation";
import { useI18n } from "../../../../core/i18n/context";

interface EmptyStateProps {
  title: string;
  showBackButton?: boolean;
}

export function EmptyState({ title, showBackButton = true }: EmptyStateProps) {
  const { backOrReplace } = useNavigationManager();
  const { t } = useI18n();
  const rocket = useRocketEasterEgg();

  return (
    <div
      className="relative flex h-full flex-col items-center justify-center gap-6 text-center text-gray-400 overflow-hidden"
      {...rocket.bind}
    >
      {rocket.isLaunched && (
        <div className="pointer-events-none absolute bottom-6 left-1/2 -translate-x-1/2 rocket-launch">
          <div className="flex h-10 w-10 items-center justify-center rounded-full border border-white/10 bg-white/10">
            <Rocket className="h-5 w-5 text-white/80" />
          </div>
        </div>
      )}
      <p className="text-lg font-semibold text-white">{title}</p>
      {showBackButton && (
        <button
          onClick={() => backOrReplace(Routes.chat)}
          className="rounded-full border border-white/15 px-5 py-2 text-sm font-medium text-white transition hover:border-white/30"
        >
          {t("chats.emptyState.goBack")}
        </button>
      )}
    </div>
  );
}
