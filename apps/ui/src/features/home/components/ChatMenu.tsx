import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { Pencil, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Avatar, Button, confirm, icon, Input, ListGroup, ListRowButton, relativeTime, Sheet, toast } from "@lettuceai/crisp";
import type { ApiClient } from "@/api/client";
import { isApiFailure } from "@/api/client";
import type { CharacterSummary, ConversationSummary } from "@/api/generated/bindings";
import { deleteChat, renameChat } from "../data/actions";

export interface ChatMenuTarget {
  chat: ConversationSummary;
  character: CharacterSummary | null;
}

/** The menu a long press or right-click opens on a chat: rename it or delete it. */
export function ChatMenu({ api, target, onClose }: { api: ApiClient; target: ChatMenuTarget | null; onClose: () => void }) {
  const { t, i18n } = useTranslation("home");
  const { t: common } = useTranslation("common");
  const queryClient = useQueryClient();
  const services = { api, queryClient };
  const [shown, setShown] = useState<ChatMenuTarget | null>(target);
  const [renaming, setRenaming] = useState<ChatMenuTarget | null>(null);
  const [title, setTitle] = useState("");
  const [busy, setBusy] = useState(false);
  if (target && target !== shown) setShown(target);

  const run = async (task: () => Promise<void>) => {
    if (busy) return;
    setBusy(true);
    try {
      await task();
    } catch (error) {
      toast.error(common(`errors.code.${isApiFailure(error) ? error.code : "internal"}`));
    } finally {
      setBusy(false);
    }
  };

  const name = shown ? (shown.character?.name ?? shown.chat.title) : "";
  const save = () => {
    const current = renaming;
    if (!current || !title.trim()) return;
    void run(async () => {
      await renameChat(services, current.chat, title);
      setRenaming(null);
    });
  };

  return (
    <>
      <Sheet
        open={Boolean(target)}
        onClose={onClose}
        size="sm"
        title={shown?.chat.title}
        description={shown && `${name} · ${relativeTime(shown.chat.updated_at, undefined, i18n.language).toLowerCase()}`}
        leftAction={shown && <Avatar src={shown.character?.avatar?.url ?? shown.chat.avatars[0]?.url ?? null} name={name} size="md" />}
      >
        {shown && (
          <div className="space-y-4 pb-1">
            <ListGroup>
              <ListRowButton
                icon={<Pencil size={icon.lg} />}
                title={t("chatMenu.rename")}
                onClick={() => {
                  setTitle(shown.chat.title);
                  setRenaming(shown);
                  onClose();
                }}
              />
            </ListGroup>
            <ListGroup>
              <ListRowButton
                icon={<Trash2 size={icon.lg} />}
                tone="danger"
                title={<span className="text-danger">{t("chatMenu.delete")}</span>}
                description={t("chatMenu.deleteBody")}
                chevron={false}
                disabled={busy}
                onClick={() => {
                  const chat = shown.chat;
                  void (async () => {
                    const sure = await confirm({
                      title: t("chatMenu.deleteTitle", { title: chat.title }),
                      message: t("chatMenu.deleteBody"),
                      confirmLabel: t("chatMenu.delete"),
                      cancelLabel: common("actions.cancel"),
                      destructive: true,
                    });
                    if (!sure) return;
                    await run(async () => {
                      await deleteChat(services, chat);
                      onClose();
                    });
                  })();
                }}
              />
            </ListGroup>
          </div>
        )}
      </Sheet>
      <Sheet
        open={Boolean(renaming)}
        onClose={() => setRenaming(null)}
        size="sm"
        title={t("chatMenu.renameTitle")}
        footer={
          <div className="flex gap-2">
            <Button variant="secondary" className="flex-1" onClick={() => setRenaming(null)}>{common("actions.cancel")}</Button>
            <Button variant="primary" className="flex-1" disabled={busy || !title.trim()} onClick={save}>{t("chatMenu.save")}</Button>
          </div>
        }
      >
        <Input
          value={title}
          onChange={(event) => setTitle(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter") save();
          }}
          aria-label={t("chatMenu.renameTitle")}
          data-autofocus
        />
      </Sheet>
    </>
  );
}
