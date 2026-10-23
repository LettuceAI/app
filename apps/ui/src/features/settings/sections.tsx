import type { ReactNode } from "react";
import {
  ArrowLeftRight, BarChart3, BookOpen, Brain, Cpu, EthernetPort, FileCode, FileText, Gauge, HardDrive, HelpCircle, Image, Info,
  Mic, Palette, RefreshCw, RotateCcw, ScrollText, Shield, Sliders, Sparkles, Volume2, Wrench,
} from "lucide-react";
import { icon } from "@lettuceai/crisp";

export const settingsSectionIds = [
  "providers", "models", "memory", "imageGeneration", "prompts", "advanced", "performance",
  "voices", "customization", "speech",
  "sync", "backup", "convert",
  "security", "usage",
  "help", "whatsNew", "about", "changelog", "docs", "logs", "guide",
  "reset", "developer",
] as const;

export type SettingsSectionId = (typeof settingsSectionIds)[number];

export interface SettingsSection {
  id: SettingsSectionId;
  icon: ReactNode;
  danger?: boolean;
}

export interface SettingsGroup {
  id: "intelligence" | "experience" | "connectivity" | "security" | "support" | "danger";
  sections: readonly SettingsSection[];
}

const glyph = (Icon: typeof Info) => <Icon size={icon.lg} />;

export const settingsGroups: readonly SettingsGroup[] = [
  {
    id: "intelligence",
    sections: [
      { id: "providers", icon: glyph(EthernetPort) },
      { id: "models", icon: glyph(Cpu) },
      { id: "memory", icon: glyph(Brain) },
      { id: "imageGeneration", icon: glyph(Image) },
      { id: "prompts", icon: glyph(FileText) },
      { id: "advanced", icon: glyph(Sliders) },
      { id: "performance", icon: glyph(Gauge) },
    ],
  },
  {
    id: "experience",
    sections: [
      { id: "voices", icon: glyph(Volume2) },
      { id: "customization", icon: glyph(Palette) },
      { id: "speech", icon: glyph(Mic) },
    ],
  },
  {
    id: "connectivity",
    sections: [
      { id: "sync", icon: glyph(RefreshCw) },
      { id: "backup", icon: glyph(HardDrive) },
      { id: "convert", icon: glyph(ArrowLeftRight) },
    ],
  },
  {
    id: "security",
    sections: [
      { id: "security", icon: glyph(Shield) },
      { id: "usage", icon: glyph(BarChart3) },
    ],
  },
  {
    id: "support",
    sections: [
      { id: "help", icon: glyph(HelpCircle) },
      { id: "whatsNew", icon: glyph(Sparkles) },
      { id: "about", icon: glyph(Info) },
      { id: "changelog", icon: glyph(ScrollText) },
      { id: "docs", icon: glyph(BookOpen) },
      { id: "logs", icon: glyph(FileCode) },
      { id: "guide", icon: glyph(HelpCircle) },
    ],
  },
  {
    id: "danger",
    sections: [
      { id: "reset", icon: glyph(RotateCcw), danger: true },
      { id: "developer", icon: glyph(Wrench) },
    ],
  },
];

/** The sections that have a page so far; the rest are listed but cannot be opened yet. */
export const builtSettingsSections: ReadonlySet<SettingsSectionId> = new Set(["about", "providers", "models", "memory", "imageGeneration", "backup", "prompts"]);

export function isSettingsSection(value: string): value is SettingsSectionId {
  return (settingsSectionIds as readonly string[]).includes(value);
}
