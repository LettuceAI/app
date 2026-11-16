import { X } from "lucide-react";
import { icon, IconButton, MediaTile } from "@lettuceai/crisp";
import type { AssetRef, LibrarySceneDraft_Serialize } from "@/api/generated/bindings";

export interface SceneImagesProps {
  scene: LibrarySceneDraft_Serialize;
  assets: readonly AssetRef[];
  disabled: boolean;
  label: (number: number) => string;
  removeLabel: (number: number) => string;
  onInsert: (linkId: string) => void;
  onRemove: (linkId: string) => void;
}

/** The scene's inline images once each, in order of first use; tapping one inserts its token at the caret again. */
export function SceneImages({ scene, assets, disabled, label, removeLabel, onInsert, onRemove }: SceneImagesProps) {
  const links = new Map(scene.assets.filter((link) => link.slot === "inline").map((link) => [link.id, link]));
  const ordered = [...new Set(scene.content.parts.flatMap((part) => (part.kind === "inline_asset" && links.has(part.link_id) ? [part.link_id] : [])))].map((id) => links.get(id)!);
  if (ordered.length === 0) return null;
  return (
    <div className="grid grid-cols-3 gap-2 @xl:grid-cols-5">
      {ordered.map((link, index) => {
        const asset = assets.find((entry) => entry.asset_id === link.asset_id);
        return (
          <div key={link.id} className="relative" onMouseDown={(event) => event.preventDefault()}>
            {asset ? (
              <MediaTile src={asset.url} alt={label(index + 1)} className="aspect-square" onClick={() => !disabled && onInsert(link.id)} />
            ) : (
              <button type="button" aria-label={label(index + 1)} disabled={disabled} className="flex aspect-square w-full items-center justify-center rounded-lg border border-line text-sm text-fg-3" onClick={() => onInsert(link.id)}>{index + 1}</button>
            )}
            <div className="absolute right-1 top-1">
              <IconButton size="sm" variant="overlay" label={removeLabel(index + 1)} disabled={disabled} onClick={() => onRemove(link.id)}>
                <X size={icon.sm} />
              </IconButton>
            </div>
          </div>
        );
      })}
    </div>
  );
}
