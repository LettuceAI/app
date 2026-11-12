import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { useBlocker } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { Camera } from "lucide-react";
import { Avatar, Button, confirm, Field, icon, IconButton, Input, MediaTile, Textarea, toast } from "@lettuceai/crisp";
import { isApiFailure, type ApiClient } from "@/api/client";
import type { AssetRef, CreatorDraft, CreatorImageView, CreatorKind } from "@/api/generated/bindings";
import { AvatarGeneration, pickImage, staysOnPage, useAvatarCrop } from "@/entities/image-input";
import { EditSaveBar } from "@/shared/ui/EditSaveBar";
import { DraftScenes } from "./DraftScenes";

/** Crops an image for the avatar, from a pick or from the session's images. */
function useAvatarChoice(api: ApiClient, disabled: boolean, onChange: (avatar: AssetRef) => void) {
  const { t: common } = useTranslation();
  const crop = useAvatarCrop(api);
  const [picking, setPicking] = useState(false);
  const choose = async (source: () => Promise<AssetRef | null>) => {
    if (picking || disabled) return;
    setPicking(true);
    try {
      const picked = await source();
      const asset = picked ? await crop.choose(picked) : null;
      if (asset) onChange(asset);
    } catch (error) {
      toast.error(common(`errors.code.${isApiFailure(error) ? error.code : "internal"}`));
    } finally {
      setPicking(false);
    }
  };
  return { choose, busy: picking || disabled, editor: crop.editor };
}

/** The avatar beside the name, as the character and persona editors show it. */
function DraftAvatar({ api, draft, busy, onPick, onChange }: {
  api: ApiClient;
  draft: CreatorDraft;
  busy: boolean;
  onPick: () => void;
  onChange: (avatar: AssetRef | null) => void;
}) {
  const { t } = useTranslation("creator");
  return (
    <div className="flex shrink-0 flex-col items-center gap-2">
      <IconButton label={t("avatar.choose")} variant="ghost" disabled={busy} className="relative h-28 w-28 overflow-visible rounded-full p-0" onClick={onPick}>
        <Avatar src={draft.avatar?.url ?? null} name={draft.name.trim() || "?"} size="xl" className="h-28 w-28 text-3xl" />
        <span className="absolute -right-1 -bottom-1 flex h-9 w-9 items-center justify-center rounded-full border border-line-2 bg-surface-2 text-fg shadow-raised"><Camera size={icon.md} /></span>
      </IconButton>
      <AvatarGeneration
        api={api}
        disabled={busy}
        readContext={() => ({ subjectName: draft.name.trim(), subjectDescription: draft.definition.trim(), currentAvatar: draft.avatar })}
        onUse={onChange}
      />
      {draft.avatar && <Button size="sm" variant="ghost" disabled={busy} onClick={() => onChange(null)}>{t("avatar.remove")}</Button>}
    </div>
  );
}

/** The session's images as a small grid; one becomes the avatar after cropping. */
function SessionImages({ images, busy, onPick }: {
  images: readonly CreatorImageView[];
  busy: boolean;
  onPick: (asset: AssetRef) => void;
}) {
  const { t } = useTranslation("creator");
  if (images.length === 0) return null;
  return (
    <section className="space-y-2">
      <p className="text-sm font-medium text-fg">{t("avatar.sessionImages")}</p>
      <div className="grid grid-cols-4 gap-1.5 @xl:grid-cols-6">
        {images.map((image) => (
          <MediaTile
            key={image.asset.asset_id}
            src={image.asset.preview_url ?? image.asset.url}
            alt={t(`avatar.origin.${image.origin}`)}
            disabled={busy}
            onClick={() => onPick(image.asset)}
          />
        ))}
      </div>
    </section>
  );
}

/**
 * The draft column: the fields the creator writes, editable by hand. Hand edits stay local until saved; `onPending`
 * reports them meanwhile, so the page can save them before it sends a message or opens the review.
 */
