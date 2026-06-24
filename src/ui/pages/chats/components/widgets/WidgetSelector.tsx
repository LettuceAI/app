import { useState, type ReactNode } from "react";
import { ChevronRight, Cpu, NotebookPen, User } from "lucide-react";
import type { SelectorNode } from "../../../../../core/storage/chatWidgetSchemas";
import { cn, interactive, radius, typography } from "../../../../design-tokens";
import { useI18n, type TranslationKey } from "../../../../../core/i18n/context";
import { AvatarImage } from "../../../../components/AvatarImage";
import { ModelSelectionBottomMenu } from "../../../../components/ModelSelectionBottomMenu";
import { useAvatar } from "../../../../hooks/useAvatar";
import type { WidgetDesign } from "../../../../../core/storage/chatWidgetSchemas";
import { PersonaSelector } from "../../../group-chats/components/settings";
import { AuthorNoteBottomMenu } from "../AuthorNoteBottomMenu";
import { useWidgetContext } from "./WidgetContext";
import { widgetCardClass } from "./widgetSurface";

interface WidgetSelectorProps {
  node: SelectorNode;
}

const DEFAULT_LABEL = {
  persona: "chats.widgets.selector.persona",
  model: "chats.widgets.selector.model",
  author_note: "chats.widgets.selector.authorNote",
} satisfies Record<SelectorNode["kind"], TranslationKey>;

export function WidgetSelector({ node }: WidgetSelectorProps) {
  const { t } = useI18n();
  const ctx = useWidgetContext();
  const [open, setOpen] = useState(false);
  const label = node.title ?? t(DEFAULT_LABEL[node.kind]);

  if (node.kind === "persona") {
    return (
      <PersonaSelectorWidget
        label={label}
        open={open}
        setOpen={setOpen}
        design={node.design}
      />
    );
  }

  if (node.kind === "author_note") {
    const note = ctx.session?.authorNote?.trim();
    return (
      <>
        <ChipRow
          icon={<NotebookPen className="h-4 w-4" />}
          label={label}
          value={note ? note : t("chats.widgets.selector.addNote")}
          onClick={() => setOpen(true)}
          design={node.design}
        />
        <AuthorNoteBottomMenu
          isOpen={open}
          onClose={() => setOpen(false)}
          session={ctx.session}
          onSaved={ctx.onAuthorNoteSaved}
        />
      </>
    );
  }

  const currentId = ctx.currentModelId;
  const current = ctx.models.find((m) => m.id === currentId);
  const value =
    current?.displayName ?? current?.name ?? t("chats.widgets.selector.appDefault");
  const onSelect = ctx.onSelectModel;

  return (
    <>
      <ChipRow
        icon={<Cpu className="h-4 w-4" />}
        label={label}
        value={value}
        onClick={() => setOpen(true)}
        design={node.design}
      />
      <ModelSelectionBottomMenu
        isOpen={open}
        onClose={() => setOpen(false)}
        title={label}
        models={ctx.models}
        selectedModelIds={currentId ? [currentId] : []}
        searchPlaceholder={t("chats.widgets.selector.searchModels")}
        theme="dark"
        tone="emerald"
        includeExitIcon={false}
        location="bottom"
        onSelectModel={(modelId) => {
          void onSelect(modelId);
          setOpen(false);
        }}
        clearOption={{
          label: t("chats.widgets.selector.useGlobalDefault"),
          icon: Cpu,
          selected: !currentId,
          onClick: () => {
            void onSelect(null);
            setOpen(false);
          },
        }}
      />
    </>
  );
}

function PersonaSelectorWidget({
  label,
  open,
  setOpen,
  design,
}: {
  label: string;
  open: boolean;
  setOpen: (v: boolean) => void;
  design?: WidgetDesign;
}) {
  const { t } = useI18n();
  const ctx = useWidgetContext();
  const session = ctx.session;
  const personas = ctx.personas;

  const selectedPersonaId = (() => {
    if (!session) return undefined;
    if (session.personaDisabled || session.personaId === "") return "";
    if (session.personaId) return session.personaId;
    return personas.find((p) => p.isDefault)?.id;
  })();

  const personaForAvatar =
    selectedPersonaId && selectedPersonaId !== ""
      ? (personas.find((p) => p.id === selectedPersonaId) ?? null)
      : null;
  const avatarUrl = useAvatar(
    "persona",
    personaForAvatar?.id ?? "",
    personaForAvatar?.avatarPath,
    "round",
  );

  const value = (() => {
    if (!session) return t("chats.widgets.selector.openSessionFirst");
    if (session.personaDisabled || session.personaId === "") return t("common.labels.none");
    if (!session.personaId) {
      const def = personas.find((p) => p.isDefault);
      if (!def) return t("common.labels.none");
      return def.nickname
        ? t("chats.widgets.selector.personaWithNicknameDefault", {
            title: def.title,
            nickname: def.nickname,
          })
        : t("chats.widgets.selector.personaDefault", { title: def.title });
    }
    const p = personas.find((p) => p.id === session.personaId);
    if (!p) return t("chats.widgets.selector.customPersona");
    return p.nickname ? `${p.title} (${p.nickname})` : p.title;
  })();

  return (
    <>
      <ChipRow
        icon={
          avatarUrl ? (
            <div className="h-full w-full overflow-hidden rounded-full">
              <AvatarImage
                src={avatarUrl}
                alt={personaForAvatar?.title ?? t("chats.widgets.selector.persona")}
                crop={personaForAvatar?.avatarCrop}
                applyCrop
              />
            </div>
          ) : (
            <User className="h-4 w-4" />
          )
        }
        label={label}
        value={value}
        onClick={() => setOpen(true)}
        design={design}
      />
      <PersonaSelector
        isOpen={open}
        onClose={() => setOpen(false)}
        personas={personas}
        selectedPersonaId={selectedPersonaId}
        onSelect={(personaId) => {
          void ctx.onSelectPersona(personaId);
          setOpen(false);
        }}
      />
    </>
  );
}

interface ChipRowProps {
  icon: ReactNode;
  label: string;
  value: string;
  onClick: () => void;
  design?: WidgetDesign;
}

function ChipRow({ icon, label, value, onClick, design }: ChipRowProps) {
  const { hasBackground } = useWidgetContext();
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "group flex min-h-14 w-full items-center justify-between",
        radius.md,
        "p-4 text-left",
        widgetCardClass(hasBackground, design),
        interactive.transition.default,
        interactive.active.scale,
        "hover:border-fg/20 hover:bg-fg/6",
      )}
    >
      <div className="flex min-w-0 items-center gap-3">
        <div
          className={cn(
            "flex h-10 w-10 items-center justify-center overflow-hidden",
            radius.full,
            "border border-fg/15 bg-fg/8 text-fg/80",
          )}
        >
          {icon}
        </div>
        <div className="min-w-0 flex-1">
          <div
            className={cn(
              typography.overline.size,
              typography.overline.weight,
              typography.overline.tracking,
              typography.overline.transform,
              "text-fg/50",
            )}
          >
            {label}
          </div>
          <div className={cn(typography.bodySmall.size, "truncate text-fg")}>{value}</div>
        </div>
      </div>
      <ChevronRight className="h-4 w-4 text-fg/40 transition-colors group-hover:text-fg/80" />
    </button>
  );
}
