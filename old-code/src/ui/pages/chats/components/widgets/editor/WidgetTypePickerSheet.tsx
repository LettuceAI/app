import {
  ArrowLeftRight,
  Box as BoxIcon,
  Brain,
  Clock,
  Dices,
  FileText,
  Gauge,
  Image as ImageIcon,
  Info,
  ListChecks,
  Minus,
  NotebookPen,
  Sparkles,
  User,
  UserCircle,
  Zap,
  BookOpen,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { BottomMenu, MenuButton } from "../../../../../components";
import { useI18n } from "../../../../../../core/i18n/context";
import { openDocs } from "../../../../../../core/utils/docs";
import { useWidgetAvailability } from "../WidgetContext";
import {
  isWidgetTypeAvailable,
  WIDGET_TYPE_DESC,
  WIDGET_TYPE_LABEL,
  type WidgetType,
} from "./widgetFactories";

const TYPE_ORDER: WidgetType[] = [
  "character_info",
  "persona_info",
  "companion_state",
  "time",
  "memory",
  "stat_tracker",
  "scratch_pad",
  "author_note",
  "quick_snippets",
  "dice",
  "session_info",
  "selector",
  "button",
  "image",
  "box",
  "divider",
];

const TYPE_ICON: Record<WidgetType, LucideIcon> = {
  divider: Minus,
  box: BoxIcon,
  character_info: User,
  persona_info: UserCircle,
  scratch_pad: NotebookPen,
  image: ImageIcon,
  selector: ListChecks,
  button: ArrowLeftRight,
  stat_tracker: Gauge,
  quick_snippets: Zap,
  dice: Dices,
  memory: Brain,
  companion_state: Sparkles,
  session_info: Info,
  author_note: FileText,
  time: Clock,
};

interface WidgetTypePickerSheetProps {
  open: boolean;
  onClose: () => void;
  onPick: (type: WidgetType) => void;
}

export function WidgetTypePickerSheet({
  open,
  onClose,
  onPick,
}: WidgetTypePickerSheetProps) {
  const { t } = useI18n();
  const availability = useWidgetAvailability();
  const types = TYPE_ORDER.filter((type) => isWidgetTypeAvailable(type, availability));
  return (
    <BottomMenu isOpen={open} onClose={onClose} title={t("chats.widgets.picker.title")}>
      <div className="flex flex-col gap-2">
        {types.map((type) => (
          <MenuButton
            key={type}
            icon={TYPE_ICON[type]}
            title={t(WIDGET_TYPE_LABEL[type])}
            description={t(WIDGET_TYPE_DESC[type])}
            onClick={() => {
              onPick(type);
              onClose();
            }}
          />
        ))}
        <MenuButton
          icon={BookOpen}
          title={t("chats.widgets.picker.learnMore")}
          description={t("chats.widgets.picker.learnMoreDesc")}
          onClick={() => void openDocs("chatWidgets")}
        />
      </div>
    </BottomMenu>
  );
}
