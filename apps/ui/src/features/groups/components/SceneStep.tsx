import { useQueries, useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Avatar, ChoiceCard, icon, IconButton, InlineMarkdown, ListGroup, ListRowButton, LoadingState, Text, toast } from "@lettuceai/crisp";
import { Check, Copy } from "lucide-react";
import type { CharacterView_Serialize, LibrarySceneDraft_Serialize } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { FailureState } from "@/shared/ui/FailureState";
import { sceneFromText } from "../data/form";
import { GroupSceneEditor } from "./GroupSceneEditor";
import { castQuery } from "../data/queries";
import type { StepProps } from "./MembersStep";

export function SceneStep({ api, form, update, autoFocus = true }: StepProps & { autoFocus?: boolean }) {
  const { t } = useTranslation("groups");
  const cast = useQuery(castQuery(api));
  const queries = useQueries({ queries: form.draft.members.map((member) => ({ queryKey: [...queryKeys.characters.all, "detail", member.character_id], queryFn: () => api.call("characterGet", { character_id: member.character_id }) })) });
  const failed = queries.find((query) => query.isError);
  const loading = queries.some((query) => query.isPending);
  const scenes = queries.flatMap((query) => query.data ? query.data.draft.scenes.map((scene) => ({ owner: query.data!, scene })) : []);
  const selectSource = (sceneSource: typeof form.sceneSource) => update((current) => current.sceneSource === sceneSource ? current : ({ ...current, sceneSource, draft: { ...current.draft, starting_scene: sceneSource === "none" ? null : sceneSource === "custom" ? sceneFromText("", crypto.randomUUID()) : null } }));
  const copyScene = (scene: LibrarySceneDraft_Serialize, owner: CharacterView_Serialize) => {
    update((current) => ({ ...current, sceneSource: "custom", sceneAssets: [...(current.sceneAssets ?? []), ...owner.assets], draft: { ...current.draft, starting_scene: { scene: { ...scene, ordinal: 0 }, variants: owner.draft.variants.filter((variant) => variant.scene_id === scene.id) } } }));
    toast.success(t("scene.copied"));
  };
  return <div className="grid grid-cols-1 gap-6 @4xl:grid-cols-2 @4xl:gap-x-10">
    <ListGroup label={t("scene.opening")} className="self-start">{(["none", "custom", "member"] as const).map((id) => <ListRowButton key={id} role="radio" aria-checked={form.sceneSource === id} disabled={id === "member" && !loading && !failed && scenes.length === 0} title={t(`scene.${id}`)} description={t(id === "member" && scenes.length === 0 && !loading && !failed ? "scene.noMemberScenes" : `scene.${id}Body`)} chevron={false} trailing={form.sceneSource === id ? <Check size={icon.md} /> : undefined} onClick={() => selectSource(id)} />)}</ListGroup>
    <div className="space-y-3">
      {form.sceneSource === "none" && <Text variant="hint" className="px-1 @4xl:pt-8">{t("scene.noneHint")}</Text>}
      {form.sceneSource === "custom" && <GroupSceneEditor api={api} form={form} update={update} autoFocus={autoFocus} assets={queries.flatMap((query) => query.data?.assets ?? [])} names={form.draft.members.flatMap((member) => cast.data?.filter((entry) => entry.id === member.character_id).map((entry) => entry.name) ?? [])} />}
      {form.sceneSource === "member" && (loading ? <LoadingState /> : failed ? <FailureState error={failed.error} onRetry={() => void failed.refetch()} /> : scenes.map(({ owner, scene }) => {
        const character = cast.data?.find((item) => item.id === owner.id);
        const text = scene.content.parts.flatMap((part) => part.kind === "text" ? [part.text] : []).join("");
        return <div key={scene.id} className="relative"><ChoiceCard selected={form.draft.starting_scene?.scene.id === scene.id} icon={<Avatar src={character?.avatar?.url ?? null} name={owner.draft.profile.name} size="md" />} title={<span className="block truncate pr-9">{owner.draft.profile.name}</span>} description={<span className="line-clamp-3"><InlineMarkdown>{text}</InlineMarkdown></span>} className="p-3" onClick={() => update((current) => ({ ...current, draft: { ...current.draft, starting_scene: { scene: { ...scene, ordinal: 0 }, variants: owner.draft.variants.filter((variant) => variant.scene_id === scene.id) } } }))} /><IconButton label={t("scene.copy")} size="sm" className="absolute right-11 top-2.5" onClick={() => copyScene(scene, owner)}><Copy size={icon.sm} /></IconButton></div>;
      }))}
    </div>
  </div>;
}
