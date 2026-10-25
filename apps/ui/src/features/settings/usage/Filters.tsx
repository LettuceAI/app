import { useTranslation } from "react-i18next";
import { Field, Segmented, SelectMenu } from "@lettuceai/crisp";
import type { Range } from "./data";
export function RangeControl({
  value,
  onChange,
  phone,
}: {
  value: Range;
  onChange: (range: Range) => void;
  phone: boolean;
}) {
  const { t } = useTranslation("usage");
  return (
    <Segmented<Range>
      size="sm"
      block={phone}
      aria-label={t("timeRange")}
      value={value}
      onChange={onChange}
      options={(["today", "7d", "30d", "all"] as const).map((value) => ({
        value,
        label: t(`ranges.${value}`),
      }))}
    />
  );
}
export function ReportFilters({
  range,
  onRange,
  provider,
  onProvider,
  providers,
  phone,
}: {
  range: Range;
  onRange: (range: Range) => void;
  provider: string;
  onProvider: (provider: string) => void;
  providers: { value: string; label: string }[];
  phone: boolean;
}) {
  const { t } = useTranslation("usage");
  return (
    <div className={phone ? "flex flex-col gap-2" : "flex items-center justify-between gap-2"}>
      <RangeControl value={range} onChange={onRange} phone={phone} />
      <Field label={<span className="sr-only">{t("provider")}</span>} className="space-y-0">
        <SelectMenu
          className={phone ? "w-full" : "w-48"}
          value={provider}
          onChange={(value) => onProvider(value ?? "all")}
          options={[{ value: "all", label: t("allProviders") }, ...providers]}
        />
      </Field>
    </div>
  );
}
