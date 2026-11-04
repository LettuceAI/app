import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { AvatarGroup, cn, InlineMarkdown, MediaCard, relativeTime, useLongPress } from "@lettuceai/crisp";
import { faces } from "../data/faces";
import type { CharacterSummary, ConversationSummary, GroupView_Serialize } from "@/api/generated/bindings";


export interface GroupEntry {
  group: GroupView_Serialize;
  members: CharacterSummary[];
  chats: ConversationSummary[];
}



function useGroupCover(members: readonly CharacterSummary[]) {
  const key = JSON.stringify(members.map((member) => [member.id, member.name, member.avatar?.url]));
  const [cover, setCover] = useState<{ key: string; url: string } | null>(null);
  useEffect(() => {
    let live = true;
    let url: string | null = null;
    const cast: [string, string, string | null][] = JSON.parse(key);
    const canvas = document.createElement("canvas");
    canvas.width = 600; canvas.height = 800;
    const context = canvas.getContext("2d");
    if (!context || !cast.length) return;
    const style = getComputedStyle(document.documentElement);
    const background = style.getPropertyValue("--color-surface-2").trim();
    const foreground = style.getPropertyValue("--color-fg-2").trim();
    const draw = async () => {
      const tiles = cast.slice(0, 4);
      const images = await Promise.all(tiles.map((member) => new Promise<HTMLImageElement | null>((resolve) => {
        const src = member[2];
        if (!src) { resolve(null); return; }
        const image = new Image(); image.crossOrigin = "anonymous";
        image.onload = () => resolve(image); image.onerror = () => resolve(null); image.src = src;
      })));
      if (!live) return;
      tiles.forEach((member, index) => {
        const x = tiles.length === 1 ? 0 : tiles.length === 3 ? (index === 0 ? 0 : 300) : index % 2 * 300;
        const y = tiles.length <= 2 || (tiles.length === 3 && index === 0) ? 0 : tiles.length === 3 ? (index - 1) * 400 : Math.floor(index / 2) * 400;
        const w = tiles.length === 1 ? 600 : 300;
        const h = tiles.length <= 2 || (tiles.length === 3 && index === 0) ? 800 : 400;
        context.fillStyle = background; context.fillRect(x, y, w, h);
        const image = images[index];
        if (image) {
          const scale = Math.max(w / image.width, h / image.height);
          const sw = w / scale; const sh = h / scale;
          context.drawImage(image, (image.width - sw) / 2, (image.height - sh) / 2, sw, sh, x, y, w, h);
        }
        if (!image || (cast.length > 4 && index === 3)) {
          context.fillStyle = background; context.fillRect(x, y, w, h);
          context.fillStyle = foreground; context.font = "700 80px system-ui"; context.textAlign = "center"; context.textBaseline = "middle";
          context.fillText(cast.length > 4 && index === 3 ? `+${cast.length - 3}` : member[1]!.split(/\s+/).slice(0, 2).map((word) => word[0]).join("").toLocaleUpperCase(), x + w / 2, y + h / 2);
        }
      });
      try {
        canvas.toBlob((blob) => { if (!blob || !live) return; url = URL.createObjectURL(blob); setCover({ key, url }); }, "image/webp");
      } catch { setCover(null); }
    };
    void draw();
    return () => { live = false; if (url) URL.revokeObjectURL(url); };
  }, [key]);
  return cover?.key === key ? cover.url : null;
}

export function GroupCard({ entry, layout, compact, held, onOpen, onMenu }: { entry: GroupEntry; layout: "list" | "gallery"; compact: boolean; held: boolean; onOpen: () => void; onMenu: () => void }) {
  const { t, i18n } = useTranslation("groups");
  const press = useLongPress(onMenu);
  const cover = useGroupCover(layout === "gallery" ? entry.members : []);
  const latest = entry.chats[0];
  const count = (value: number) => new Intl.NumberFormat(i18n.language).format(value);
  const summary = [t("memberCount", { count: entry.members.length, formatted: count(entry.members.length) }), ...(entry.chats.length ? [t("chatCount", { count: entry.chats.length, formatted: count(entry.chats.length) })] : [])].join(" · ");
  const when = latest ? relativeTime(latest.updated_at, undefined, i18n.language) : undefined;
  if (layout === "gallery") return <MediaCard title={entry.group.draft.name} description={summary} image={cover} meta={when} aria-haspopup="dialog" {...press.handlers} onClick={press.guardClick(onOpen)} className={cn(held && "ring-2 ring-inset ring-fg/40")} />;
  return <button type="button" aria-haspopup="dialog" {...press.handlers} onClick={press.guardClick(onOpen)} className={cn("flex w-full items-center gap-4 rounded-2xl border border-line bg-surface-1 p-3.5 text-left transition-colors motion-instant hover:bg-fill @xl:p-4", held && "ring-2 ring-inset ring-fg/30")}>
    <AvatarGroup people={faces(entry.members)} max={3} size={compact ? "md" : "lg"} />
    <span className="min-w-0 flex-1">
      <span className="flex items-baseline justify-between gap-3">
        <span className="truncate text-base font-semibold text-fg">{entry.group.draft.name}</span>
        {when && <span className="shrink-0 text-xs tabular-nums text-fg-3">{when}</span>}
      </span>
      <span className="block truncate text-sm text-fg-3">{entry.members.length ? entry.members.map((member) => member.name).join(", ") : summary}</span>
      <span className="mt-1 line-clamp-1 wrap-anywhere text-sm text-fg-2">{latest ? latest.last_message_preview ? <InlineMarkdown>{latest.last_message_preview}</InlineMarkdown> : t("noMessages") : t("noChats")}</span>
    </span>
  </button>;
}
