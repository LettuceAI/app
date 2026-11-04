import { useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import {
  AlignCenter,
  Bold,
  Clock,
  Columns3,
  Copy,
  Image,
  Info,
  Layers,
  MessageSquare,
  MoveHorizontal,
  Palette,
  RotateCcw,
  Space,
  SquareRoundCorner,
  Type,
  User,
  Users,
} from "lucide-react";
import {
  Avatar,
  AvatarGroup,
  Button,
  ColorPicker,
  Collapsible,
  Composer,
  Field,
  IconButton,
  InlineMarkdown,
  ListGroup,
  ListRow,
  Message,
  NumberInput,
  Segmented,
  Select,
  SelectMenu,
  Slider,
  Surface,
  Switch,
  Text,
} from "@lettuceai/crisp";
import { contrastRatio } from "@lettuceai/crisp";
import { cn } from "@lettuceai/crisp";
import { icon } from "@lettuceai/crisp";
import { useTheme, themeToVars } from "@lettuceai/crisp";
import backdropImage from "@/shared/assets/customization/throne-room.webp";
import seraAvatar from "@/shared/assets/customization/sera-face.webp";
import hunterAvatar from "@/shared/assets/customization/hunter-face.webp";
import { ChatAppearanceProvider } from "@/entities/preferences";
import { ParticipantsBar } from "@/features/conversations";
import { DETAILS, opts, type ChatLook, type Detail, type Opt } from "./options";
import { SettingsControlRow as Row } from "@/shared/ui/SettingsControlRow";
export { SettingsControlRow as Row } from "@/shared/ui/SettingsControlRow";

export function Pick<T extends string>({
  value,
  options,
  onChange,
  label,
  menu,
  block,
  native,
}: {
  value: T;
  options: Opt<T>;
  onChange: (value: T) => void;
  label: string;
  menu?: boolean;
  block?: boolean;
  native?: boolean;
}) {
  if (menu || options.length > 3) {
    if (native) {
      return (
        <Field label={<span className="sr-only">{label}</span>} className="w-full space-y-0">
          <Select
            className="min-h-11 w-full text-ellipsis"
            value={value}
            options={options}
            onChange={(event) => onChange(event.target.value as T)}
          />
        </Field>
      );
    }
    return (
      <Field label={<span className="sr-only">{label}</span>} className="space-y-0">
        <SelectMenu<T>
          className={block ? "w-full" : "w-40"}
          value={value}
          options={options}
          onChange={(v) => v && onChange(v)}
        />
      </Field>
    );
  }
  return (
    <Segmented<T>
      size="sm"
      block={block ?? false}
      aria-label={label}
      value={value}
      options={options}
      onChange={onChange}
    />
  );
}

type Set = <K extends keyof ChatLook>(key: K, value: ChatLook[K]) => void;

import { chatAppearance } from "@/entities/preferences";

export function PreviewBackground({ image, onChange }: { image: boolean; onChange: (image: boolean) => void }) {
  const { t } = useTranslation("customization");
  return (
    <Segmented<"plain" | "image">
      size="sm"
      aria-label={t("previewBackground")}
      value={image ? "image" : "plain"}
      onChange={(v) => onChange(v === "image")}
      options={[
        { value: "plain", label: t("plain") },
        { value: "image", label: t("backgroundImage") },
      ]}
    />
  );
}

/**
 * Without `image` the preview owns its background switch and sits above the chat; with it,
 * the caller renders the switch (a panel or screen header) and controls the background.
 */
export function ChatPreview({
  look,
  className,
  group = false,
  image,
}: {
  look: ChatLook;
  className?: string;
  group?: boolean;
  image?: boolean;
}) {
  const { t, i18n } = useTranslation("customization");
  const [ownImage, setOwnImage] = useState(false);
  const [draft, setDraft] = useState("");
  const [selected, setSelected] = useState<string | null>("sera");
  const [muted, setMuted] = useState<string[]>([]);
  const backdropOn = image ?? ownImage;
  const members = [
    { id: "sera", name: t("seraphine"), avatar: seraAvatar, characterId: "sera", muted: muted.includes("sera") },
    { id: "hunter", name: t("hunter"), avatar: hunterAvatar, characterId: "hunter", muted: muted.includes("hunter") },
  ];
  const appearance = chatAppearance(look);
  const time = { relative: t("previewRelativeTime"), time: "19:04", datetime: t("oct241904") }[
    look.timestampFormat
  ];
  const format = new Intl.NumberFormat(i18n.language, { maximumFractionDigits: 1 });
  const samples = {
    model: "Gemma 4 31B",
    input: t("metricInput", { value: format.format(3812) }),
    output: t("metricOutput", { value: format.format(412) }),
    total: t("metricTotal", { value: format.format(4224) }),
    ttft: t("metricTtft", { value: format.format(0.8) }),
    tps: t("metricSpeed", { value: format.format(38) }),
    mtp: t("metricDraft", { value: format.format(71) }),
  };
  const details =
    look.details.length > 0
      ? DETAILS.filter((d) => look.details.includes(d.value)).map((d) => (
          <span key={d.value}>{samples[d.value]}</span>
        ))
      : undefined;
  const sera = { name: t("seraphine"), avatar: seraAvatar };
  const hunter = { name: t("hunter"), avatar: hunterAvatar };
  const message = (
    key: string,
    role: "assistant" | "user",
    author: { name: string; avatar?: string },
    text: string,
    extra?: ReactNode,
  ) => (
    <Message
      key={key}
      role={role}
      author={author}
      timestamp={time}
      appearance={appearance}
      details={extra}
    >
      <InlineMarkdown quotes>{text}</InlineMarkdown>
    </Message>
  );

  return (
    <div className={cn("flex min-h-0 flex-col gap-2", className)}>
      {image === undefined && <PreviewBackground image={ownImage} onChange={setOwnImage} />}
      <Surface
        image={backdropOn ? backdropImage : null}
        dim={look.backgroundDim}
        blur={look.backgroundBlur}
        {...(look.textMode !== "auto"
          ? { tone: look.textMode === "light" ? ("dark" as const) : ("light" as const) }
          : {})}
        className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-xl border border-line bg-surface"
      >
        <div
          className={cn(
            "flex shrink-0 items-center gap-2 px-3 py-2 text-sm font-medium text-fg",
            !(backdropOn && look.transparentHeader) && "border-b border-line bg-surface-1",
          )}
        >
          {group ? (
            <AvatarGroup people={[{ name: sera.name, src: sera.avatar }, { name: hunter.name, src: hunter.avatar }]} size="sm" />
          ) : (
            <Avatar name={sera.name} src={sera.avatar} size="sm" />
          )}
          <span className="truncate">{group ? t("previewGroupName") : sera.name}</span>
        </div>
        <div className="min-h-0 flex-1 overflow-y-auto p-3">
          {message("a1", "assistant", sera, t("sheSetsTheLanternDownYouHeardThatTooRightTheOldKeyIsStillWarm"), details)}
          {group
            ? message("a2", "assistant", hunter, t("iNodSlowlyThenWeGoNow"))
            : message("u1", "user", hunter, t("iNodSlowlyThenWeGoNow"))}
          {group && message("u1", "user", { name: t("previewYou") }, t("previewGroupReply"))}
          {message("a3", "assistant", sera, t("previewFollowUp"))}
        </div>
        <div className="shrink-0 p-3">
          {group && look.participantsBarEnabled && (
            <ChatAppearanceProvider value={look}>
              <ParticipantsBar
                members={members}
                draft={draft}
                setDraft={setDraft}
                disabled={false}
                director={{ behavior: "cue", selectedId: selected, wiggle: 0 }}
                onSelect={(member) => setSelected(member.id)}
                onToggleMute={(member, value) => setMuted((current) => value ? [...current, member.id] : current.filter((id) => id !== member.id))}
              />
            </ChatAppearanceProvider>
          )}
          <Composer
            value={draft}
            onChange={setDraft}
            onSend={() => {}}
            placeholder={t(group ? "messageGroup" : "messageSeraphine")}
            fontSize={look.composerFontSize}
            {...(look.footerInputColorHex ? { color: look.footerInputColorHex } : {})}
          />
        </div>
      </Surface>
    </div>
  );
}

interface Own {
  set: boolean;
  label: string;
  resetLabel: string;
  reset: () => void;
}

function OwnMarker({ label }: { label: string }) {
  return <span role="img" aria-label={label} className="size-1.5 shrink-0 rounded-full bg-accent" />;
}

function ResetButton({ label, onClick }: { label: string; onClick: () => void }) {
  return (
    <IconButton label={label} size="sm" onClick={onClick}>
      <RotateCcw size={icon.sm} />
    </IconButton>
  );
}

function ColorRow({
  phone,
  title,
  description,
  value,
  fallback,
  onChange,
  checkContrast = true,
  own,
}: {
  phone: boolean;
  title: string;
  description?: string;
  value: string | null;
  fallback: string;
  checkContrast?: boolean;
  own?: Own;
  onChange: (value: string | null) => void;
}) {
  const { t } = useTranslation("customization");
  const { theme } = useTheme();
  const low = checkContrast && value !== null && contrastRatio(value, theme.colors.surface) < 4.5;
  const shownReset = own ? own.set : value !== null;
  return (
    <Row
      phone={phone}
      title={title}
      description={
        low
          ? t("lowContrast", { ratio: contrastRatio(value!, theme.colors.surface).toFixed(1) })
          : value
            ? value
            : own
              ? undefined
              : (description ?? t("themeDefault"))
      }
      {...(low ? { tone: "warning" as const } : {})}
      {...(own?.set ? { marker: <OwnMarker label={own.label} /> } : {})}
      {...(shownReset ? { reset: <ResetButton label={own ? own.resetLabel : t("resetColor", { title })} onClick={() => (own ? own.reset() : onChange(null))} /> } : {})}
      control={
        <Field label={<span className="sr-only">{title}</span>} className="space-y-0">
          <ColorPicker {...(phone ? { className: "w-full" } : {})} value={value ?? fallback} onChange={onChange} />
        </Field>
      }
    />
  );
}

export function ChatSettings({
  look,
  set,
  phone,
  onReset,
  group = false,
  groupView = group,
  resetField,
  overridden,
  ownLabel,
  resetLabel,
  onCopy,
}: {
  look: ChatLook;
  set: Set;
  phone: boolean;
  onReset: () => void;
  onCopy?: () => void;
  group?: boolean;
  groupView?: boolean;
  resetField?: (key: keyof ChatLook) => void;
  overridden?: (key: keyof ChatLook) => boolean;
  ownLabel?: string;
  resetLabel?: (title: string) => string;
}) {
  const { t } = useTranslation("customization");
  const { theme } = useTheme();
  const isOwn = (key: keyof ChatLook) => overridden?.(key) === true;
  const marked = (key: keyof ChatLook) => (isOwn(key) && ownLabel ? { marker: <OwnMarker label={ownLabel} /> } : {});
  const resettable = (key: keyof ChatLook, title: string) =>
    isOwn(key) && resetField ? { reset: <ResetButton label={resetLabel?.(title) ?? t("controls.reset")} onClick={() => resetField(key)} /> } : {};
  const own = (key: keyof ChatLook, title: string): Own | undefined =>
    overridden && ownLabel
      ? { set: isOwn(key), label: ownLabel, resetLabel: resetLabel?.(title) ?? t("controls.reset"), reset: () => resetField?.(key) }
      : undefined;
  const ownProp = (key: keyof ChatLook, title: string) => {
    const value = own(key, title);
    return value ? { own: value } : {};
  };
  const SIZE3 = opts([
    ["small", t("small")],
    ["medium", t("medium")],
    ["large", t("large")],
  ] as const);
  const SPACING3 = opts([
    ["tight", t("tight")],
    ["normal", t("normal")],
    ["relaxed", t("relaxed")],
  ] as const);
  const ALIGN3 = opts([
    ["left", t("left")],
    ["center", t("center")],
    ["right", t("right")],
  ] as const);
  const row = <K extends keyof ChatLook>(
    key: K,
    title: string,
    options: Opt<Extract<ChatLook[K], string>>,
    rowIcon?: ReactNode,
    description?: string,
  ) => (
    <Row
      phone={phone}
      icon={rowIcon}
      title={title}
      description={description}
      {...marked(key)}
      {...resettable(key, title)}
      control={
        <Pick
          label={title}
          native={phone}
          block={phone}
          menu={options.length > 3}
          value={look[key] as Extract<ChatLook[K], string>}
          options={options}
          onChange={(v) => set(key, v as ChatLook[K])}
        />
      }
    />
  );
  const toggle = (
    key: keyof ChatLook,
    title: string,
    description?: string,
    rowIcon?: ReactNode,
  ) => {
    const markerProps = marked(key);
    const resetProps = resettable(key, title);
    return (
      <ListRow
        icon={rowIcon}
        title={<span className="inline-flex max-w-full items-center gap-2 whitespace-normal">{title}{markerProps.marker}</span>}
        description={description}
        trailing={
          <span className="flex items-center gap-2">
            {resetProps.reset}
            <Switch
              checked={look[key] as boolean}
              onChange={(v) => set(key, v as never)}
              aria-label={title}
            />
          </span>
        }
      />
    );
  };

  return (
    <div className="space-y-7">
      <ListGroup label={t("text")}>
        {row(
          "fontSize",
          t("textSize"),
          opts([
            ["small", t("small")],
            ["medium", t("medium")],
            ["large", t("large")],
            ["xlarge", t("extraLarge")],
          ] as const),
          <Type size={icon.lg} />,
        )}
        {row("lineSpacing", t("lineSpacing"), SPACING3, <AlignCenter size={icon.lg} />)}
        {!group && row(
          "composerFontSize",
          t("composerTextSize"),
          opts([
            ["small", t("small")],
            ["medium", t("medium")],
            ["large", t("large")],
            ["xlarge", t("extraLarge")],
          ] as const),
          <Type size={icon.lg} />,
          t("theBoxYouTypeIn"),
        )}
      </ListGroup>

      <ListGroup label={t("bubbles")}>
        {row(
          "bubbleStyle",
          t("style"),
          opts([
            ["bordered", t("outline")],
            ["filled", t("filled")],
            ["minimal", t("noBubble")],
          ] as const),
          <MessageSquare size={icon.lg} />,
        )}
        {row(
          "bubbleRadius",
          t("corners"),
          opts([
            ["sharp", t("sharp")],
            ["rounded", t("rounded")],
            ["pill", t("round")],
          ] as const),
          <SquareRoundCorner size={icon.lg} />,
        )}
        {row(
          "bubbleMaxWidth",
          t("width"),
          opts([
            ["compact", t("narrow")],
            ["normal", t("normal")],
            ["wide", t("wide")],
          ] as const),
          <MoveHorizontal size={icon.lg} />,
        )}
        {row(
          "bubblePadding",
          t("padding"),
          opts([
            ["compact", t("tight")],
            ["normal", t("normal")],
            ["spacious", t("roomy")],
          ] as const),
          <Space size={icon.lg} />,
        )}
      </ListGroup>

      <ListGroup label={t("namesAndTime")}>
        {toggle(
          "showMessageAuthor",
          t("showNames"),
          t("theSpeakersNameOnEachMessage"),
          <User size={icon.lg} />,
        )}
        {toggle("showMessageTimestamp", t("showTime"), undefined, <Clock size={icon.lg} />)}
        {look.showMessageTimestamp &&
          row(
            "timestampFormat",
            t("timeFormat"),
            opts([
              ["relative", "2m ago"],
              ["time", "19:04"],
              ["datetime", t("oct241904")],
            ] as const),
          )}
        {(look.showMessageAuthor || look.showMessageTimestamp) &&
          row(
            "messageHeaderPlacement",
            t("placement"),
            opts([
              ["inside", t("inTheBubble")],
              ["above", t("aboveIt")],
              ["below", t("bottom")],
            ] as const),
          )}
      </ListGroup>

      <ListGroup label={t("replyDetails")}>
        <Row
          phone={phone}
          icon={<Info size={icon.lg} />}
          title={t("showUnderReplies")}
          description={t("modelTokenCountsAndSpeed")}
          {...marked("details")}
          {...resettable("details", t("showUnderReplies"))}
          control={
            <SelectMenu<Detail>
              multiple
              className={phone ? "w-full" : "w-48"}
              placeholder={t("none")}
              value={look.details}
              options={DETAILS.map(({ value }) => ({ value, label: t(`details.${value}`) }))}
              onChange={(v) => set("details", v)}
            />
          }
        />
        {look.details.length > 0 &&
          row(
            "messageInfoPlacement",
            t("placement"),
            opts([
              ["belowHeader", t("underTheName")],
              ["belowHeaderOutside", t("underTheNameOutside")],
              ["insideBubble", t("inTheBubble")],
              ["belowBubble", t("underTheBubble")],
            ] as const),
          )}
        {look.details.length > 0 && row("messageInfoSize", t("size"), SIZE3)}
      </ListGroup>

      <ListGroup label={t("layout")}>
        {row("messageGap", t("spaceBetweenMessages"), SPACING3, <Layers size={icon.lg} />)}
        {row(
          "avatarShape",
          t("avatars"),
          opts([
            ["circle", t("circle")],
            ["rounded", t("square")],
            ["hidden", t("hidden")],
          ] as const),
          <User size={icon.lg} />,
        )}
        {look.avatarShape !== "hidden" && row("avatarSize", t("avatarSize"), SIZE3)}
      </ListGroup>

      {groupView && <ListGroup label={t("participantsBar")}>
        {toggle("participantsBarEnabled", t("showParticipantsBar"), t("showParticipantsBarHint"), <Users size={icon.lg} />)}
        {look.participantsBarEnabled && (
          <>
            {row("participantsBarAvatarSize", t("avatarSize"), SIZE3, <User size={icon.lg} />)}
            {row(
              "participantsBarAvatarShape",
              t("barAvatarShape"),
              opts([
                ["round", t("barRound")],
                ["boxed", t("barBoxed")],
                ["rounded_box", t("barRoundedBox")],
              ] as const),
              <SquareRoundCorner size={icon.lg} />,
            )}
            {row(
              "participantsBarBackground",
              t("barBackground"),
              opts([
                ["solid", t("solid")],
                ["fading", t("barFading")],
                ["transparent", t("barTransparent")],
              ] as const),
              <Image size={icon.lg} />,
            )}
            {row("participantsBarGap", t("barSpacing"), SPACING3, <Space size={icon.lg} />)}
            {row("participantsBarAlign", t("alignment"), ALIGN3, <AlignCenter size={icon.lg} />)}
            {row(
              "participantsBarHintPosition",
              t("barHint"),
              opts([
                ["top", t("barHintTop")],
                ["bottom", t("barHintBottom")],
                ["hidden", t("hidden")],
              ] as const),
              <Info size={icon.lg} />,
            )}
          </>
        )}
      </ListGroup>}

      {!phone && (
        <ListGroup label={t("chatColumn")}>
          {row(
            "chatColumnWidth",
            t("width"),
            opts([
              ["narrow", t("narrow")],
              ["normal", t("normal")],
              ["wide", t("wide")],
              ["xl", t("extraWide")],
              ["full", t("fullWindow")],
              ["custom", t("custom")],
            ] as const),
            <Columns3 size={icon.lg} />,
          )}
          {look.chatColumnWidth === "custom" && (
            <Row
              phone={phone}
              title={t("customWidth")}
              description={t("400To2400Pixels")}
              {...marked("chatColumnWidthPx")}
              {...resettable("chatColumnWidthPx", t("customWidth"))}
              control={
                <NumberInput
                  className={phone ? "w-full" : "w-32"}
                  value={look.chatColumnWidthPx}
                  min={400}
                  max={2400}
                  step={40}
                  onChange={(v) => set("chatColumnWidthPx", v ?? 960)}
                />
              }
            />
          )}
          {look.chatColumnWidth !== "full" && row("chatColumnAlign", t("position"), ALIGN3)}
          {look.chatColumnWidth !== "full" &&
            toggle("chatHeaderMoves", t("headerFollowsTheColumn"))}
          {look.chatColumnWidth !== "full" &&
            toggle("chatFooterMoves", t("composerFollowsTheColumn"))}
          {!group && look.chatColumnWidth !== "full" &&
            toggle("chatWidgetAreaEnabled", t("widgetsBesideTheChat"), t("futureWidgets"))}
          {!group && look.chatColumnWidth !== "full" &&
            look.chatWidgetAreaEnabled &&
            row(
              "chatWidgetCenterMode",
              t("widgetSides"),
              opts([
                ["both", t("both")],
                ["left", t("left")],
                ["right", t("right")],
              ] as const),
            )}
        </ListGroup>
      )}

      <section className="space-y-2">
        <ListGroup label={t("colors")}>
          <Row
            phone={phone}
            icon={<Palette size={icon.lg} />}
            title={t("yourMessages")}
            {...marked("userBubbleColor")}
            {...resettable("userBubbleColor", t("yourMessages"))}
            control={
              <Pick
                label={t("yourMessages")}
                menu
                native={phone}
                block={phone}
                value={look.userBubbleColor}
                options={opts([
                  ["accent", t("accent")],
                  ["info", t("info")],
                  ["secondary", t("secondary")],
                  ["warning", t("warning")],
                  ["custom", t("custom")],
                ] as const)}
                onChange={(v) => set("userBubbleColor", v)}
              />
            }
          />
          {look.userBubbleColor === "custom" && (
            <ColorRow
            phone={phone}
              checkContrast={false}
              title={t("yourBubbleColor")}
              value={look.userBubbleColorHex}
              {...ownProp("userBubbleColorHex", t("yourBubbleColor"))}
              fallback={theme.colors.accent}
              onChange={(v) => set("userBubbleColorHex", v)}
            />
          )}
          <Row
            phone={phone}
            icon={<Palette size={icon.lg} />}
            title={t("characterMessages")}
            {...marked("assistantBubbleColor")}
            {...resettable("assistantBubbleColor", t("characterMessages"))}
            control={
              <Pick
                label={t("characterMessages")}
                menu
                native={phone}
                block={phone}
                value={look.assistantBubbleColor}
                options={opts([
                  ["neutral", t("neutral")],
                  ["accent", t("accent")],
                  ["info", t("info")],
                  ["secondary", t("secondary")],
                  ["custom", t("custom")],
                ] as const)}
                onChange={(v) => set("assistantBubbleColor", v)}
              />
            }
          />
          {look.assistantBubbleColor === "custom" && (
            <ColorRow
            phone={phone}
              checkContrast={false}
              title={t("characterBubbleColor")}
              value={look.assistantBubbleColorHex}
              {...ownProp("assistantBubbleColorHex", t("characterBubbleColor"))}
              fallback={themeToVars(theme)["--color-surface-1"] ?? theme.colors.surface}
              onChange={(v) => set("assistantBubbleColorHex", v)}
            />
          )}
          <ColorRow
            phone={phone}
            checkContrast={false}
            title={t("composer")}
            description={t("themeDefault")}
            value={look.footerInputColorHex}
              {...ownProp("footerInputColorHex", t("composer"))}
            fallback={themeToVars(theme)["--color-surface-el"] ?? theme.colors.surface}
            onChange={(v) => set("footerInputColorHex", v)}
          />
        </ListGroup>
        <Collapsible
          title={t("textColors")}
          description={t("narrationDialogueAndCodeCanEachHaveTheirOwnColor")}
          variant="panel"
        >
          <ListGroup>
            <ColorRow
            phone={phone}
              title={t("allMessageText")}
              value={look.messageTextColorHex}
              {...ownProp("messageTextColorHex", t("allMessageText"))}
              fallback={theme.colors.fg}
              onChange={(v) => set("messageTextColorHex", v)}
            />
            <ColorRow
            phone={phone}
              title={t("plainText")}
              value={look.plainTextColorHex}
              {...ownProp("plainTextColorHex", t("plainText"))}
              fallback={theme.colors.fg}
              onChange={(v) => set("plainTextColorHex", v)}
            />
            <ColorRow
            phone={phone}
              title={t("italicActions")}
              value={look.italicTextColorHex}
              {...ownProp("italicTextColorHex", t("italicActions"))}
              fallback={theme.colors.fg}
              onChange={(v) => set("italicTextColorHex", v)}
            />
            <ColorRow
            phone={phone}
              title={t("quotesDialogue")}
              value={look.quotedTextColorHex}
              {...ownProp("quotedTextColorHex", t("quotesDialogue"))}
              fallback={theme.colors.fg}
              onChange={(v) => set("quotedTextColorHex", v)}
            />
            <ColorRow
            phone={phone}
              title={t("inlineCode")}
              value={look.inlineCodeTextColorHex}
              {...ownProp("inlineCodeTextColorHex", t("inlineCode"))}
              fallback={theme.colors.fg}
              onChange={(v) => set("inlineCodeTextColorHex", v)}
            />
          </ListGroup>
        </Collapsible>
      </section>

      <section className="space-y-2">
        <ListGroup label={t("backgroundImage")}>
          {toggle("transparentHeader", t("seethroughHeader"), undefined, <Image size={icon.lg} />)}
          <Row
            phone={phone}
            title={t("dim")}
            {...marked("backgroundDim")}
            {...resettable("backgroundDim", t("dim"))}
            control={
              <Slider
                className={phone ? "w-full" : "w-40"}
                value={look.backgroundDim}
                min={0}
                max={80}
                onChange={(v) => set("backgroundDim", v)}
                showValue={(v) => t("percent", { value: v })}
                aria-label={t("dim")}
              />
            }
          />
          <Row
            phone={phone}
            title={t("blur")}
            {...marked("backgroundBlur")}
            {...resettable("backgroundBlur", t("blur"))}
            control={
              <Slider
                className={phone ? "w-full" : "w-40"}
                value={look.backgroundBlur}
                min={0}
                max={20}
                onChange={(v) => set("backgroundBlur", v)}
                showValue={(v) => t("pixels", { value: v })}
                aria-label={t("blur")}
              />
            }
          />
          <Row
            phone={phone}
            title={t("bubbleOpacity")}
            {...marked("bubbleOpacity")}
            {...resettable("bubbleOpacity", t("bubbleOpacity"))}
            control={
              <Slider
                className={phone ? "w-full" : "w-40"}
                value={look.bubbleOpacity}
                min={20}
                max={100}
                onChange={(v) => set("bubbleOpacity", v)}
                showValue={(v) => t("percent", { value: v })}
                aria-label={t("bubbleOpacity")}
              />
            }
          />
          {row(
            "bubbleBlur",
            t("bubbleBlur"),
            opts([
              ["none", t("none")],
              ["light", t("light")],
              ["medium", t("medium")],
              ["heavy", t("heavy")],
            ] as const),
          )}
          {row(
            "textMode",
            t("textOverTheImage"),
            opts([
              ["auto", t("automatic")],
              ["light", t("light")],
              ["dark", t("dark")],
            ] as const),
            <Bold size={icon.lg} />,
          )}
        </ListGroup>
        <Text variant="hint" className="px-1">
          {t("onlyAppliesWhenACharacterHasABackgroundImage")}
        </Text>
      </section>

      <div className="flex flex-wrap items-center justify-between gap-3 px-1">
        <Text variant="hint" className="max-w-md">
          {t("inheritanceHint")}
        </Text>
        <div className="flex flex-wrap items-center gap-1">
          {onCopy && (
            <Button variant="ghost" size="sm" leading={<Copy size={icon.sm} />} onClick={onCopy}>
              {t("copyFromNormalChat")}
            </Button>
          )}
          <Button variant="ghost" size="sm" leading={<RotateCcw size={icon.sm} />} onClick={onReset}>
            {t("resetChatLook")}
          </Button>
        </div>
      </div>
    </div>
  );
}
