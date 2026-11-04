import { useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useBlocker } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { Check } from "lucide-react";
import { Avatar, Button, Checkbox, EmptyState, Field, icon, Input, ListGroup, ListRow, ListRowButton, LoadingState, SearchField, Segmented, Sheet, Text, toast } from "@lettuceai/crisp";
import { isApiFailure } from "@/api/client";
import type { ConversationView, GroupChatMode } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import type { CharacterChoice } from "@/entities/character/CharacterChoices";
import { plainText } from "@/shared/formatting/plainText";
import { FailureState } from "@/shared/ui/FailureState";
import { branchGroupToCharacter, branchToGroup, branchToNewChat, type ConversationServices } from "../data/actions";
import { libraryCastQuery } from "../data/settings-queries";

export interface BranchTarget {
  messageId: string;
  revision: number;
  kind: "character" | "group";
}

export function BranchSheet({ open, services, view, target, onClose, onCreated }: {
  open: boolean;
  services: ConversationServices;
  view: ConversationView;
  target: BranchTarget;
  onClose: () => void;
  onCreated: (id: string) => void;
}) {
  const { t } = useTranslation("conversations");
  const { t: common } = useTranslation("common");
  const { t: groups } = useTranslation("groups");
  const source = view.participants.find((participant) => participant.role === "character");
  const ownerId = view.source.type === "direct" ? view.source.character_id : null;
  const group = target.kind === "group";
  const [selected, setSelected] = useState<string[]>(group && ownerId ? [ownerId] : []);
  const [mode, setMode] = useState<GroupChatMode>("conversation");
  const [title, setTitle] = useState("");
  const [query, setQuery] = useState("");
  const [busy, setBusy] = useState(false);
  const running = useRef(false);
  const operation = useRef<{ payload: string; id: string } | null>(null);
  const cast = useQuery(libraryCastQuery(services.api));
  useBlocker({
    shouldBlockFn: ({ action }) => {
      if (!open || action !== "BACK") return false;
      if (!running.current) onClose();
      return true;
    },
    enableBeforeUnload: false,
  });
  const choices: CharacterChoice[] = (cast.data ?? []).filter((character) => group || view.kind === "group" || character.id !== ownerId);
  if (group && ownerId && !choices.some((character) => character.id === ownerId)) {
    choices.unshift({ id: ownerId, name: source?.name ?? "", avatar: source?.avatar ?? null, description: t("branchSetup.sourceLocked") });
  }
  const memberIds = view.participants.flatMap((participant) => participant.character_id ? [participant.character_id] : []);
  choices.sort((left, right) => Number(memberIds.includes(right.id)) - Number(memberIds.includes(left.id)));
  const needle = query.trim().toLocaleLowerCase();
  const shown = choices.filter((character) => `${character.name} ${character.description ?? ""}`.toLocaleLowerCase().includes(needle));
  const valid = !!cast.data && (group ? selected.length >= 2 : selected.length === 1)
    && selected.every((id) => choices.some((character) => character.id === id));
  const pick = (id: string) => {
    if (busy || (group && id === ownerId)) return;
    setSelected((current) => group ? current.includes(id) ? current.filter((item) => item !== id) : [...current, id] : [id]);
  };
  const submit = async () => {
    if (running.current || !valid) return;
    running.current = true;
    setBusy(true);
    const payload = JSON.stringify({ target, selected, mode, title: title.trim() });
    if (operation.current?.payload !== payload) operation.current = { payload, id: crypto.randomUUID() };
    const options = { revision: target.revision, operationId: operation.current.id };
    try {
      const message = { id: target.messageId };
      const id = group
        ? await branchToGroup(services, message, selected, mode, title, options)
        : view.kind === "group"
          ? await branchGroupToCharacter(services, message, selected[0]!, options)
          : await branchToNewChat(services, message, selected[0]!, options);
      onCreated(id);
    } catch (error) {
      toast.error(common(`errors.code.${isApiFailure(error) ? error.code : "internal"}`));
      if (isApiFailure(error) && error.code === "conflict") {
        await services.queryClient.invalidateQueries({ queryKey: queryKeys.conversations.detail(services.conversationId) });
      }
    } finally {
      running.current = false;
      setBusy(false);
    }
  };
  return <Sheet open={open} onClose={onClose} size="md" title={t(group ? "branchSetup.groupTitle" : "branchSetup.characterTitle")}
    description={t("branchSetup.history")} dismissible={!busy}
    footer={<div className="flex gap-2">
      <Button variant="secondary" className="flex-1" disabled={busy} onClick={onClose}>{common("actions.cancel")}</Button>
      <Button variant="primary" className="flex-1" loading={busy} disabled={!valid || busy || view.revision !== target.revision} onClick={() => void submit()}>{t("branchSetup.create")}</Button>
    </div>}>
    <div className="space-y-4 pb-1">
      {group && <>
        <Field label={groups("setup.name")}><Input value={title} disabled={busy} placeholder={t("branchSetup.defaultName", { name: source?.name ?? view.title })} onChange={(event) => setTitle(event.target.value)} /></Field>
        <Field label={groups("setup.mode")} hint={groups(`setup.${mode}Body`)}>
          <Segmented<GroupChatMode> block aria-label={groups("setup.mode")} value={mode} onChange={(value) => { if (!busy) setMode(value); }}
            options={[{ value: "conversation", label: groups("setup.conversation") }, { value: "roleplay", label: groups("setup.roleplay") }]} />
        </Field>
        <div className="space-y-1 px-1">
          <Text variant="label" className="block">{t("branchSetup.characters")}</Text>
          <Text variant="hint">{t("branchSetup.sourceLocked")}</Text>
        </div>
      </>}
      <SearchField value={query} onChange={setQuery} placeholder={t("branchSetup.search")} aria-label={t("branchSetup.search")} />
      {cast.isPending ? <LoadingState /> : cast.isError ? <FailureState error={cast.error} onRetry={() => void cast.refetch()} /> : shown.length === 0 ? <EmptyState size="sm" title={t("branchSetup.empty")} /> :
        <ListGroup>
          {shown.map((character) => {
            const on = selected.includes(character.id);
            const avatar = <Avatar src={character.avatar?.url ?? null} name={character.name} size="sm" />;
            const description = character.description ? <span className="line-clamp-1">{plainText(character.description)}</span> : undefined;
            return group
              ? <ListRow key={character.id} icon={avatar} title={character.name} description={description}
                trailing={<Checkbox checked={on} disabled={busy || character.id === ownerId} aria-label={character.name} onChange={() => pick(character.id)} />} />
              : <ListRowButton key={character.id} role="radio" aria-checked={on} tone={on ? "accent" : "neutral"} icon={avatar} title={character.name}
                description={description} chevron={false} disabled={busy} trailing={on ? <Check size={icon.md} className="text-accent" /> : undefined} onClick={() => pick(character.id)} />;
          })}
        </ListGroup>}
      {group && <Text variant="hint">{t("branchSetup.memberCount", { count: selected.length })}</Text>}
      {view.revision !== target.revision && <div className="space-y-2">
        <Text variant="hint">{t("branchSetup.changed")}</Text>
        <Button variant="secondary" onClick={onClose}>{t("branchSetup.chooseAgain")}</Button>
      </div>}
    </div>
  </Sheet>;
}
