import { useTranslation } from "react-i18next";
import { ArrowUpRight } from "lucide-react";
import {
  Accordion,
  AccordionItem,
  icon,
  ListGroup,
  ListRowButton,
  Page,
  Text,
} from "@lettuceai/crisp";
import { projectLinks } from "@/shared/links";
import { useSettingsLayout } from "../data";
import { useExternalLink } from "../use-external-link";

const sections = [
  {
    id: "gettingStarted",
    items: ["whatIsLettuceai", "whatIsApiKey", "cheapestStart", "alternative", "cost"],
  },
  {
    id: "privacySafety",
    items: ["apiKeySafe", "chatsStored", "chatsDeleted", "editRemotely", "shutdown", "training"],
  },
  {
    id: "modelsProviders",
    items: [
      "whatIsModel",
      "freeProviders",
      "freeVsPaid",
      "whatIsToken",
      "noDefaultModel",
      "cloudVsLocal",
    ],
  },
  {
    id: "charactersChats",
    items: ["whatIsCharacter", "whatIsPersona", "charactersPrivate", "backupMove"],
  },
] as const;

export function HelpPage() {
  const { t } = useTranslation("help");
  const { back } = useSettingsLayout();
  const externalLink = useExternalLink();
  const external = <ArrowUpRight size={icon.md} className="text-fg-4" />;

  return (
    <Page title={t("title")} {...back} contentClassName="space-y-6">
      <Text className="text-fg-3">{t("subtitle")}</Text>
      <Accordion
        defaultOpen={["whatIsLettuceai"]}
        className="space-y-6 divide-y-0 rounded-none border-0 bg-transparent"
      >
        {sections.map((section) => (
          <ListGroup key={section.id} label={t(`sections.${section.id}`)}>
            {section.items.map((item) => (
              <AccordionItem key={item} value={item} title={t(`faq.${item}.question`)}>
                <Text className="leading-relaxed text-fg-2">{t(`faq.${item}.answer`)}</Text>
              </AccordionItem>
            ))}
          </ListGroup>
        ))}
      </Accordion>
      <ListGroup label={t("stillStuck.title")}>
        <ListRowButton
          title={t("stillStuck.docsTitle")}
          description={t("stillStuck.docsDescription")}
          chevron={false}
          trailing={external}
          disabled={externalLink.isPending}
          onClick={() => externalLink.mutate(projectLinks.docs)}
        />
        <ListRowButton
          title={t("stillStuck.discordTitle")}
          description={t("stillStuck.discordDescription")}
          chevron={false}
          trailing={external}
          disabled={externalLink.isPending}
          onClick={() => externalLink.mutate(projectLinks.discord)}
        />
      </ListGroup>
    </Page>
  );
}
