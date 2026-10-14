export interface Roll {
  id: number;
  total: number;
  rolls: number[];
  modifier: number;
  sides: number;
  notation: string;
}

export const quickDice = [4, 6, 8, 10, 12, 20, 100] as const;

export const defaultNotation = "1d20";

export function diceLabel(sides: number): string {
  return `d${sides}`;
}

const pattern = /^(\d*)d(\d+)\s*([+-]\s*\d+)?$/;

export function rollNotation(notation: string, id: number): Roll | null {
  const match = pattern.exec(notation.trim().toLowerCase());
  if (!match) return null;
  const count = Math.min(Math.max(parseInt(match[1] || "1", 10), 1), 100);
  const sides = Math.min(Math.max(parseInt(match[2] ?? "", 10), 2), 1000);
  const modifier = match[3] ? parseInt(match[3].replace(/\s/g, ""), 10) : 0;
  const rolls = Array.from({ length: count }, () => Math.floor(Math.random() * sides) + 1);
  const total = rolls.reduce((sum, value) => sum + value, 0) + modifier;
  return { id, total, rolls, modifier, sides, notation: `${count}d${sides}${modifier ? (modifier > 0 ? `+${modifier}` : modifier) : ""}` };
}

export function rollText(roll: Roll): string {
  return `🎲 ${roll.notation}: ${roll.total}`;
}
