import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { Download, EyeOff, FileCode, Package, RectangleHorizontal, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Avatar, confirm, icon, ListGroup, ListRowButton, relativeTime, Sheet, toast } from "@lettuceai/crisp";
import type { ApiClient } from "@/api/client";
import { isApiFailure } from "@/api/client";
import type { CharacterExportFormat } from "@/api/generated/bindings";
import { deleteCharacter, exportCharacter, hideCharacter, toggleCardStyle } from "../data/actions";
import type { HomeCharacter } from "../data/home-characters";

const formats: readonly { format: CharacterExportFormat; icon: React.ReactNode }[] = [
  { format: "uec", icon: <Package size={icon.lg} /> },
  { format: "v3", icon: <FileCode size={icon.lg} /> },
  { format: "v2", icon: <FileCode size={icon.lg} /> },
];

/** The menu a long press or right-click opens on a home character. */
export function CharacterMenu({ api, home, onClose }: { api: ApiClient; home: HomeCharacter | null; onClose: () => void }) {
  const { t, i18n } = useTranslation("home");
  const { t: common } = useTranslation("common");
  const queryClient = useQueryClient();
  const services = { api, queryClient };
  const [shown, setShown] = useState<HomeCharacter | null>(home);
  const [exporting, setExporting] = useState<HomeCharacter | null>(null);
  const [busy, setBusy] = useState(false);
  if (home && home !== shown) setShown(home);

  const fail = (error: unknown) => {
    toast.error(common(`errors.code.${isApiFailure(error) ? error.code : "internal"}`));
  };
  const run = async (task: () => Promise<void>) => {
    if (busy) return;
    setBusy(true);
    try {
      await task();
    } catch (error) {
      fail(error);
    } finally {
      setBusy(false);
    }
  };

  const character = shown?.character;
  return (
    <>
      <Sheet
        open={Boolean(home)}
        onClose={onClose}
        size="sm"
        title={character?.name}
        description={
          shown?.latest ? t("lastChat", { when: relativeTime(shown.latest.updated_at, undefined, i18n.language).toLowerCase() }) : shown ? t("menu.noChats") : undefined
        }
        leftAction={character && <Avatar src={character.avatar?.url ?? null} name={character.name} size="md" />}
      >
        {shown && character && (
          <div className="space-y-4 pb-1">
            <ListGroup>
              <ListRowButton
                icon={<RectangleHorizontal size={icon.lg} />}
                title={t("menu.cardStyle")}
                description={character.card_style === "banner" ? t("menu.cardBanner") : t("menu.cardRow")}
                chevron={false}
                disabled={busy}
                onClick={() => void run(() => toggleCardStyle(services, character))}
              />
              <ListRowButton
                icon={<Download size={icon.lg} />}
                title={t("menu.export")}
                description={t("menu.exportBody")}
                onClick={() => {
                  setExporting(shown);
                  onClose();
                }}
              />
            </ListGroup>
            <ListGroup>
              {shown.latest && (
                <ListRowButton
                  icon={<EyeOff size={icon.lg} />}
                  title={t("menu.hide")}
                  description={t("menu.hideBody")}
                  chevron={false}
                  disabled={busy}
                  onClick={() => {
                    const latest = shown.latest;
                    if (!latest) return;
                    void run(async () => {
                      await hideCharacter(services, latest);
                      onClose();
                    });
                  }}
                />
              )}
              <ListRowButton
                icon={<Trash2 size={icon.lg} />}
                tone="danger"
                title={<span className="text-danger">{t("menu.delete")}</span>}
                chevron={false}
                disabled={busy}
                onClick={() => {
                  void (async () => {
                    const sure = await confirm({
                      title: t("menu.deleteTitle", { name: character.name }),
                      message: t("menu.deleteBody"),
                      confirmLabel: t("menu.delete"),
                      cancelLabel: common("actions.cancel"),
                      destructive: true,
                    });
                    if (!sure) return;
                    await run(async () => {
                      await deleteCharacter(services, character);
                      onClose();
                    });
                  })();
                }}
              />
            </ListGroup>
          </div>
        )}
      </Sheet>
      <Sheet open={Boolean(exporting)} onClose={() => setExporting(null)} size="sm" title={t("export.title")} description={t("export.body")}>
        <ListGroup className="mb-1">
          {formats.map(({ format, icon: glyph }) => (
            <ListRowButton
              key={format}
              icon={glyph}
              title={t(`export.${format}.title`)}
              description={t(`export.${format}.body`)}
              chevron={false}
              disabled={busy}
              onClick={() => {
                const target = exporting;
                if (!target) return;
                void run(async () => {
                  const result = await exportCharacter(services, target.character, format);
                  if (!result) return;
                  setExporting(null);
                  toast.success(t("export.done", { name: result.suggested_name }));
                });
              }}
            />
          ))}
        </ListGroup>
      </Sheet>
    </>
  );
}
