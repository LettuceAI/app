import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { ImagePlus, Trash2 } from "lucide-react";
import { Button, Field, icon, IconButton, ListGroup, ListRow, ListRowButton, MediaTile, Textarea } from "@lettuceai/crisp";
import { FailureState } from "@/shared/ui/FailureState";
import { editedScene, sceneEditorText, sceneImageToken } from "../data/scene-editor";
import type { StepProps } from "./MembersStep";
import type { AssetRef } from "@/api/generated/bindings";

export function GroupSceneEditor({ api, form, update, autoFocus, assets, names }: StepProps & { autoFocus: boolean; assets: AssetRef[]; names: string[] }) {
  const { t } = useTranslation("groups");
  const scene = form.draft.starting_scene?.scene;
  const text = scene ? sceneEditorText(scene) : "";
  const editor = useRef<HTMLTextAreaElement>(null);
  const caret = useRef<number | null>(null);
  const mounted = useRef(true);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  const [uploading, setUploading] = useState(false);
  const [error, setError] = useState<unknown>(null);
  const [query, setQuery] = useState<{ start: number; end: number; text: string } | null>(null);
  const [index, setIndex] = useState(0);
  const matches = query ? names.filter((name) => name.toLocaleLowerCase().includes(query.text.toLocaleLowerCase())) : [];
  useLayoutEffect(() => {
    if (caret.current === null) return;
    editor.current?.focus({ preventScroll: true });
    editor.current?.setSelectionRange(caret.current, caret.current);
    caret.current = null;
  }, [text]);
  const change = (value: string) => update((current) => {
    const starting = current.draft.starting_scene;
    return starting ? { ...current, draft: { ...current.draft, starting_scene: { ...starting, scene: editedScene(starting.scene, value, starting.variants) } } } : current;
  });
  const complete = (name: string) => {
    if (!query) return;
    const token = `{{@"${name}"}}`;
    caret.current = query.start + token.length;
    change(text.slice(0, query.start) + token + text.slice(query.end));
    setQuery(null);
  };
  const upload = async () => {
    if (uploading) return;
    setUploading(true); setError(null);
    const start = editor.current?.selectionStart ?? text.length;
    const end = editor.current?.selectionEnd ?? start;
    try {
      const picked = await api.call("filesPickOpen", { kinds: ["image"], multiple: false });
      if (!picked.sources[0]) return;
      const asset = await api.call("assetsIngest", { source: picked.sources[0], role: "reference_image" });
      if (!mounted.current) return;
      const link = { id: crypto.randomUUID(), asset_id: asset.asset_id, slot: "inline" as const, ordinal: scene?.assets.length ?? 0 };
      const token = sceneImageToken(link.id);
      caret.current = start + token.length;
      update((current) => {
        const starting = current.draft.starting_scene;
        if (!starting || starting.scene.id !== scene?.id || sceneEditorText(starting.scene) !== text) return current;
        const next = { ...starting.scene, assets: [...starting.scene.assets, link] };
        return { ...current, sceneAssets: [...(current.sceneAssets ?? []), asset], draft: { ...current.draft, starting_scene: { ...starting, scene: editedScene(next, text.slice(0, start) + token + text.slice(end), starting.variants) } } };
      });
    } catch (error) { setError(error); }
    finally { setUploading(false); }
  };
  return <div className="space-y-3">
    <div className="relative">
      <Field label={t("scene.title")} required hint={t("scene.hint")}><Textarea ref={editor} data-autofocus={autoFocus || undefined} disabled={uploading} autoGrow rows={8} maxRows={20} className="min-h-48" value={text} onChange={(event) => {
        const value = event.target.value;
        const end = event.target.selectionStart;
        const match = value.slice(0, end).match(/\{\{@"([^"]*)$/);
        setQuery(match ? { start: end - match[0].length, end, text: match[1]! } : null); setIndex(0); change(value);
      }} onKeyDown={(event) => {
        if (!query) return;
        if (event.key === "Escape") { event.preventDefault(); event.stopPropagation(); setQuery(null); }
        else if (matches.length && (event.key === "ArrowDown" || event.key === "ArrowUp")) { event.preventDefault(); setIndex((current) => (current + (event.key === "ArrowDown" ? 1 : -1) + matches.length) % matches.length); }
        else if (matches[index] && (event.key === "Enter" || event.key === "Tab")) { event.preventDefault(); complete(matches[index]!); }
      }} /></Field>
      {matches.length > 0 && <div className="absolute inset-x-0 top-full z-20 mt-1 max-h-48 overflow-y-auto rounded-xl border border-line bg-surface-el shadow-raised" onMouseDown={(event) => event.preventDefault()}><ListGroup label={t("sceneEditor.characters")}>{matches.map((name, at) => <ListRowButton key={name} title={name} description={`{{@"${name}"}}`} aria-pressed={at === index} chevron={false} onClick={() => complete(name)} />)}</ListGroup></div>}
    </div>
    <Button size="sm" variant="tonal" leading={<ImagePlus size={icon.md} />} loading={uploading} onClick={() => void upload()}>{t("sceneEditor.addImage")}</Button>
    <ListGroup>{scene?.assets.filter((link) => link.slot === "inline" && scene.content.parts.some((part) => part.kind === "inline_asset" && part.link_id === link.id)).map((link, ordinal) => {
      const asset = [...assets, ...(form.sceneAssets ?? [])].find((asset) => asset.asset_id === link.asset_id);
      return <ListRow key={link.id} title={t("sceneEditor.image", { number: ordinal + 1 })} icon={asset ? <span className="block w-16 shrink-0"><MediaTile src={asset.url} alt={t("sceneEditor.image", { number: ordinal + 1 })} onClick={() => { const start = text.indexOf(sceneImageToken(link.id)); editor.current?.focus(); editor.current?.setSelectionRange(start, start + sceneImageToken(link.id).length); }} /></span> : <ImagePlus size={icon.lg} />} trailing={<IconButton label={t("sceneEditor.removeImage", { number: ordinal + 1 })} size="sm" disabled={uploading} onClick={() => change(text.split(sceneImageToken(link.id)).join(""))}><Trash2 size={icon.md} /></IconButton>} />;
    })}</ListGroup>
    {error !== null && <FailureState error={error} onRetry={() => void upload()} />}
  </div>;
}
