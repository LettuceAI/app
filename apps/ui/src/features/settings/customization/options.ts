import type { ThemeColors } from "@lettuceai/crisp";
export { DETAILS, type ChatLook, type Detail } from "@/entities/preferences";
export type Opt<T extends string> = readonly { value: T; label: string; description?: string }[];
export const opts = <T extends string>(pairs: readonly (readonly [T, string])[]): Opt<T> =>
  pairs.map(([value, label]) => ({ value, label }));
export function createThemePreset(name: string, colors: ThemeColors) {
  return { id: crypto.randomUUID(), name, colors, createdAt: Date.now() };
}
