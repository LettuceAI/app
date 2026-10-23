import type { PromptCondition } from "@/api/generated/bindings";
import { RULES, type RuleDef } from "./presentation";
import type { Rule, Rules, RuleValue } from "./types";

const rule = (condition: PromptCondition, join: Rule["join"] = "and"): Rule => {
  if (condition.type === "all" || condition.type === "any" || condition.type === "not") return { id: crypto.randomUUID(), type: "group", value: "", join, condition };
  return { id: crypto.randomUUID(), type: condition.type, value: "values" in condition ? condition.values : condition.value, join };
};
export function rulesFromCondition(condition: PromptCondition | null): Rules {
  if (!condition) return { include: [], exclude: [] };
  if (condition.type === "not") return { include: [], exclude: (condition.condition.type === "any" ? condition.condition.conditions : [condition.condition]).map((item) => rule(item, "or")) };
  if (condition.type === "all") {
    const include: Rule[] = []; const exclude: Rule[] = [];
    for (const item of condition.conditions) {
      if (item.type === "not") exclude.push(...(item.condition.type === "any" ? item.condition.conditions : [item.condition]).map((value) => rule(value, "or")));
      else include.push(rule(item));
    }
    return { include, exclude };
  }
  return { include: (condition.type === "any" ? condition.conditions : [condition]).map((item) => rule(item, condition.type === "any" ? "or" : "and")), exclude: [] };
}
function atom(item: Rule): PromptCondition {
  if (item.type === "group") return item.condition!;
  if (Array.isArray(item.value)) return { type: item.type, values: item.value } as PromptCondition;
  return { type: item.type, value: item.value } as PromptCondition;
}
export function conditionFromRules(rules: Rules): PromptCondition | null {
  let included: PromptCondition | null = null;
  for (const item of rules.include) included = included ? { type: item.join === "or" ? "any" : "all", conditions: [included, atom(item)] } : atom(item);
  const excluded: PromptCondition | null = rules.exclude.length ? { type: "not", condition: rules.exclude.length === 1 ? atom(rules.exclude[0]!) : { type: "any", conditions: rules.exclude.map(atom) } } : null;
  return included && excluded ? { type: "all", conditions: [included, excluded] } : included ?? excluded;
}
export const ruleDefinition = (type: string) => RULES.find((item) => item.type === type);
export const defaultValue = (def: RuleDef): RuleValue => def.shape.kind === "flag" ? true : def.shape.kind === "number" ? Math.max(def.shape.min, 6) : def.shape.kind === "words" ? [] : def.shape.options[0]!.value;
type Translate = (key: string, values?: Record<string, unknown>) => string;
export function describeCondition(condition: PromptCondition, t: Translate): string {
  if (condition.type === "all" || condition.type === "any") return condition.conditions.map((item) => `(${describeCondition(item, t)})`).join(` ${t(condition.type === "all" ? "and" : "or")} `);
  if (condition.type === "not") return `${t("Except when")} ${describeCondition(condition.condition, t)}`;
  const definition = ruleDefinition(condition.type)!;
  const value = "values" in condition ? condition.values : condition.value;
  const text = typeof value === "boolean" ? t(value ? "Yes" : "No") : Array.isArray(value) ? value.join(", ") : definition.shape.kind === "choice" ? t(definition.shape.options.find((option) => option.value === value)?.label ?? String(value)) : String(value);
  return `${t(definition.label)}: ${text}`;
}
export function describeRules(rules: Rules, t: Translate): string {
  const condition = conditionFromRules(rules);
  return condition ? describeCondition(condition, t) : t("Always");
}
export function contradictions(rules: Rules): boolean {
  if (rules.include.some((item, index) => index > 0 && item.join === "or")) return false;
  const seen = new Map<string, RuleValue>();
  for (const item of rules.include) {
    if (item.type === "group" || Array.isArray(item.value) || typeof item.value === "number") continue;
    if (seen.has(item.type) && seen.get(item.type) !== item.value) return true;
    seen.set(item.type, item.value);
  }
  return rules.exclude.some((item) => item.type !== "group" && rules.include.some((included) => included.type === item.type && JSON.stringify(included.value) === JSON.stringify(item.value)));
}
