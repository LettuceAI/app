import { useState } from "react";
import { useMutation, useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { ScanLine } from "lucide-react";
import { Alert, Button, ChoiceCard, Field, icon, Input, OtpInput, Panel } from "@lettuceai/crisp";
import { appStatusQuery } from "@/api/app-status-query";
import { scanQrCode } from "@/api/barcode-scanner";
import { useSettingsApi } from "../data";
import { isPin, parseEndpoint, parseSyncCode, pinLength, type SyncEndpoint, useSyncAction, useSyncFailureText } from "./sync-data";

type Mode = "join" | "host";

function JoinForm() {
  const { t } = useTranslation("sync");
  const api = useSettingsApi();
  const platform = useQuery(appStatusQuery(api)).data?.platform;
  const failureText = useSyncFailureText();
  const connection = useSyncAction();
  const [address, setAddress] = useState("");
  const [pin, setPin] = useState("");
  const [touched, setTouched] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const endpoint = parseEndpoint(address);

  const connect = (target: SyncEndpoint & { pin: string }) => connection.mutate((client) => client.call("syncConnect", target));
  const scan = useMutation({
    mutationFn: () => scanQrCode(platform),
    onMutate: () => setNotice(null),
    onSuccess: (result) => {
      if (result.status === "denied") return setNotice(t("join.scanDenied"));
      if (result.status === "cancelled") return;
      const code = parseSyncCode(result.content);
      if (!code) return setNotice(t("join.scanInvalid"));
      setAddress(`${code.address}:${code.port}`);
      setPin(code.pin);
      connect(code);
    },
    onError: () => setNotice(t("join.scanUnavailable")),
  });
  const mobile = platform === "android" || platform === "ios";
  const busy = connection.isPending || scan.isPending;
  const message = notice ?? (connection.isError ? failureText(connection.error) : null);

  return (
    <Panel>
      <form
        className="space-y-5"
        onSubmit={(event) => {
          event.preventDefault();
          if (!endpoint || !isPin(pin) || busy) return;
          setNotice(null);
          connect({ ...endpoint, pin });
        }}
      >
        <div className="grid grid-cols-1 items-start gap-5 md:grid-cols-[minmax(0,1fr)_auto]">
          <Field
            label={t("join.address")}
            hint={t("join.addressHint")}
            {...(touched && address.trim() !== "" && !endpoint ? { error: t("join.addressInvalid") } : {})}
          >
            <Input
              value={address}
              placeholder={t("join.addressPlaceholder")}
              autoComplete="off"
              autoCapitalize="off"
              spellCheck={false}
              onChange={(event) => setAddress(event.target.value)}
              onBlur={() => setTouched(true)}
            />
          </Field>
          <Field label={t("join.pin")}>
            <OtpInput length={pinLength} value={pin} onChange={setPin} />
          </Field>
        </div>
        {message && <Alert tone="danger" title={message} />}
        <div className="flex flex-col-reverse gap-2 border-t border-line pt-4 sm:flex-row sm:justify-end">
          {mobile && (
            <Button type="button" variant="secondary" leading={<ScanLine size={icon.md} />} loading={scan.isPending} disabled={busy} onClick={() => scan.mutate()}>
              {t("join.scan")}
            </Button>
          )}
          <Button type="submit" variant="primary" loading={connection.isPending} disabled={!endpoint || !isPin(pin) || busy}>
            {t("join.connect")}
          </Button>
        </div>
      </form>
    </Panel>
  );
}

function HostPanel() {
  const { t } = useTranslation("sync");
  const failureText = useSyncFailureText();
  const start = useSyncAction();
  return (
    <Panel>
      <div className="space-y-5">
        <div className="space-y-1 text-sm text-fg-2">
          <p>{t("host.body1")}</p>
          <p>{t("host.body2")}</p>
        </div>
        {start.isError && <Alert tone="danger" title={failureText(start.error)} />}
        <div className="flex justify-end border-t border-line pt-4">
          <Button variant="primary" loading={start.isPending} onClick={() => start.mutate((client) => client.call("syncShareStart"))}>
            {t("host.start")}
          </Button>
        </div>
      </div>
    </Panel>
  );
}

/** The idle session: pick whether this device joins or hosts, then do it. `joinOnly` drops the host choice. */
export function SyncStart({ joinOnly = false }: { joinOnly?: boolean }) {
  const { t } = useTranslation("sync");
  const [mode, setMode] = useState<Mode>("join");
  return (
    <div className="space-y-5">
      {!joinOnly && (
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
          <ChoiceCard selected={mode === "join"} indicator="radio" title={t("mode.join.title")} description={t("mode.join.body")} onClick={() => setMode("join")} />
          <ChoiceCard selected={mode === "host"} indicator="radio" title={t("mode.host.title")} description={t("mode.host.body")} onClick={() => setMode("host")} />
        </div>
      )}
      {joinOnly || mode === "join" ? <JoinForm /> : <HostPanel />}
    </div>
  );
}
