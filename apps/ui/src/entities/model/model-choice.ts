import type { ModelKindContract, ModelModality, ModelView, ProviderAccountView } from "@/api/generated/bindings";

export interface ModelChoice {
  model: ModelView;
  account: ProviderAccountView;
}

/** What a caller needs from a model. Every listed modality must be supported. */
export interface ModelRequirement {
  kind?: ModelKindContract;
  input?: readonly ModelModality[];
  output?: readonly ModelModality[];
  /** Extra caller rule, e.g. no on-device image engine on a phone. */
  allow?: (choice: ModelChoice) => boolean;
}

export interface ProviderGroup {
  account: ProviderAccountView;
  choices: ModelChoice[];
}

const onDeviceKind = "llamacpp";
const maxGap = 2;
const minSubsequenceLength = 3;

/** Joins models to their provider accounts in list order; a model whose account is missing is dropped. */
export function modelChoices(models: readonly ModelView[], accounts: readonly ProviderAccountView[]): ModelChoice[] {
  const byId = new Map(accounts.map((account) => [account.id, account]));
  return models.flatMap((model) => {
    const account = byId.get(model.provider_account_id);
    return account ? [{ model, account }] : [];
  });
}

export function meetsRequirement(choice: ModelChoice, requirement: ModelRequirement | undefined): boolean {
  if (!requirement) return true;
  const { model } = choice;
  if (requirement.kind !== undefined && model.kind !== requirement.kind) return false;
  if (requirement.input?.some((modality) => !model.input_scopes.includes(modality))) return false;
  if (requirement.output?.some((modality) => !model.output_scopes.includes(modality))) return false;
  return requirement.allow ? requirement.allow(choice) : true;
}

/** On-device models first, then the other providers alphabetically by label, with each
    provider's models sorted alphabetically by name. */
export function groupByProvider(choices: readonly ModelChoice[]): ProviderGroup[] {
  const groups = new Map<string, ProviderGroup>();
  for (const choice of choices) {
    const group = groups.get(choice.account.id);
    if (group) group.choices.push(choice);
    else groups.set(choice.account.id, { account: choice.account, choices: [choice] });
  }
  const all = [...groups.values()];
  for (const group of all) group.choices.sort((a, b) => a.model.display_name.localeCompare(b.model.display_name));
  const byLabel = (a: ProviderGroup, b: ProviderGroup) => a.account.label.localeCompare(b.account.label);
  return [
    ...all.filter((group) => group.account.provider_kind === onDeviceKind).sort(byLabel),
    ...all.filter((group) => group.account.provider_kind !== onDeviceKind).sort(byLabel),
  ];
}

interface Field {
  words: string[];
  text: string;
  compact: string;
  weight: number;
}

function normalize(value: string): string {
  return value
    .normalize("NFKD")
    .replace(/\p{M}/gu, "")
    .toLowerCase()
    .replace(/[^\p{L}\p{N}]+/gu, " ")
    .trim();
}

function field(value: string, weight: number): Field {
  const text = normalize(value);
  return { words: text.split(" "), text, compact: text.replaceAll(" ", ""), weight };
}

function chains(token: string, compact: string, index: number, at: number): boolean {
  if (index === token.length) return true;
  for (let next = at + 1; next <= at + 1 + maxGap && next < compact.length; next += 1) {
    if (compact[next] === token[index] && chains(token, compact, index + 1, next)) return true;
  }
  return false;
}

function isSubsequence(token: string, compact: string): boolean {
  for (let start = 0; start < compact.length; start += 1) {
    if (compact[start] === token[0] && chains(token, compact, 1, start)) return true;
  }
  return false;
}

function tokenScore(token: string, target: Field): number {
  let score = 0;
  if (target.words.some((word) => word.startsWith(token))) score = 3;
  else if (target.text.includes(token) || target.compact.includes(token)) score = 2;
  else if (token.length >= minSubsequenceLength && isSubsequence(token, target.compact)) score = 1;
  return score * target.weight;
}

/**
 * Ranked matches for a query typed in any word order. A token matches a field as a word
 * prefix, a substring (also across separators, so `gpt4o` finds `gpt-4o`), or, from three
 * characters, as letters in order with at most two skipped between neighbours. Every token
 * must match some field; higher-weight fields rank first. An empty query returns every item
 * in its order.
 */
export function rankMatches<T>(items: readonly T[], query: string, fieldsOf: (item: T) => readonly (readonly [string, number])[]): T[] {
  const normalized = normalize(query);
  if (normalized === "") return [...items];
  const tokens = normalized.split(" ");
  const ranked: { item: T; score: number; order: number }[] = [];
  items.forEach((item, order) => {
    const fields = fieldsOf(item).map(([value, weight]) => field(value, weight));
    let total = 0;
    for (const token of tokens) {
      const best = Math.max(...fields.map((target) => tokenScore(token, target)));
      if (best === 0) return;
      total += best;
    }
    ranked.push({ item, score: total, order });
  });
  return ranked.sort((a, b) => b.score - a.score || a.order - b.order).map((entry) => entry.item);
}

export function searchModels(choices: readonly ModelChoice[], query: string): ModelChoice[] {
  return rankMatches(choices, query, ({ model, account }) => [
    [model.display_name, 2],
    [model.external_model_id, 1],
    [account.label, 1],
    [account.provider_kind, 1],
  ]);
}
