import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  Archive, ArchiveRestore, BookOpen, CalendarClock, Check, Copy, Cpu, Download, History, Image as ImageIcon, MessageSquarePlus, Pencil,
  NotebookPen, Sparkles, SlidersHorizontal, Trash2, Upload, User, GitBranch,
} from "lucide-react";
import { useTranslation } from "react-i18next";
import { Alert, Avatar, Button, confirm, icon, IconButton, Input, ListGroup, ListRow, ListRowButton, LoadingState, Switch, toast } from "@lettuceai/crisp";
import { overrideCount } from "@/entities/model/model-config";
import { queryKeys } from "@/api/query-keys";
import { startDirectChat } from "../../data/actions";
import { duplicateChat, exportChat, importChatLog, removeChat, renameChat, setArchived } from "../../data/chat-actions";
import { memoryQuery, modelsQuery, personasQuery } from "../../data/queries";
import { characterChatsQuery, characterQuery, effectivePersonaId, lorebooksQuery, messageCountQuery } from "../../data/settings-queries";
import { useChatModels } from "../../data/use-models";
import { formatTime } from "../../data/messages";
import type { PanelCtx } from "./context";
import { SettingRow } from "./SettingRow";

export interface SettingsRootProps {
  ctx: PanelCtx;
  name: string;
  avatar: string | null;
  onOpenMemories: () => void;
  onBranches: () => void;
  onModelsSettings: () => void;
  onHome: () => void;
}

