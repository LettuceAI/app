import { useTranslation } from "react-i18next";
import { Field, Input } from "@lettuceai/crisp";
import { ProviderMark, providerGroup } from "@/entities/provider";
import { useDescriptor, useOnboarding } from "../flow/context";

export function ProviderForm() {
  const { t } = useTranslation("onboarding");
  const { state, dispatch } = useOnboarding();
  const descriptor = useDescriptor(state.providerKind);
  const patch = (value: Partial<typeof state>) => dispatch({ type: "patch", patch: value });

  if (!descriptor) {
    return (
      <div className="flex h-56 flex-col items-center justify-center gap-1 rounded-2xl border border-dashed border-line-2 px-6 text-center">
        <p className="text-base text-fg-2">{t("connect.pickTitle")}</p>
        <p className="text-sm text-fg-3">{t("connect.pickBody")}</p>
      </div>
    );
  }
  return (
    <div className="space-y-4">
      <div className="flex items-center gap-3">
        <span className="flex h-10 w-10 items-center justify-center rounded-xl bg-fill-2">
          <ProviderMark kind={descriptor.kind} name={descriptor.display_name} />
        </span>
        <h2 className="text-2xl font-semibold text-fg">{descriptor.display_name}</h2>
      </div>
      {descriptor.endpoint_editable && (
        <Field
          label={t("connect.form.address")}
          hint={providerGroup(descriptor) === "local" ? t("connect.form.addressHintLocal") : t("connect.form.addressHintCustom")}
        >
          <Input
            value={state.baseUrl}
            onChange={(event) => patch({ baseUrl: event.target.value })}
            placeholder={descriptor.default_endpoint ?? t("connect.form.addressPlaceholder")}
          />
        </Field>
      )}
      {descriptor.api_key !== "not_used" && (
        <Field
          label={descriptor.api_key === "optional" ? t("connect.form.apiKeyOptional") : t("connect.form.apiKey")}
          hint={t("connect.form.apiKeyHint")}
        >
          <Input
            type="password"
            value={state.apiKey}
            onChange={(event) => patch({ apiKey: event.target.value })}
            placeholder={t("connect.form.apiKeyPlaceholder")}
          />
        </Field>
      )}
      <Field label={t("connect.form.name")} hint={t("connect.form.nameHint")}>
        <Input
          value={state.label}
          onChange={(event) => patch({ label: event.target.value })}
          placeholder={t("connect.form.namePlaceholder", { name: descriptor.display_name })}
        />
      </Field>
    </div>
  );
}
