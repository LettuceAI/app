import type { LibrarySceneDraft_Serialize, LibraryScenePart, LibrarySceneVariantDraft_Serialize } from "@/api/generated/bindings";

export const sceneImageToken = (id: string) => `{{image::${id}}}`;

export function sceneEditorText(scene: LibrarySceneDraft_Serialize): string {
  return scene.content.parts.map((part) => part.kind === "text" ? part.text : sceneImageToken(part.link_id)).join("");
}

export function editedScene(scene: LibrarySceneDraft_Serialize, text: string, variants: LibrarySceneVariantDraft_Serialize[] = []): LibrarySceneDraft_Serialize {
  const links = new Map(scene.assets.filter((asset) => asset.slot === "inline").map((asset) => [asset.id, asset]));
  const parts: LibraryScenePart[] = [];
  const used = new Set(variants.flatMap((variant) => variant.content.parts.flatMap((part) => part.kind === "inline_asset" ? [part.link_id] : [])));
  const pattern = /\{\{image::([^}]+)\}\}/g;
  let start = 0;
  for (const match of text.matchAll(pattern)) {
    if (!links.has(match[1]!)) continue;
    if (match.index > start) parts.push({ kind: "text", text: text.slice(start, match.index) });
    parts.push({ kind: "inline_asset", link_id: match[1]! });
    used.add(match[1]!);
    start = match.index + match[0].length;
  }
  if (start < text.length || parts.length === 0) parts.push({ kind: "text", text: text.slice(start) });
  const ordinals = { inline: 0, background: 0 };
  return { ...scene, selected_variant_id: null, content: { ...scene.content, parts }, assets: scene.assets.filter((asset) => asset.slot !== "inline" || used.has(asset.id)).map((asset) => ({ ...asset, ordinal: ordinals[asset.slot]++ })) };
}
