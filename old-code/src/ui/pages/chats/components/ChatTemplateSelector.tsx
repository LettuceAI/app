import { MessageSquare, FileText, Star } from "lucide-react";
import { BottomMenu, MenuButton, MenuButtonGroup } from "../../../components/BottomMenu";
import type { ChatTemplate } from "../../../../core/storage/schemas";
import { useI18n } from "../../../../core/i18n/context";

interface ChatTemplateSelectorProps {
  isOpen: boolean;
  onClose: () => void;
  templates: ChatTemplate[];
  defaultTemplateId?: string | null;
  onSelect: (templateId: string | null) => void;
}

export function ChatTemplateSelector({
  isOpen,
  onClose,
  templates,
  defaultTemplateId,
  onSelect,
}: ChatTemplateSelectorProps) {
  const { t } = useI18n();
  return (
    <BottomMenu isOpen={isOpen} onClose={onClose} title={t("chats.templates.startWithTemplate")}>
      <MenuButtonGroup>
        <MenuButton
          icon={<FileText className="h-4 w-4" />}
          title={t("chats.templates.noTemplate")}
          description={t("chats.templates.startWithSceneOnly")}
          color="from-blue-500 to-cyan-600"
          onClick={() => onSelect(null)}
        />
        {templates.map((template) => {
          const preview = template.messages
            .slice(0, 2)
            .map(
              (m) =>
                `${m.role === "user" ? t("chats.templates.you") : t("chats.templates.bot")}: ${m.content.slice(0, 60)}${m.content.length > 60 ? "..." : ""}`,
            )
            .join(" | ");
          return (
            <MenuButton
              key={template.id}
              icon={<MessageSquare className="h-4 w-4" />}
              title={template.name}
              description={
                preview ||
                t("chats.templates.messageCount", { count: template.messages.length })
              }
              color="from-blue-500 to-cyan-600"
              rightElement={
                defaultTemplateId === template.id ? (
                  <Star className="h-3.5 w-3.5 fill-warning text-warning" />
                ) : undefined
              }
              onClick={() => onSelect(template.id)}
            />
          );
        })}
      </MenuButtonGroup>
    </BottomMenu>
  );
}
