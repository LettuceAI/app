import { useI18n } from "../../../../core/i18n/context";

type Props = {
  name: string;
  era: string;
  role: string;
  setting: string;
  coreIdentity: string;
  backstory: string;
  boosted: boolean;
  onFieldChange: (field: string, value: string) => void;
  onNext: () => void;
};

export function CharacterIdentityStep({
  name,
  era,
  role,
  setting,
  coreIdentity,
  backstory,
  boosted,
  onFieldChange,
  onNext,
}: Props) {
  const { t } = useI18n();
  const canContinue = name.trim().length > 0;

  return (
    <div className="space-y-4 px-4 py-6">
      <div className="flex items-center gap-2">
        <h2 className="text-lg font-semibold text-white">{t("engine.characterCreate.steps.identity.title")}</h2>
        {boosted && (
          <span className="rounded-full bg-indigo-500/20 px-2 py-0.5 text-[10px] font-medium text-indigo-300">
            {t("engine.characterCreate.steps.identity.aiGenerated")}
          </span>
        )}
      </div>

      <div>
        <label className="mb-1 block text-[11px] font-medium text-white/70">{t("engine.characterCreate.steps.identity.nameLabel")}</label>
        <input
          type="text"
          value={name}
          onChange={(e) => onFieldChange("name", e.target.value)}
          placeholder={t("engine.characterCreate.steps.identity.namePlaceholder")}
          className="w-full rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-sm text-white placeholder-white/40 focus:border-white/30 focus:outline-none"
        />
      </div>

      <div className="grid grid-cols-2 gap-3">
        <div>
          <label className="mb-1 block text-[11px] font-medium text-white/70">{t("engine.characterCreate.steps.identity.eraLabel")}</label>
          <input
            type="text"
            value={era}
            onChange={(e) => onFieldChange("era", e.target.value)}
            placeholder={t("engine.characterCreate.steps.identity.eraPlaceholder")}
            className="w-full rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-sm text-white placeholder-white/40 focus:border-white/30 focus:outline-none"
          />
        </div>
        <div>
          <label className="mb-1 block text-[11px] font-medium text-white/70">{t("engine.characterCreate.steps.identity.roleLabel")}</label>
          <input
            type="text"
            value={role}
            onChange={(e) => onFieldChange("role", e.target.value)}
            placeholder={t("engine.characterCreate.steps.identity.rolePlaceholder")}
            className="w-full rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-sm text-white placeholder-white/40 focus:border-white/30 focus:outline-none"
          />
        </div>
      </div>

      <div>
        <label className="mb-1 block text-[11px] font-medium text-white/70">{t("engine.characterCreate.steps.identity.settingLabel")}</label>
        <textarea
          value={setting}
          onChange={(e) => onFieldChange("setting", e.target.value)}
          placeholder={t("engine.characterCreate.steps.identity.settingPlaceholder")}
          rows={2}
          className="w-full rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-sm text-white placeholder-white/40 focus:border-white/30 focus:outline-none resize-none"
        />
      </div>

      <div>
        <label className="mb-1 block text-[11px] font-medium text-white/70">{t("engine.characterCreate.steps.identity.coreIdentityLabel")}</label>
        <textarea
          value={coreIdentity}
          onChange={(e) => onFieldChange("coreIdentity", e.target.value)}
          placeholder={t("engine.characterCreate.steps.identity.coreIdentityPlaceholder")}
          rows={3}
          className="w-full rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-sm text-white placeholder-white/40 focus:border-white/30 focus:outline-none resize-none"
        />
      </div>

      <div>
        <label className="mb-1 block text-[11px] font-medium text-white/70">{t("engine.characterCreate.steps.identity.backstoryLabel")}</label>
        <textarea
          value={backstory}
          onChange={(e) => onFieldChange("backstory", e.target.value)}
          placeholder={t("engine.characterCreate.steps.identity.backstoryPlaceholder")}
          rows={4}
          className="w-full rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-sm text-white placeholder-white/40 focus:border-white/30 focus:outline-none resize-none"
        />
      </div>

      <button
        onClick={onNext}
        disabled={!canContinue}
        className="w-full rounded-lg border border-emerald-400/40 bg-emerald-500/20 px-4 py-3 text-sm font-semibold text-emerald-100 transition hover:border-emerald-400/60 hover:bg-emerald-500/30 disabled:cursor-not-allowed disabled:opacity-50"
      >
        {t("common.buttons.continue")}
      </button>
    </div>
  );
}
