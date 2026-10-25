import { useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import {
  AlignCenter,
  Bold,
  Clock,
  Columns3,
  Eye,
  Image,
  Info,
  Layers,
  MessageSquare,
  Palette,
  RotateCcw,
  Type,
  User,
  Users,
} from "lucide-react";
import {
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
}: {
  value: T;
  options: Opt<T>;
  onChange: (value: T) => void;
  label: string;
  menu?: boolean;
  block?: boolean;
}) {
  if (menu || options.length > 3) {
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

export function ChatPreview({ look, className }: { look: ChatLook; className?: string }) {
  const { t, i18n } = useTranslation("customization");
  const [backdropOn, setBackdropOn] = useState(false);
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

  return (
    <div className={cn("space-y-2", className)}>
      <Segmented<"plain" | "image">
        size="sm"
        block
        aria-label={t("previewBackground")}
        value={backdropOn ? "image" : "plain"}
        onChange={(v) => setBackdropOn(v === "image")}
        options={[
          { value: "plain", label: t("plain") },
          { value: "image", label: t("backgroundImage") },
        ]}
      />
      <Surface
        image={backdropOn ? backdropImage : null}
        dim={look.backgroundDim}
        blur={look.backgroundBlur}
        {...(look.textMode !== "auto"
          ? { tone: look.textMode === "light" ? ("dark" as const) : ("light" as const) }
          : {})}
        className="overflow-hidden rounded-xl border border-line bg-surface"
      >
        <div
          className={cn(
            "px-3 py-2 text-sm font-medium text-fg",
            !(backdropOn && look.transparentHeader) && "border-b border-line bg-surface-1",
          )}
        >
          {t("seraphine")}
        </div>
        <div className="p-3">
          <Message
            role="assistant"
            author={{ name: t("seraphine"), avatar: seraAvatar }}
            timestamp={time}
            appearance={appearance}
            details={details}
          >
            <InlineMarkdown quotes>
              {t("sheSetsTheLanternDownYouHeardThatTooRightTheOldKeyIsStillWarm")}
            </InlineMarkdown>
          </Message>
          <Message
            role="user"
            author={{ name: t("hunter"), avatar: hunterAvatar }}
            timestamp={time}
            appearance={appearance}
          >
            <InlineMarkdown quotes>{t("iNodSlowlyThenWeGoNow")}</InlineMarkdown>
          </Message>
          <Composer
            className="mt-4"
            value=""
            onChange={() => {}}
            onSend={() => {}}
            placeholder={t("messageSeraphine")}
            fontSize={look.composerFontSize}
            {...(look.footerInputColorHex ? { color: look.footerInputColorHex } : {})}
          />
        </div>
      </Surface>
    </div>
  );
}

function ColorRow({
  title,
  description,
  value,
  fallback,
  onChange,
  checkContrast = true,
}: {
  title: string;
  description?: string;
  value: string | null;
  fallback: string;
  checkContrast?: boolean;
  onChange: (value: string | null) => void;
}) {
  const { t } = useTranslation("customization");
  const { theme } = useTheme();
  const low = checkContrast && value !== null && contrastRatio(value, theme.colors.surface) < 4.5;
  return (
    <ListRow
      title={title}
      description={
        low
          ? t("lowContrast", { ratio: contrastRatio(value!, theme.colors.surface).toFixed(1) })
          : value
            ? value
            : (description ?? t("themeDefault"))
      }
      {...(low ? { tone: "warning" as const } : {})}
      trailing={
        <span className="flex items-center gap-1">
          {value && (
            <IconButton label={t("resetColor", { title })} size="sm" onClick={() => onChange(null)}>
              <RotateCcw size={icon.sm} />
            </IconButton>
          )}
          <Field label={<span className="sr-only">{title}</span>} className="space-y-0">
            <ColorPicker value={value ?? fallback} onChange={onChange} />
          </Field>
        </span>
      }
    />
  );
}

export function ChatSettings({
  look,
  set,
  phone,
  onReset,
}: {
  look: ChatLook;
  set: Set;
  phone: boolean;
  onReset: () => void;
}) {
  const { t } = useTranslation("customization");
  const { theme } = useTheme();
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
      control={
        <Pick
          label={title}
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
  ) => (
    <ListRow
      icon={rowIcon}
      title={title}
      description={description}
      trailing={
        <Switch
          checked={look[key] as boolean}
          onChange={(v) => set(key, v as never)}
          aria-label={title}
        />
      }
    />
  );

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
        {row(
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
        )}
        {row(
          "bubbleMaxWidth",
          t("width"),
          opts([
            ["compact", t("narrow")],
            ["normal", t("normal")],
            ["wide", t("wide")],
          ] as const),
        )}
        {row(
          "bubblePadding",
          t("padding"),
          opts([
            ["compact", t("tight")],
            ["normal", t("normal")],
            ["spacious", t("roomy")],
          ] as const),
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
            <ListRow
              title={t("customWidth")}
              description={t("400To2400Pixels")}
              trailing={
                <NumberInput
                  className="w-32"
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
          {look.chatColumnWidth !== "full" &&
            toggle("chatWidgetAreaEnabled", t("widgetsBesideTheChat"), t("futureWidgets"))}
          {look.chatColumnWidth !== "full" &&
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
            control={
              <Pick
                label={t("yourMessages")}
                menu
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
              checkContrast={false}
              title={t("yourBubbleColor")}
              value={look.userBubbleColorHex}
              fallback={theme.colors.accent}
              onChange={(v) => set("userBubbleColorHex", v)}
            />
          )}
          <Row
            phone={phone}
            icon={<Palette size={icon.lg} />}
            title={t("characterMessages")}
            control={
              <Pick
                label={t("characterMessages")}
                menu
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
              checkContrast={false}
              title={t("characterBubbleColor")}
              value={look.assistantBubbleColorHex}
              fallback={themeToVars(theme)["--color-surface-1"] ?? theme.colors.surface}
              onChange={(v) => set("assistantBubbleColorHex", v)}
            />
          )}
          <ColorRow
            checkContrast={false}
            title={t("composer")}
            description={t("themeDefault")}
            value={look.footerInputColorHex}
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
              title={t("allMessageText")}
              value={look.messageTextColorHex}
              fallback={theme.colors.fg}
              onChange={(v) => set("messageTextColorHex", v)}
            />
            <ColorRow
              title={t("plainText")}
              value={look.plainTextColorHex}
              fallback={theme.colors.fg}
              onChange={(v) => set("plainTextColorHex", v)}
            />
            <ColorRow
              title={t("italicActions")}
              value={look.italicTextColorHex}
              fallback={theme.colors.fg}
              onChange={(v) => set("italicTextColorHex", v)}
            />
            <ColorRow
              title={t("quotesDialogue")}
              value={look.quotedTextColorHex}
              fallback={theme.colors.fg}
              onChange={(v) => set("quotedTextColorHex", v)}
            />
            <ColorRow
              title={t("inlineCode")}
              value={look.inlineCodeTextColorHex}
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

      <Collapsible
        title={t("groupChats")}
        description={t("futureGroup")}
        variant="panel"
        trailing={
          <Switch
            checked={look.participantsBarEnabled}
            onChange={(v) => set("participantsBarEnabled", v)}
            aria-label={t("showParticipantsBar")}
          />
        }
      >
        {look.participantsBarEnabled ? (
          <ListGroup>
            {row("participantsBarAvatarSize", t("avatarSize"), SIZE3, <Users size={icon.lg} />)}
            {row(
              "participantsBarAvatarShape",
              t("avatarShape"),
              opts([
                ["round", t("circle")],
                ["rounded_box", t("rounded")],
                ["boxed", t("square")],
              ] as const),
            )}
            {row(
              "participantsBarBackground",
              t("background"),
              opts([
                ["solid", t("solid")],
                ["fading", t("fade")],
                ["transparent", t("none")],
              ] as const),
            )}
            {row("participantsBarGap", t("spacing"), SPACING3)}
            {row("participantsBarAlign", t("alignment"), ALIGN3)}
            {row(
              "participantsBarHintPosition",
              t("hint"),
              opts([
                ["top", t("above")],
                ["bottom", t("below")],
                ["hidden", t("hidden")],
              ] as const),
              <Eye size={icon.lg} />,
            )}
          </ListGroup>
        ) : (
          <Text variant="hint">{t("theBarIsHiddenInGroupChats")}</Text>
        )}
      </Collapsible>

      <div className="flex flex-wrap items-center justify-between gap-3 px-1">
        <Text variant="hint" className="max-w-md">
          {t("inheritanceHint")}
        </Text>
        <Button variant="ghost" size="sm" leading={<RotateCcw size={icon.sm} />} onClick={onReset}>
          {t("resetChatLook")}
        </Button>
      </div>
    </div>
  );
}
