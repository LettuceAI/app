import { useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { FilePlus2, ShieldAlert, ShieldCheck, Trash2 } from "lucide-react";
import {
  confirm,
  icon,
  ListGroup,
  ListRow,
  ListRowButton,
  Page,
  Segmented,
  SkeletonRow,
  Spinner,
  Switch,
  Text,
  toast,
  relativeTime,
  useLongPress,
  Sheet,
} from "@lettuceai/crisp";
import { isApiFailure } from "@/api/client";
import type {
  CertificatesView,
  SettingsPatch_Serialize,
  SettingsPureMode,
  SettingsView,
  TrustedCertificateView,
} from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { useClosingValue } from "@/shared/hooks/useClosingValue";
import { FailureState } from "@/shared/ui/FailureState";
import { SettingsControlRow } from "@/shared/ui/SettingsControlRow";
import { useSettingsApi, useSettingsLayout } from "../data";
import { useSettingsWrite } from "../use-settings-write";

const levels = ["off", "low", "standard", "strict"] as const satisfies readonly SettingsPureMode[];

const general = (
  change: Partial<Pick<Extract<SettingsPatch_Serialize, { section: "general" }>, "pure_mode" | "auto_download_character_card_avatars">>,
): SettingsPatch_Serialize => ({
  section: "general",
  pure_mode: null,
  analytics_enabled: null,
  update_checks_enabled: null,
  developer_mode_enabled: null,
  auto_download_character_card_avatars: null,
  manual_mode_context_window: null,
  lorebook_scan_depth: null,
  ...change,
});

export function SecurityPage() {
  const { t } = useTranslation("settings");
  const api = useSettingsApi();
  const { back } = useSettingsLayout();
  const settings = useQuery({
    queryKey: queryKeys.settings.detail(),
    queryFn: () => api.call("settingsGet"),
  });
  let body;
  if (settings.isPending)
    body = (
      <div role="status" aria-busy="true" aria-label={t("security.loading")} className="space-y-7">
        <ListGroup label={t("security.pure.group")}>
          <SkeletonRow />
        </ListGroup>
        <ListGroup label={t("security.privacy.group")}>
          <SkeletonRow />
        </ListGroup>
        <ListGroup label={t("security.certificates.group")}>
          <SkeletonRow />
          <SkeletonRow />
        </ListGroup>
      </div>
    );
  else if (settings.isError)
    body = <FailureState error={settings.error} onRetry={() => void settings.refetch()} />;
  else
    body = (
      <div className="space-y-7">
        <FilteringGroups view={settings.data} />
        <CertificatesGroup view={settings.data} />
      </div>
    );
  return (
    <Page title={t("sections.security.title")} {...back}>
      {body}
    </Page>
  );
}

function FilteringGroups({ view }: { view: SettingsView }) {
  const { t } = useTranslation("settings");
  const api = useSettingsApi();
  const { phone } = useSettingsLayout();
  const write = useSettingsWrite(api);
  const [pendingLevel, setPendingLevel] = useState<SettingsPureMode | null>(null);
  const [pendingAvatars, setPendingAvatars] = useState<boolean | null>(null);
  const levelChange = useRef(0);
  const avatarChange = useRef(0);

  const level = pendingLevel ?? view.global.pure_mode;
  const avatars = pendingAvatars ?? view.global.auto_download_character_card_avatars;

  const setLevel = async (next: SettingsPureMode) => {
    const change = ++levelChange.current;
    setPendingLevel(next);
    await write((latest) =>
      latest.global.pure_mode === next ? null : general({ pure_mode: next }),
    );
    if (levelChange.current === change) setPendingLevel(null);
  };
  const setAvatars = async (next: boolean) => {
    const change = ++avatarChange.current;
    setPendingAvatars(next);
    await write((latest) =>
      latest.global.auto_download_character_card_avatars === next
        ? null
        : general({ auto_download_character_card_avatars: next }),
    );
    if (avatarChange.current === change) setPendingAvatars(null);
  };

  return (
    <>
      <ListGroup label={t("security.pure.group")}>
        <SettingsControlRow
          phone={phone}
          title={t("security.pure.title")}
          description={t(`security.pure.levels.${level}.description`)}
          control={
            <Segmented<SettingsPureMode>
              size="sm"
              block={phone}
              aria-label={t("security.pure.title")}
              value={level}
              onChange={(next) => void setLevel(next)}
              options={levels.map((value) => ({
                value,
                label: t(`security.pure.levels.${value}.label`),
              }))}
            />
          }
        />
      </ListGroup>
      <ListGroup label={t("security.privacy.group")}>
        <ListRow
          title={t("security.privacy.avatars")}
          description={t("security.privacy.avatarsBody")}
          trailing={
            <Switch
              checked={avatars}
              onChange={(next) => void setAvatars(next)}
              aria-label={t("security.privacy.avatars")}
            />
          }
        />
      </ListGroup>
    </>
  );
}

function CertificatesGroup({ view }: { view: SettingsView }) {
  const { t } = useTranslation("settings");
  const { t: common } = useTranslation("common");
  const api = useSettingsApi();
  const queryClient = useQueryClient();
  const [selected, setSelected] = useState<TrustedCertificateView | null>(null);
  const closing = useClosingValue(selected);
  const [busy, setBusy] = useState<"import" | "remove" | null>(null);

  const failure = (error: unknown) => {
    if (isApiFailure(error)) {
      if (error.details?.type === "certificate_already_imported")
        return t("security.certificates.alreadyImported");
      if (
        error.code === "invalid_input" &&
        error.details?.type === "invalid_field" &&
        error.details.field === "source"
      )
        return t("security.certificates.notPem");
    }
    return common(`errors.code.${isApiFailure(error) ? error.code : "internal"}`);
  };

  const store = async (stored: CertificatesView) => {
    queryClient.setQueryData<SettingsView>(queryKeys.settings.detail(), (current) =>
      current
        ? {
            ...current,
            device: {
              ...current.device,
              trusted_certificates: stored.certificates,
              revision: stored.revision,
            },
          }
        : current,
    );
    await queryClient.invalidateQueries({ queryKey: queryKeys.settings.all });
  };

  const importCertificate = async () => {
    if (busy) return;
    setBusy("import");
    try {
      const picked = await api.call("filesPickOpen", { kinds: ["certificate"], multiple: false });
      const source = picked.sources[0];
      if (!source) return;
      await store(
        await api.call("certificatesImport", {
          source,
          client_operation_id: crypto.randomUUID(),
        }),
      );
    } catch (error) {
      toast.error(failure(error));
      await queryClient.invalidateQueries({ queryKey: queryKeys.settings.all });
    } finally {
      setBusy(null);
    }
  };

  const removeCertificate = async (certificate: TrustedCertificateView) => {
    setSelected(null);
    const sure = await confirm({
      title: t("security.certificates.removeTitle"),
      message: t("security.certificates.removeBody", { name: certificate.name }),
      confirmLabel: t("security.certificates.remove"),
      cancelLabel: common("actions.cancel"),
      destructive: true,
    });
    if (!sure) return;
    const current = queryClient.getQueryData<SettingsView>(queryKeys.settings.detail());
    if (!current || busy) return;
    setBusy("remove");
    try {
      await store(
        await api.call("certificatesRemove", {
          certificate_id: certificate.id,
          expected_revision: current.device.revision,
        }),
      );
    } catch (error) {
      toast.error(failure(error));
      await queryClient.invalidateQueries({ queryKey: queryKeys.settings.all });
    } finally {
      setBusy(null);
    }
  };

  return (
    <>
      <div className="space-y-2">
        <ListGroup label={t("security.certificates.group")}>
          {view.device.trusted_certificates.map((certificate) => (
            <CertificateRow
              key={certificate.id}
              certificate={certificate}
              disabled={busy !== null}
              onOpen={() => setSelected(certificate)}
            />
          ))}
          <ListRowButton
            icon={<FilePlus2 size={icon.lg} />}
            title={t("security.certificates.import")}
            chevron={false}
            disabled={busy === "remove"}
            aria-busy={busy === "import"}
            trailing={busy === "import" ? <Spinner size="sm" /> : undefined}
            onClick={() => void importCertificate()}
          />
        </ListGroup>
        <Text variant="hint" className="px-1">
          {t("security.certificates.footer")}
        </Text>
      </div>
      <Sheet
        open={selected !== null}
        onClose={() => setSelected(null)}
        title={closing.shown?.name}
      >
        <ListGroup>
          <ListRowButton
            icon={<Trash2 size={icon.lg} />}
            tone="danger"
            title={t("security.certificates.remove")}
            chevron={false}
            onClick={() => closing.shown && void removeCertificate(closing.shown)}
          />
        </ListGroup>
      </Sheet>
    </>
  );
}

function CertificateRow({
  certificate,
  disabled,
  onOpen,
}: {
  certificate: TrustedCertificateView;
  disabled: boolean;
  onOpen: () => void;
}) {
  const { t, i18n } = useTranslation("settings");
  const press = useLongPress(onOpen);
  const reason = certificate.reason ?? "invalid_pem";
  const when = relativeTime(certificate.imported_at, undefined, i18n.language);
  return (
    <ListRowButton
      icon={
        certificate.valid ? <ShieldCheck size={icon.lg} /> : <ShieldAlert size={icon.lg} />
      }
      tone={certificate.valid ? "neutral" : "warning"}
      title={certificate.name}
      description={
        certificate.valid
          ? t("security.certificates.imported", {
              when: when.charAt(0).toLocaleLowerCase(i18n.language) + when.slice(1),
            })
          : t(`security.certificates.invalid.${reason}`)
      }
      chevron={false}
      disabled={disabled}
      {...press.handlers}
      onClick={press.guardClick(onOpen)}
    />
  );
}
