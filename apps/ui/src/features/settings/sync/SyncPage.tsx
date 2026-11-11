import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { GitMerge } from "lucide-react";
import { icon, ListGroup, ListRowButton, Page } from "@lettuceai/crisp";
import { useSettingsApi, useSettingsLayout } from "../data";
import { SyncSession } from "./SyncSession";
import { openConflictCount, syncConflictsQuery } from "./sync-data";

export function SyncPage() {
  const { t } = useTranslation("settings");
  const { t: sync } = useTranslation("sync");
  const api = useSettingsApi();
  const { back } = useSettingsLayout();
  const navigate = useNavigate();
  const count = openConflictCount(useQuery(syncConflictsQuery(api)).data);
  return (
    <Page title={t("sections.sync.title")} {...back}>
      <div className="space-y-7">
        <SyncSession />
        {count > 0 && (
          <ListGroup>
            <ListRowButton
              icon={<GitMerge size={icon.lg} />}
              title={sync("conflicts.row")}
              description={sync("conflicts.needChoice", { count })}
              onClick={() => void navigate({ to: "/settings/sync/conflicts" })}
            />
          </ListGroup>
        )}
        <p className="text-sm text-fg-3">{sync("disclaimer")}</p>
      </div>
    </Page>
  );
}
