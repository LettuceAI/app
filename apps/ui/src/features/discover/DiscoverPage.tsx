import { useMutation } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { Compass, MessagesSquare, Plus } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button, EmptyState, icon, Page } from "@lettuceai/crisp";
import { openExternalUrl } from "@/api/external-links";
import { projectLinks } from "@/shared/links";
import { MobileSettingsButton } from "@/shared/ui/MobileSettingsButton";

export function DiscoverPage() {
  const { t } = useTranslation("home");
  const navigate = useNavigate();
  const discord = useMutation({ mutationFn: () => openExternalUrl(projectLinks.discord) });
  return (
    <Page title={t("discover.title")} actions={<MobileSettingsButton />}>
      <EmptyState
        icon={<Compass size={icon.xl} />}
        title={t("discover.soonTitle")}
        description={t("discover.soonBody")}
        className="pt-16"
        action={
          <div className="flex flex-wrap justify-center gap-2">
            <Button variant="primary" leading={<Plus size={icon.md} />} onClick={() => void navigate({ to: "/characters/new" })}>
              {t("discover.create")}
            </Button>
            <Button variant="secondary" leading={<MessagesSquare size={icon.md} />} loading={discord.isPending} onClick={() => discord.mutate()}>
              {t("discover.discord")}
            </Button>
          </div>
        }
      />
    </Page>
  );
}
