import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { FileUp, Search, Users } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Avatar, Button, CharacterCardSkeleton, ChoiceCard, EmptyState, icon, ListGroup, ListRow, SearchField, Text } from "@lettuceai/crisp";
import type { ApiClient } from "@/api/client";
import { FailureState } from "@/shared/ui/FailureState";
import { castQuery } from "../data/queries";
import type { GroupForm } from "../data/form";

export interface StepProps {
  api: ApiClient;
  form: GroupForm;
  update: (change: (current: GroupForm) => GroupForm) => void;
}

export function MembersStep({ api, form, update, onImport }: StepProps & { onImport: () => void }) {
  const { t, i18n } = useTranslation("groups");
  const navigate = useNavigate();
  const cast = useQuery(castQuery(api));
  const [query, setQuery] = useState("");
  const needle = query.trim().toLocaleLowerCase();
  const visible = (cast.data ?? []).filter((member) => [member.name, member.description].some((text) => text?.toLocaleLowerCase().includes(needle)));
  const picked = form.draft.members.map((member) => member.character_id);
  const grid = "grid grid-cols-1 gap-2 @3xl:grid-cols-2 @6xl:grid-cols-3";
  return <div className="space-y-4">
    <div className="flex flex-wrap items-center gap-x-4 gap-y-3">
      <div className="min-w-0 flex-1 basis-64"><SearchField value={query} onChange={setQuery} placeholder={t("members.search")} aria-label={t("members.search")} size="sm" /></div>
      <Text variant="hint" className="tabular-nums" aria-live="polite">{t(picked.length < 2 ? "members.picked" : "members.count", { count: picked.length, formatted: new Intl.NumberFormat(i18n.language).format(picked.length) })}</Text>
    </div>
    {cast.isPending ? <div className={grid}>{Array.from({ length: 6 }, (_, i) => <CharacterCardSkeleton key={i} />)}</div> : cast.isError ? <FailureState error={cast.error} onRetry={() => void cast.refetch()} /> : cast.data.length < 2 ?
      <EmptyState icon={<Users size={icon.xl} />} title={t("members.emptyTitle")} description={t("members.emptyBody")} action={<Button variant="tonal" onClick={() => void navigate({ to: "/characters/new" })}>{t("members.create")}</Button>} /> : visible.length === 0 ?
      <EmptyState size="sm" icon={<Search size={icon.xl} />} title={t("noMatch", { query: query.trim() })} action={<Button variant="secondary" onClick={() => setQuery("")}>{t("clear")}</Button>} /> :
      <div className={grid}>{visible.map((member) => <ChoiceCard key={member.id} selected={picked.includes(member.id)} indicator="checkbox" icon={<Avatar src={member.avatar?.url ?? null} name={member.name} size="md" />} title={<span className="block truncate">{member.name}</span>} description={<span className="line-clamp-1">{member.description}</span>} className="p-3" onClick={() => update((current) => ({ ...current, draft: { ...current.draft, members: picked.includes(member.id) ? current.draft.members.filter((item) => item.character_id !== member.id) : [...current.draft.members, { character_id: member.id, ordinal: current.draft.members.length, muted: false, model_profile_override: null }] } }))} />)}</div>}
    <ListGroup><ListRow icon={<FileUp size={icon.lg} />} title={t("members.import")} description={t("members.importBody")} trailing={<Button size="sm" variant="tonal" onClick={onImport}>{t("members.chooseFile")}</Button>} /></ListGroup>
  </div>;
}
