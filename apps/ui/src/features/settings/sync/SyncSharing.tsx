import { useState } from "react";
import { useTranslation } from "react-i18next";
import QRCode from "react-qr-code";
import { Alert, Button, CopyButton, Field, Panel, SelectMenu, Sheet } from "@lettuceai/crisp";
import { useClosingValue } from "@/shared/hooks/useClosingValue";
import { useSyncAction, useSyncFailureText } from "./sync-data";

const qrSize = 168;

function Credential({ label, value, copyLabel, large = false }: { label: string; value: string; copyLabel: string; large?: boolean }) {
  const { t } = useTranslation("sync");
  return (
    <div className="rounded-lg border border-line bg-surface-2 px-4 py-3">
      <p className="text-sm text-fg-3">{label}</p>
      <div className="mt-1 flex items-center justify-between gap-3">
        <p className={large ? "font-mono text-2xl tracking-[0.12em] text-fg tabular-nums" : "truncate font-mono text-lg text-fg tabular-nums"}>{value}</p>
        <CopyButton value={value} variant="ghost" size="sm" label={copyLabel} copiedLabel={t("sharing.copied")} />
      </div>
    </div>
  );
}

export function Sharing({ addresses, port, pin }: { addresses: string[]; port: number; pin: string }) {
  const { t } = useTranslation("sync");
  const failureText = useSyncFailureText();
  const stop = useSyncAction();
  const [chosen, setChosen] = useState(addresses[0] ?? "");
  const address = addresses.includes(chosen) ? chosen : (addresses[0] ?? "");
  const endpoint = `${address}:${port}`;

  return (
    <Panel title={t("sharing.group")}>
      <div className="space-y-5">
        <div className="flex flex-col gap-5 sm:flex-row sm:items-start">
          <div role="img" aria-label={t("sharing.qr")} className="self-center rounded-lg bg-white p-3 sm:self-start">
            <QRCode value={JSON.stringify({ v: 1, address, port, pin })} size={qrSize} />
          </div>
          <div className="min-w-0 flex-1 space-y-4">
            {addresses.length > 1 && (
              <Field label={t("sharing.addressPicker")}>
                <SelectMenu value={address} options={addresses.map((value) => ({ value, label: value }))} onChange={(value) => value && setChosen(value)} />
              </Field>
            )}
            <div className="grid grid-cols-1 gap-3 lg:grid-cols-2">
              <Credential label={t("sharing.address")} value={endpoint} copyLabel={t("sharing.copyAddress")} />
              <Credential label={t("sharing.pin")} value={pin} copyLabel={t("sharing.copyPin")} large />
            </div>
            <p className="text-sm text-fg-3">{t("sharing.pinBody")}</p>
          </div>
        </div>
        <div className="space-y-2">
          <p className="text-sm font-medium text-fg">{t("sharing.steps")}</p>
          <ol className="list-decimal space-y-1 pl-5 text-sm text-fg-2">
            <li>{t("sharing.step1")}</li>
            <li>{t("sharing.step2")}</li>
            <li>{t("sharing.step3")}</li>
          </ol>
        </div>
        {stop.isError && <Alert tone="danger" title={failureText(stop.error)} />}
        <div className="flex justify-end border-t border-line pt-4">
          <Button variant="secondary" loading={stop.isPending} onClick={() => stop.mutate((client) => client.call("syncStop"))}>
            {t("sharing.stop")}
          </Button>
        </div>
      </div>
    </Panel>
  );
}

/** A peer that authenticated and asks to sync; closing the sheet declines. */
export function ApprovalSheet({ name }: { name: string | null }) {
  const { t } = useTranslation("sync");
  const failureText = useSyncFailureText();
  const answer = useSyncAction();
  const shown = useClosingValue(name).shown;
  const decide = (approve: boolean) => answer.mutate((client) => client.call("syncShareApprove", { approve }));
  return (
    <Sheet
      open={name !== null}
      size="sm"
      title={t("request.title")}
      dismissible={!answer.isPending}
      onClose={() => decide(false)}
      footer={
        <div className="flex gap-2">
          <Button variant="secondary" className="flex-1" disabled={answer.isPending} onClick={() => decide(false)}>{t("request.decline")}</Button>
          <Button variant="primary" className="flex-1" loading={answer.isPending} onClick={() => decide(true)}>{t("request.accept")}</Button>
        </div>
      }
    >
      <div className="space-y-3 pb-1">
        <p className="text-sm text-fg-2">{t("request.body", { name: shown || t("unknownDevice") })}</p>
        {answer.isError && <Alert tone="danger" title={failureText(answer.error)} />}
      </div>
    </Sheet>
  );
}

export function ReadyToSync({ name }: { name: string }) {
  const { t } = useTranslation("sync");
  const failureText = useSyncFailureText();
  const action = useSyncAction();
  return (
    <Panel title={t("ready.title")} description={t("ready.body", { name: name || t("unknownDevice") })}>
      {action.isError && <div className="mb-3"><Alert tone="danger" title={failureText(action.error)} /></div>}
      <div className="flex flex-wrap gap-2">
        <Button variant="primary" loading={action.isPending} onClick={() => action.mutate((client) => client.call("syncShareBegin"))}>{t("ready.start")}</Button>
        <Button variant="secondary" disabled={action.isPending} onClick={() => action.mutate((client) => client.call("syncStop"))}>{t("ready.cancel")}</Button>
      </div>
    </Panel>
  );
}