export function SettingsRoot({ ctx, name, avatar, onOpenMemories, onBranches, onModelsSettings, onHome }: SettingsRootProps) {
  const { t, i18n } = useTranslation("conversations");
  const { api, queryClient, conversationId, view, settings } = ctx;
  const character = useQuery(characterQuery(api, ctx.characterId));
  const count = useQuery(messageCountQuery(api, conversationId));
  const chats = useQuery(characterChatsQuery(api, ctx.characterId));
  const personas = useQuery({ ...personasQuery(api, conversationId), enabled: settings !== undefined });
  const lorebooks = useQuery({ ...lorebooksQuery(api, conversationId), enabled: settings !== undefined });
  const chat = useChatModels(api);
  const modelsStatus = useQuery(modelsQuery(api));
  const memoryOn = settings !== undefined && settings.memory.mode !== "disabled";
  const memory = useQuery({ ...memoryQuery(api, conversationId), enabled: memoryOn });
  const [renaming, setRenaming] = useState(false);
  const [title, setTitle] = useState("");

  if (!settings) return <LoadingState />;

  const personaId = effectivePersonaId(settings.persona, personas.data?.defaultId ?? null);
  const personaValue = personas.isPending ? t("settings.loading") : personas.isError ? t("settings.loadFailed") : personaId ? (personas.data.items.find((entry) => entry.id === personaId)?.title ?? t("persona.none")) : t("persona.none");
  const model = chat.find(settings.model.id);
  const modelValue = !settings.model.id
    ? t("settings.model.none")
    : modelsStatus.isPending
      ? t("settings.loading")
      : modelsStatus.isError
        ? t("settings.loadFailed")
        : (model?.name ?? t("settings.model.unknown"));
  const overrides = overrideCount(settings.model_settings);
  const companion = character.data?.draft.defaults.interaction_mode === "companion";
  const starters = character.data?.draft.starters.length ?? 0;
  const lastActive = chats.data?.find((entry) => entry.id === conversationId)?.updated_at;
  const meta = [count.data !== undefined ? t("settings.meta.messages", { count: count.data }) : null, lastActive ? t("settings.meta.active", { date: formatTime(lastActive, i18n.language) }) : null].filter(Boolean).join(" · ");
  const saveTitle = async () => {
    const next = title.trim();
    if (!next || next === view.title) {
      setRenaming(false);
      return;
    }
    const done = await ctx.run(() => renameChat(api, queryClient, conversationId, view.revision, next).then(() => true));
    if (done) setRenaming(false);
  };
  const startChat = async () => {
    if (!ctx.characterId) return;
    if (starters > 0) {
      ctx.open("starters");
      return;
    }
    const id = await ctx.run(() => startDirectChat(api, queryClient, ctx.characterId ?? ""));
    if (id) ctx.goTo(id, true);
  };
  const duplicate = async () => {
    const id = await ctx.run(() => duplicateChat(api, queryClient, conversationId));
    if (id) {
      toast.success(t("settings.duplicate.done"));
      ctx.goTo(id, false);
    }
  };
  const exportThis = async () => {
    const uri = await ctx.run(() => exportChat(api, conversationId, view.title));
    if (uri) toast.success(t("settings.export.done"));
  };
  const importLog = async () => {
    if (!ctx.characterId) return;
    const id = await ctx.run(() => importChatLog(api, queryClient, ctx.characterId ?? ""));
    if (id) ctx.goTo(id, true);
  };
  const archive = async () => {
    const done = await ctx.run(() => setArchived(api, queryClient, conversationId, !view.archived).then(() => true));
    if (done && !view.archived) onHome();
  };
  const remove = async () => {
    const sure = await confirm({
      title: t("settings.delete.title"),
      message: t("settings.delete.body", { title: view.title || t("settings.history.untitled") }),
      confirmLabel: t("settings.delete.confirm"),
      cancelLabel: t("confirm.cancel"),
      destructive: true,
    });
    if (!sure) return;
    const done = await ctx.run(() => removeChat(api, queryClient, conversationId).then(() => true));
    if (done) onHome();
  };
  const retryMemory = () =>
    void ctx.run(async () => {
      const outcome = await api.call("memoryRewindRetry", { conversation_id: conversationId });
      await queryClient.invalidateQueries({ queryKey: queryKeys.conversations.detail(conversationId) });
      if (outcome.type === "still_failing") toast.warning(t("banner.memoryRetryStill"));
      else toast.success(t("banner.memoryRetried"));
    });

  return (
    <div className="space-y-6 pb-4">
      <div className="flex items-center gap-3">
        <Avatar src={avatar} name={name} size="lg" />
        <div className="min-w-0 flex-1">
          <p className="truncate text-lg font-semibold text-fg">{name}</p>
          {renaming ? (
            <div className="mt-1 flex items-center gap-1.5">
              <Input
                data-autofocus
                value={title}
                onChange={(event) => setTitle(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter") void saveTitle();
                  if (event.key === "Escape") setRenaming(false);
                }}
                aria-label={t("settings.rename.field")}
              />
              <IconButton label={t("settings.rename.save")} disabled={ctx.busy} onClick={() => void saveTitle()}><Check size={icon.md} /></IconButton>
            </div>
          ) : (
            <div className="flex items-center gap-1">
              <p className="truncate text-sm text-fg-2">{view.title || t("settings.history.untitled")}</p>
              <IconButton
                label={t("settings.rename.open")}
                size="sm"
                onClick={() => {
                  setTitle(view.title);
                  setRenaming(true);
                }}
              >
                <Pencil size={icon.sm} />
              </IconButton>
            </div>
          )}
          {meta && <p className="mt-0.5 text-xs text-fg-3">{meta}</p>}
        </div>
      </div>

      <ListGroup label={t("settings.sections.chat")}>
        <SettingRow icon={<User size={icon.lg} />} title={t("tools.persona")} value={personaValue} source={settings.persona.source} disabled={ctx.busy} onOpen={() => ctx.openSheet("persona")} onReset={() => void ctx.save({ persona: { type: "reset" } })} />
        <SettingRow
          icon={<Cpu size={icon.lg} />}
          title={t("settings.model.title")}
          value={modelValue}
          source={settings.model.source}
          disabled={ctx.busy}
          onOpen={() => ctx.openSheet("model")}
          onReset={() => void ctx.save({ model: { type: "reset" } })}
        />
        <ListRowButton icon={<NotebookPen size={icon.lg} />} title={t("tools.note")} description={settings.author_note?.trim() ? t("tools.noteActive") : t("tools.noteInactive")} onClick={() => ctx.openSheet("note")} />
        <SettingRow
          icon={<ImageIcon size={icon.lg} />}
          title={t("tools.background")}
          value={settings.background.hidden ? t("tools.backgroundHidden") : settings.background.asset ? t("settings.background.set") : t("tools.backgroundNone")}
          source={settings.background.source}
          disabled={ctx.busy}
          onOpen={() => ctx.openSheet("background")}
          onReset={() => void ctx.save({ background: { type: "reset" } })}
        />
      </ListGroup>

      <ListGroup label={t("settings.sections.lorebooks")}>
        <SettingRow
          icon={<BookOpen size={icon.lg} />}
          title={t("settings.lorebooks.title")}
          value={lorebooks.isPending ? t("settings.loading") : t("settings.lorebooks.count", { count: settings.lorebooks.ids.length })}
          source={settings.lorebooks.source}
          disabled={ctx.busy}
          onOpen={() => ctx.openSheet("lorebooks")}
          onReset={() => void ctx.save({ lorebooks: { type: "reset" } })}
        />
        {settings.disable_character_lorebooks && (
          <ListRow
            title={t("settings.lorebooks.useCharacter")}
            description={t(`settings.source.${settings.disable_character_lorebooks.source}`)}
            trailing={
              <span className="flex items-center gap-2">
                {settings.disable_character_lorebooks.source === "conversation" && (
                  <Button variant="ghost" size="sm" disabled={ctx.busy} onClick={() => void ctx.save({ disable_character_lorebooks: { type: "reset" } })}>{t("settings.reset")}</Button>
                )}
                <Switch
                  checked={!settings.disable_character_lorebooks.value}
                  disabled={ctx.busy}
                  aria-label={t("settings.lorebooks.useCharacter")}
                  onChange={(on) => void ctx.save({ disable_character_lorebooks: { type: "set", value: !on } })}
                />
              </span>
            }
          />
        )}
      </ListGroup>

      <section className="space-y-3">
        {view.memory_blocked && (
          <Alert tone="warning" title={t("banner.memoryBlocked")} action={<Button variant="secondary" size="sm" onClick={retryMemory}>{t("banner.memoryRetry")}</Button>} />
        )}
        {view.missing_models.includes("embedding") && (
          <Alert tone="warning" title={t("banner.modelNamed", { model: t("requiredModel.embedding") })} action={<Button variant="secondary" size="sm" onClick={onModelsSettings}>{t("banner.openModels")}</Button>} />
        )}
        <ListGroup label={t("settings.sections.memory")}>
          <ListRowButton
            icon={<Sparkles size={icon.lg} />}
            title={t("settings.memory.open")}
            description={
              memoryOn
                ? memory.isPending
                  ? t("settings.loading")
                  : memory.isError
                    ? t("settings.loadFailed")
                    : [t("settings.memory.items", { count: memory.data.items.length }), memory.data.summary?.token_count ? t("settings.memory.summaryTokens", { count: memory.data.summary.token_count }) : null].filter(Boolean).join(" · ")
                : t("settings.memory.off")
            }
            onClick={onOpenMemories}
          />
        </ListGroup>
      </section>

      <ListGroup label={t("settings.sections.generation")}>
        <ListRowButton
          icon={<SlidersHorizontal size={icon.lg} />}
          title={t("settings.generation.title")}
          description={overrides > 0 ? t("settings.generation.count", { count: overrides }) : t("settings.generation.inherited")}
          onClick={() => ctx.open("advanced")}
        />
      </ListGroup>

      {companion && (
        <ListGroup label={t("settings.sections.companion")}>
          <ListRowButton icon={<CalendarClock size={icon.lg} />} title={t("settings.notes.title")} description={t("settings.notes.rowHint")} onClick={() => ctx.open("notes")} />
        </ListGroup>
      )}

      <ListGroup label={t("settings.sections.actions")}>
        <ListRowButton icon={<MessageSquarePlus size={icon.lg} />} title={t("settings.actions.newChat")} description={t("settings.actions.newChatHint")} disabled={ctx.busy || !ctx.characterId || character.isPending} onClick={() => void startChat()} />
        <ListRowButton icon={<History size={icon.lg} />} title={t("settings.actions.history")} description={t("settings.actions.historyHint")} onClick={() => ctx.open("history")} />
        <ListRowButton icon={<GitBranch size={icon.lg} />} title={t("settings.actions.branches")} description={t("settings.actions.branchesHint")} onClick={onBranches} />
        <ListRowButton icon={<Copy size={icon.lg} />} title={t("settings.actions.duplicate")} description={t("settings.actions.duplicateHint")} chevron={false} disabled={ctx.busy} onClick={() => void duplicate()} />
        <ListRowButton icon={<Download size={icon.lg} />} title={t("settings.actions.export")} description={t("settings.actions.exportHint")} chevron={false} disabled={ctx.busy} onClick={() => void exportThis()} />
        <ListRowButton icon={<Upload size={icon.lg} />} title={t("settings.actions.import")} description={t("settings.actions.importHint")} chevron={false} disabled={ctx.busy || !ctx.characterId} onClick={() => void importLog()} />
        <ListRowButton
          icon={view.archived ? <ArchiveRestore size={icon.lg} /> : <Archive size={icon.lg} />}
          title={view.archived ? t("settings.actions.restore") : t("settings.actions.archive")}
          description={view.archived ? t("settings.actions.restoreHint") : t("settings.actions.archiveHint")}
          chevron={false}
          disabled={ctx.busy}
          onClick={() => void archive()}
        />
        <ListRowButton icon={<Trash2 size={icon.lg} />} tone="danger" title={<span className="text-danger">{t("settings.actions.delete")}</span>} description={t("settings.actions.deleteHint")} chevron={false} disabled={ctx.busy} onClick={() => void remove()} />
      </ListGroup>
    </div>
  );
}
