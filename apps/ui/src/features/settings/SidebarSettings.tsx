import { motion, useReducedMotion } from "framer-motion";
import { useTranslation } from "react-i18next";
import { cn, motion as m, NavLabel, useNavExpanded } from "@lettuceai/crisp";
import { builtSettingsSections, settingsGroups, type SettingsSectionId } from "./sections";

/** The settings groups drawn like the home sidebar's recent chats: headings fold away, icons keep the rail slot. */
export function SidebarSettings({ active, onOpen }: { active: SettingsSectionId | null; onOpen: (id: SettingsSectionId) => void }) {
  const { t } = useTranslation("settings");
  const open = useNavExpanded();
  const reduced = useReducedMotion();
  const spring = reduced ? { duration: 0 } : m.sidebar;
  const fade = reduced ? { duration: 0 } : open ? { duration: 0.2, delay: 0.1 } : { duration: 0.1 };
  return (
    <div className="pb-2" aria-label={t("title")}>
      {settingsGroups.map((group, index) => (
        <motion.section key={group.id} aria-label={t(`groups.${group.id}`)} initial={false} animate={{ marginTop: index === 0 ? 0 : open ? 12 : 2 }} transition={spring}>
          <motion.h2
            initial={false}
            animate={{ height: open ? 24 : 0, opacity: open ? 1 : 0 }}
            transition={{ height: spring, opacity: fade }}
            aria-hidden={!open}
            className="overflow-hidden whitespace-nowrap px-3 text-2xs font-medium leading-6 text-fg-3"
          >
            {t(`groups.${group.id}`)}
          </motion.h2>
          <div className="space-y-0.5">
            {group.sections.map((section) => {
              const current = section.id === active;
              const built = builtSettingsSections.has(section.id);
              const title = t(`sections.${section.id}.title`);
              return (
                <button
                  key={section.id}
                  type="button"
                  disabled={!built}
                  aria-current={current ? "page" : undefined}
                  title={open ? undefined : title}
                  onClick={() => onOpen(section.id)}
                  className={cn(
                    "flex h-9 w-full items-center overflow-hidden rounded-lg text-left text-sm transition-colors motion-instant",
                    !built && "cursor-not-allowed opacity-40",
                    current ? "bg-fill-2 text-fg" : section.danger ? "text-danger/80" : "text-fg-2",
                    built && !current && (section.danger ? "hover:bg-danger/10 hover:text-danger" : "hover:bg-fill hover:text-fg"),
                  )}
                >
                  <span className="flex w-11 shrink-0 items-center justify-center [&>svg]:h-4 [&>svg]:w-4">{section.icon}</span>
                  <NavLabel>{title}</NavLabel>
                </button>
              );
            })}
          </div>
        </motion.section>
      ))}
    </div>
  );
}
