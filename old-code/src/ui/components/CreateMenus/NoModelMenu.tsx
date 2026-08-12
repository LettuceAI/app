import { AlertTriangle, Check, Cpu, HelpCircle } from "lucide-react";
import { useNavigate } from "react-router-dom";
import { BottomMenu, MenuButton, MenuSection } from "../BottomMenu";
import { useI18n } from "../../../core/i18n/context";

export function NoModelPanel({ onClose }: { onClose: () => void }) {
  const { t } = useI18n();
  const navigate = useNavigate();

  return (
    <MenuSection>
      <div className="flex items-start gap-3 rounded-xl border border-red-500/40 bg-red-500/10 p-4">
        <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-red-500/25 text-red-300">
          <AlertTriangle size={18} strokeWidth={2.5} />
        </div>
        <div className="space-y-1">
          <p className="text-sm font-semibold text-red-200">
            {t("components.createMenu.noModel.heading")}
          </p>
          <p className="text-[13px] leading-relaxed text-red-100/80">
            {t("components.createMenu.noModel.body")}
          </p>
        </div>
      </div>

      <MenuButton
        icon={Cpu}
        title={t("components.createMenu.noModel.goToModels")}
        description={t("components.createMenu.noModel.goToModelsDesc")}
        color="from-emerald-500 to-emerald-600"
        onClick={() => {
          onClose();
          navigate("/settings/models");
        }}
      />

      <MenuButton
        icon={HelpCircle}
        title={t("components.createMenu.noModel.dontKnow")}
        description={t("components.createMenu.noModel.dontKnowDesc")}
        color="from-blue-500 to-blue-600"
        onClick={() => {
          onClose();
          navigate("/settings/help");
        }}
      />

      <MenuButton
        icon={Check}
        title={t("components.createMenu.noModel.okay")}
        description={t("components.createMenu.noModel.okayDesc")}
        color="from-white/20 to-white/10"
        onClick={onClose}
      />
    </MenuSection>
  );
}

export function NoModelMenu({ isOpen, onClose }: { isOpen: boolean; onClose: () => void }) {
  const { t } = useI18n();

  return (
    <BottomMenu
      isOpen={isOpen}
      onClose={onClose}
      title={t("components.createMenu.noModel.title")}
      includeExitIcon={false}
      location="bottom"
    >
      <NoModelPanel onClose={onClose} />
    </BottomMenu>
  );
}
