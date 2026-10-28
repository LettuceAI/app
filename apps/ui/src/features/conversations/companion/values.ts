import type { CompanionRelationshipDefaults, RelationshipChange } from "@/api/generated/bindings";

export const axes = ["closeness", "trust", "affection", "tension", "stability"] as const;
export type Axis = (typeof axes)[number];
export const emotionIds = ["warmth", "trust", "calm", "vulnerability", "longing", "hurt", "tension", "irritation", "affection_intensity", "reassurance_need"] as const;
export const emotionGroups = ["felt", "expressed", "blocked", "momentum"] as const;

export function signed(value: number, locale: string): string {
  return new Intl.NumberFormat(locale, { signDisplay: "exceptZero", maximumFractionDigits: 0 }).format(Math.round(value * 100));
}

export function numericEntries<T extends Record<string, number | null>>(values: T): [keyof T & string, number][] {
  return Object.entries(values).flatMap(([key, value]) => value !== null && Number.isFinite(value) ? [[key as keyof T & string, value] as [keyof T & string, number]] : []);
}

export function dataLabel(value: string): string {
  const words = value.replace(/^emotion:/, "").replaceAll("_", " ");
  return words.charAt(0).toUpperCase() + words.slice(1);
}

export function referenceFor(axis: Axis, defaults: CompanionRelationshipDefaults): number | null {
  return axis === "stability" ? null : defaults[axis];
}

export function changedAxes(relationship: RelationshipChange) {
  return numericEntries(relationship).filter(([, value]) => value !== 0);
}
