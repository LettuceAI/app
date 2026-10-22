import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { FolderSearch } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Alert, Badge, Button, icon, ListGroup, ListRow, ListRowButton, Skeleton, Text } from "@lettuceai/crisp";
import type { SdCatalogProfile } from "@/api/generated/bindings";
import { FailureState } from "@/shared/ui/FailureState";
import { useClosingValue } from "@/shared/hooks/useClosingValue";
import { useSettingsApi } from "../data";
import { CatalogModelSheet } from "./CatalogModelSheet";
import { variantName } from "./fit";
import { installedQuery, inventoryQuery, localImageKeys, useByteSize, type ImageInstallDetail } from "./local-data";

export function CatalogView({ fitEpoch, onJob, onEngine, onBundle, activeInstalls, downloadsReady }: { fitEpoch: number; onJob: (id: string, title: string, detail: ImageInstallDetail) => void; onEngine: () => void; onBundle: () => void; activeInstalls: ImageInstallDetail[]; downloadsReady: boolean }) {
  const { t } = useTranslation("settings");
  const api = useSettingsApi();
  const size = useByteSize();
  const inventory = useQuery(inventoryQuery(api));
  const installed = useQuery(installedQuery(api));
  const catalog = useQuery({ queryKey: localImageKeys.catalog, queryFn: () => api.call("sdCatalog") });
  const [open, setOpen] = useState<string | null>(null);
  const { shown } = useClosingValue(open);
  const [variants, setVariants] = useState<Record<string, string>>({});
  const runtime = inventory.data?.active ?? null;
  const families = new Map<string, SdCatalogProfile[]>();
  for (const profile of catalog.data?.profiles ?? []) families.set(profile.family, [...(families.get(profile.family) ?? []), profile]);
  const profile = catalog.data?.profiles.find((item) => item.id === shown);
  return (
    <>
      {!runtime && inventory.data && <Alert tone="warning" title={t("image.local.catalog.engineFirst")} description={t("image.local.catalog.engineBody")} action={<Button size="sm" variant="secondary" onClick={onEngine}>{t("image.local.engine.setup")}</Button>} />}
      {inventory.isError && <FailureState error={inventory.error} onRetry={() => void inventory.refetch()} />}
      {installed.isError && <FailureState error={installed.error} onRetry={() => void installed.refetch()} />}
      {catalog.isError && <FailureState error={catalog.error} onRetry={() => void catalog.refetch()} />}
      {!catalog.data ? !catalog.isError && <div role="status" aria-label={t("image.local.catalog.loading")} className="space-y-7">
        {[2, 3, 2, 1].map((rows, group) => <ListGroup key={group} label={<span className="flex h-lh items-center"><Skeleton className="h-3 w-16" /></span>}>
          {Array.from({ length: rows }, (_, row) => <ListRow key={row} chevron title={<span className="flex h-lh items-center"><Skeleton className="h-4 w-40 max-w-full" /></span>} description={<span className="flex h-lh items-center"><Skeleton className="h-3 w-72 max-w-full" /></span>} />)}
        </ListGroup>)}
      </div> : !catalog.data.runtime_supported ? <Alert tone="warning" title={t("image.local.catalog.unsupported")} /> : [...families].map(([family, profiles]) => <ListGroup key={family} label={family}>
        {profiles.map((item) => {
          const mine = installed.data?.models.find((model) => model.profile_id === item.id);
          const variant = item.variants.find((entry) => entry.id === mine?.variant_id);
          const tooOld = runtime && item.minimum_runtime_build !== null && Number(/^master-(\d+)-/.exec(runtime.release)?.[1] ?? 0) < item.minimum_runtime_build;
          const sizes = item.variants.map((entry) => entry.download_bytes);
          const weight = mine && variant ? variantName(variant.label) : sizes.length > 1 ? t("image.local.catalog.sizeRange", { min: size(Math.min(...sizes)), max: size(Math.max(...sizes)) }) : size(sizes[0] ?? 0);
          return <ListRowButton key={item.id} title={item.display_name} description={[weight, t(item.supports_image_edit ? "image.local.catalog.drawsEdits" : "image.local.catalog.draws"), item.requires_reference_image ? t("image.local.catalog.reference") : null].filter(Boolean).join(" · ")} trailing={mine ? <Badge tone={mine.model_id ? "accent" : "warning"}>{t(mine.model_id ? "image.local.catalog.downloaded" : "image.local.catalog.notListed")}</Badge> : tooOld ? <Badge tone="warning">{t("image.local.catalog.newEngine")}</Badge> : undefined} onClick={() => setOpen(item.id)} onContextMenu={(event) => { event.preventDefault(); setOpen(item.id); }} />;
        })}
      </ListGroup>)}
      <ListGroup label={t("image.local.catalog.other")}>
        <ListRowButton icon={<FolderSearch size={icon.lg} />} title={t("image.local.bundle.title")} description={t("image.local.bundle.meta")} onClick={onBundle} />
      </ListGroup>
      <Text variant="hint">{t("image.local.catalog.editorHint")}</Text>
      {profile && <CatalogModelSheet key={profile.id} open={open !== null} onClose={() => setOpen(null)} profile={profile} runtime={runtime} installed={installed.data?.models.filter((model) => model.profile_id === profile.id)} selected={variants[profile.id] ?? null} onVariant={(id) => setVariants((current) => ({ ...current, [profile.id]: id }))} fitEpoch={fitEpoch} onJob={onJob} activeInstalls={activeInstalls} downloadsReady={downloadsReady} />}
    </>
  );
}
