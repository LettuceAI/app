import { useState } from "react";
import { FileUp } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button, icon, ListGroup, ListRow, Switch } from "@lettuceai/crisp";
import { IdentityFields } from "./IdentityFields";
import type { StepProps } from "./step-props";

export function IdentityStep({ api, form, update, onImport, importing }: StepProps & { onImport: () => void; importing: boolean }) {
  const { t } = useTranslation("personas");
  const [picking, setPicking] = useState(false);

  return <div className="grid grid-cols-1 gap-6 @4xl:grid-cols-2 @4xl:gap-x-10">
    <div className="space-y-6">
      <IdentityFields api={api} form={form} update={update} disabled={importing} autoFocus onBusyChange={setPicking} />
    </div>
    <div className="space-y-6">
      <ListGroup>
        <ListRow title={t("identity.default")} description={t("identity.defaultBody")} trailing={<Switch checked={form.makeDefault} aria-label={t("identity.default")} onChange={(makeDefault) => update((current) => ({ ...current, makeDefault }))} />} />
      </ListGroup>
      <ListGroup>
        <ListRow icon={<FileUp size={icon.lg} />} title={t("identity.import")} description={t("identity.importBody")} trailing={<Button size="sm" variant="tonal" disabled={importing || picking} onClick={onImport}>{t("identity.chooseFile")}</Button>} />
      </ListGroup>
    </div>
  </div>;
}