export function DraftEditor({ api, images, initial, kind, disabled, phone, save, onDirty, onPending }: {
  api: ApiClient;
  images: readonly CreatorImageView[];
  initial: CreatorDraft;
  kind: CreatorKind;
  disabled: boolean;
  phone: boolean;
  save: (draft: CreatorDraft) => Promise<boolean>;
  onDirty: (dirty: boolean) => void;
  onPending: (draft: CreatorDraft | null) => void;
}) {
  const { t } = useTranslation("creator");
  const [draft, setDraft] = useState(initial);
  const setAvatar = (avatar: AssetRef | null) => setDraft((value) => ({ ...value, avatar }));
  const avatar = useAvatarChoice(api, disabled, setAvatar);
  const [saving, setSaving] = useState(false);
  const [sceneId, setSceneId] = useState<string | null>(null);
  const dirty = JSON.stringify(draft) !== JSON.stringify(initial);
  const latest = useRef(dirty);
  useLayoutEffect(() => {
    latest.current = dirty;
    onPending(dirty ? draft : null);
  }, [dirty, draft, onPending]);
  useLayoutEffect(() => () => onPending(null), [onPending]);
  useEffect(() => {
    onDirty(dirty);
    return () => onDirty(false);
  }, [dirty, onDirty]);
  useBlocker({
    shouldBlockFn: async ({ current, next }) =>
      staysOnPage(current, next) || !latest.current
        ? false
        : !(await confirm({ title: t("leaveTitle"), message: t("leaveBody"), confirmLabel: t("discard"), cancelLabel: t("keepEditing"), destructive: true })),
    enableBeforeUnload: () => latest.current,
  });

  const valid = draft.scenes.every((scene) => scene.content.trim());
  const submit = async () => {
    setSaving(true);
    try {
      await save(draft);
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="min-h-0 flex-1 overflow-y-auto px-4 py-5 @xl:px-6">
        <div className="space-y-5">
          {avatar.editor}
          <div className="flex flex-col items-center gap-5 @xl:flex-row @xl:items-start">
            <DraftAvatar api={api} draft={draft} busy={avatar.busy} onPick={() => void avatar.choose(() => pickImage(api, "avatar"))} onChange={setAvatar} />
            <div className="w-full min-w-0 @xl:flex-1">
              <Field label={t("name")}>
                <Input value={draft.name} disabled={disabled} onChange={(event) => setDraft({ ...draft, name: event.target.value })} />
              </Field>
            </div>
          </div>
          <Field label={t(kind === "character" ? "definition" : "description")}>
            <Textarea value={draft.definition} disabled={disabled} rows={8} autoGrow maxRows={24} onChange={(event) => setDraft({ ...draft, definition: event.target.value })} />
          </Field>
          <SessionImages images={images} busy={avatar.busy} onPick={(asset) => void avatar.choose(async () => asset)} />
          {kind === "character" && (
            <DraftScenes
              scenes={draft.scenes}
              selectedId={sceneId}
              disabled={disabled}
              {...(dirty ? {
                footer: (
                  <div className="space-y-2">
                    <p className="text-sm text-fg-3" role="status">{t("unsavedStatus")}</p>
                    <div className="flex gap-2">
                      <Button variant="ghost" className="flex-1" disabled={disabled || saving} onClick={() => setDraft(initial)}>{t("discard")}</Button>
                      <Button variant="primary" className="flex-1" loading={saving} disabled={disabled || !valid} onClick={() => void submit()}>{t("saveDraft")}</Button>
                    </div>
                  </div>
                ),
              } : {})}
              onSelect={setSceneId} onChange={(scenes) => setDraft((value) => ({ ...value, scenes }))} />
          )}
        </div>
      </div>
      <EditSaveBar
        visible={dirty}
        phone={phone}
        saving={saving}
        canSave={!disabled && valid}
        discardDisabled={disabled || saving}
        status={t("unsavedStatus")}
        discardLabel={t("discard")}
        saveLabel={t("saveDraft")}
        onDiscard={() => setDraft(initial)}
        onSave={() => void submit()}
      />
    </div>
  );
}
