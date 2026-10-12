import { useTranslation } from "react-i18next";
import { cn } from "@lettuceai/crisp";
import type { ProviderDescriptorContract } from "@/api/generated/bindings";
import { ProviderMark, providerGroup, providerGroups, providerHost } from "@/entities/provider";
import { useOnboarding } from "../flow/context";

function matchesSearch(descriptor: ProviderDescriptorContract, search: string): boolean {
  const host = providerHost(descriptor) ?? "";
  return `${descriptor.display_name} ${host}`.toLowerCase().includes(search.toLowerCase());
}

export function ProviderList({ descriptors }: { descriptors: readonly ProviderDescriptorContract[] }) {
  const { t } = useTranslation("onboarding");
  const { state, dispatch, phone } = useOnboarding();
  const visible = descriptors.filter((descriptor) => matchesSearch(descriptor, state.search));
  return (
    <div className="flex flex-col gap-5">
      {providerGroups.map((group) => {
        const items = visible.filter((descriptor) => providerGroup(descriptor) === group);
        if (items.length === 0) return null;
        return (
          <div key={group}>
            <p className="mb-2 px-2 text-xs font-medium uppercase tracking-wider text-fg-3">{t(`connect.groups.${group}`)}</p>
            <div className={cn("grid gap-x-2 gap-y-0.5", !phone && "grid-cols-2")}>
              {items.map((descriptor) => (
                <button
                  key={descriptor.kind}
                  type="button"
                  aria-pressed={state.providerKind === descriptor.kind}
                  onClick={() => dispatch({ type: "chooseProvider", kind: descriptor.kind })}
                  className={cn(
                    "flex items-center gap-3 rounded-xl px-2 py-2 text-left transition-colors",
                    state.providerKind === descriptor.kind ? "bg-fill-2 ring-1 ring-inset ring-accent/60" : "hover:bg-fill",
                  )}
                >
                  <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-fill-2">
                    <ProviderMark kind={descriptor.kind} name={descriptor.display_name} />
                  </span>
                  <span className="min-w-0 flex-1">
                    <span className="block truncate text-base text-fg">{descriptor.display_name}</span>
                    <span className="block truncate text-sm text-fg-3">
                      {providerHost(descriptor) ?? (group === "custom" ? t("connect.subtitle.custom") : t("connect.subtitle.local"))}
                    </span>
                  </span>
                </button>
              ))}
            </div>
          </div>
        );
      })}
    </div>
  );
}
