import { useEffect, useMemo, useRef, useState } from "react";
import { useBlocker } from "@tanstack/react-router";
import type { PromptPlaceholders } from "@/api/generated/bindings";
import { usePromptText } from "./copy";
import { blankEntry } from "./data";
import { AnimatePresence, motion } from "framer-motion";
import { ArrowLeft, Braces, Plus, Search, Trash2, X } from "lucide-react";
import {
  Alert, Button, Chip, confirm, Collapsible, EmptyState, Field, IconButton, Input, ListGroup, ListRow, ListRowButton, NumberInput, SearchField, Segmented,
  Select, Sheet, Switch, TagInput, Text, Textarea,
} from "@lettuceai/crisp";
import { cn, icon, motion as m } from "@lettuceai/crisp";
import { contradictions, defaultValue, describeRules, describeCondition, ruleDefinition as RULE } from "./rules";
import { POSITION, SLOT_LABEL, RULES, type RuleDef } from "./presentation";
import type { Entry, ImageSlot, Position, Role, Rule, Rules } from "./types";


type View = "form" | "include" | "exclude";

export function PromptEntrySheet({
  entry, phone, registry, promptDirty, onClose, onDone, onDelete,
}: {
  entry: Entry | "new" | null;
  phone: boolean;
  promptDirty: boolean;
  registry: PromptPlaceholders | undefined;
  onClose: () => void;
  onDone: (entry: Entry) => void;
  onDelete: (entry: Entry) => void;
}) {
  const { t } = usePromptText();
  const existing = entry !== null && entry !== "new" ? entry : null;
  const [seed, setSeed] = useState<Entry | "new" | null>(null);
  const [draft, setDraft] = useState<Entry>(() => blankEntry());
  const [view, setView] = useState<View>("form");
  const [ruleQuery, setRuleQuery] = useState("");
  useBlocker({ shouldBlockFn: async ({ action }) => {
    if (entry === null) return false;
    if (action !== "BACK") {
      const changed = entry === "new" ? !!draft.name || !!draft.content || !!draft.imageSlot : JSON.stringify(draft) !== JSON.stringify(entry);
      return (changed || promptDirty) && !await confirm({ title: t("Discard your changes?"), message: t("Your unsaved prompt and entry changes will be lost."), confirmLabel: t("Discard"), cancelLabel: t("Keep editing"), destructive: true });
    }
    if (view !== "form") setView("form"); else onClose();
    return true;
  }, enableBeforeUnload: entry !== null && (promptDirty || (entry === "new" ? !!draft.name || !!draft.content : JSON.stringify(draft) !== JSON.stringify(entry))) });
  const contentRef = useRef<HTMLTextAreaElement>(null);
  const topRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    let el = topRef.current?.parentElement ?? null;
    while (el && !/(auto|scroll)/.test(getComputedStyle(el).overflowY)) el = el.parentElement;
    el?.scrollTo({ top: 0 });
  }, [view]);
  if (entry !== seed) {
    setSeed(entry);
    setDraft(existing ?? blankEntry());
    setView("form");
    setRuleQuery("");
  }
  const set = <K extends keyof Entry>(key: K, value: Entry[K]) => setDraft((d) => ({ ...d, [key]: value }));
  const setRules = (rules: Rules) => set("rules", rules);

  const slots = registry?.image_slots ?? [];
  const placeholders = registry;
  const problems = contradictions(draft.rules) ? [t("These rules can't both be true")] : [];
  const insert = (token: string) => {
    const el = contentRef.current;
    const at = el?.selectionStart ?? draft.content.length;
    const end = el?.selectionEnd ?? at;
    set("content", draft.content.slice(0, at) + token + draft.content.slice(end));
    requestAnimationFrame(() => {
      el?.focus();
      el?.setSelectionRange(at + token.length, at + token.length);
    });
  };
  const addRule = (def: RuleDef) => {
    const rule: Rule = { id: crypto.randomUUID(), type: def.type, value: defaultValue(def), join: view === "exclude" ? "or" : "and" };
    setRules(view === "exclude" ? { ...draft.rules, exclude: [...draft.rules.exclude, rule] } : { ...draft.rules, include: [...draft.rules.include, rule] });
    setView("form");
    setRuleQuery("");
  };

  const rq = ruleQuery.trim().toLowerCase();
  const ruleGroups = useMemo(() => {
    const by = new Map<string, RuleDef[]>();
    for (const r of RULES) if (!rq || t(r.label).toLowerCase().includes(rq)) by.set(r.group, [...(by.get(r.group) ?? []), r]);
    return [...by.entries()];
  }, [rq, t]);

  const picking = view !== "form";
  const words = draft.content.length;

  const form = (
    <div className="space-y-6">
      <ListGroup>
        <ListRow
          title={t("Include this entry")}
          description={t(draft.required ? "This prompt can't work without it" : draft.enabled ? "Sent when its rules match" : "Kept, but never sent")}
          trailing={<Switch checked={draft.enabled || draft.required} disabled={draft.required} onChange={(on) => set("enabled", on)} aria-label={t("Include this entry")} />}
        />
      </ListGroup>

      <Field label={t("Name")} required hint={t("For you. The model doesn't see it.")}>
        <Input value={draft.name} onChange={(e) => set("name", e.target.value)} placeholder={t("Pacing")} data-autofocus />
      </Field>

      {(slots.length > 0 || draft.imageSlot) && (
        <ListGroup>
          <ListRow
            title={t("Sends")}
            description={t(draft.imageSlot ? "A picture, with the text below as its note" : "Text")}
            trailing={
              <Segmented
                size="sm"
                aria-label={t("Entry sends")}
                value={draft.imageSlot ? "picture" : "text"}
                onChange={(v) => set("imageSlot", v === "picture" ? draft.imageSlot ?? slots[0] ?? null : null)}
                options={[{ value: "text", label: t("Text") }, { value: "picture", label: t("Picture") }]}
              />
            }
          />
          {draft.imageSlot && (
            <ListRow
              title={t("Which picture")}
              description={t("Left out when the chat has none")}
              trailing={<Select aria-label={t("Which picture")} value={draft.imageSlot} onChange={(e) => set("imageSlot", e.target.value as ImageSlot)} options={[...new Set([...slots, ...(draft.imageSlot ? [draft.imageSlot] : [])])].map((s) => ({ value: s, label: t(SLOT_LABEL[s]) }))} />}
            />
          )}
        </ListGroup>
      )}

      <Field label={t(draft.imageSlot ? "Note with the picture" : "Content")} hint={t("Content character hint", { count: words })}>
        <Textarea ref={contentRef} rows={phone ? 7 : 10} value={draft.content} onChange={(e) => set("content", e.target.value)} className="font-mono text-sm" placeholder={t("Let the story breathe. One beat per reply.")} />
      </Field>

      {placeholders && (
        <Collapsible variant="plain" title={t("Placeholders")} description={t("{{count}} for this kind of prompt. Tap one to insert it.", { count: placeholders.allowed.length })}>
          <div className="space-y-2 pt-2">
            <div className="flex flex-wrap gap-1.5">
              {placeholders.allowed.map((p) => {
                const used = draft.content.includes(p);
                return (
                  <Chip key={p} selected={used} onClick={() => insert(p)} icon={<Braces size={icon.xs} />} title={p}>
                    <span className="font-mono text-xs">{p.slice(2, -2)}</span>
                  </Chip>
                );
              })}
            </div>
            {placeholders.required.length > 0 && (
              <Text variant="hint">{t("The prompt needs {{names}} somewhere in its included entries.", { names: placeholders.required.map((p) => p.slice(2, -2)).join(", ") })}</Text>
            )}
          </div>
        </Collapsible>
      )}

      <ListGroup label={t("Placement")}>
        <ListRow
          title={t("Sent as")}
          description={t(draft.role === "system" ? "An instruction" : draft.role === "user" ? "As if you wrote it" : "As if the character wrote it")}
          trailing={
            <Segmented size="sm" aria-label={t("Role")} value={draft.role} onChange={(v) => set("role", v as Role)} options={[{ value: "system", label: t("System") }, { value: "user", label: t("User") }, { value: "assistant", label: t("Assistant") }]} />
          }
        />
        <ListRow
          title={t("Where")}
          description={t(POSITION[draft.position].hint)}
          trailing={
            <Select
              aria-label={t("Where")}
              value={draft.position}
              onChange={(e) => {
                const p = e.target.value as Position;
                setDraft((d) => ({ ...d, position: p, minMessages: p === "conditional" ? d.minMessages ?? 6 : d.minMessages, everyTurns: p === "interval" ? d.everyTurns ?? 3 : d.everyTurns }));
              }}
              options={(Object.keys(POSITION) as Position[]).map((p) => ({ value: p, label: t(POSITION[p].label) }))}
            />
          }
        />
        {draft.position === "conditional" && (
          <ListRow title={t("Starting at message")} description={t("Left out until the chat is this long")} trailing={<NumberInput aria-label={t("Starting at message")} value={draft.minMessages ?? 1} min={1} onChange={(v) => set("minMessages", v)} />} />
        )}
        {draft.position === "interval" && (
          <ListRow title={t("Every")} description={t("Messages between each time it comes back")} trailing={<NumberInput aria-label={t("Every")} value={draft.everyTurns ?? 3} min={1} suffix={t("messages")} onChange={(v) => set("everyTurns", v)} />} />
        )}
        {draft.position !== "relative" && (
          <ListRow title={t("Distance from the newest")} description={draft.depth === 0 ? t("Right after the newest message") : t("{{count}} messages back from the newest", { count: draft.depth })} trailing={<NumberInput aria-label={t("Distance from the newest")} value={draft.depth} min={0} onChange={(v) => set("depth", v)} />} />
        )}
      </ListGroup>

      <RuleList
        label={t("Include when")}
        empty={t("Always included")}
        rules={draft.rules.include}
        joins
        onChange={(include) => setRules({ ...draft.rules, include })}
        onAdd={() => setView("include")}
        addLabel={t("Add a rule")}
      />
      <RuleList
        label={t("Except when")}
        empty={t("No exceptions")}
        rules={draft.rules.exclude}
        onChange={(exclude) => setRules({ ...draft.rules, exclude })}
        onAdd={() => setView("exclude")}
        addLabel={t("Add an exception")}
      />
      <div className="space-y-3 px-1">
        <Text variant="hint">{t("{{rules}}.", { rules: describeRules(draft.rules, t) })}</Text>
        {problems.length > 0 && <Alert tone="danger" title={t("These rules can't both be true")} description={problems.join(". ")} />}
      </div>
    </div>
  );

  const ruleList = (
    <div className="space-y-5">
      <SearchField value={ruleQuery} onChange={setRuleQuery} placeholder={t("Search rules")} />
      {ruleGroups.length === 0 ? (
        <EmptyState size="sm" icon={<Search size={icon.lg} />} title={t("No rule matches")} />
      ) : (
        ruleGroups.map(([group, list]) => (
          <ListGroup key={group} label={t(group)}>
            {list.map((r) => <ListRowButton key={r.type} title={t(r.label)} chevron={false} onClick={() => addRule(r)} />)}
          </ListGroup>
        ))
      )}
    </div>
  );

  return (
    <Sheet
      open={entry !== null}
      onClose={onClose}
      placement={phone ? "bottom" : "side"}
      size="lg"
      title={t(picking ? (view === "exclude" ? "Add an exception" : "Add a rule") : existing ? "Edit entry" : "New entry")}
      leftAction={picking ? <IconButton label={t("Back to the entry")} shape="round" onClick={() => setView("form")}><ArrowLeft size={icon.lg} /></IconButton> : undefined}
      footer={
        picking ? undefined : (
          <div className="flex items-center gap-2">
            {existing && !draft.required && <IconButton label={t("Remove entry")} onClick={() => onDelete(existing)}><Trash2 size={icon.md} className="text-danger" /></IconButton>}
            <span className="flex-1" />
            <Button variant="ghost" onClick={onClose}>{t("Cancel")}</Button>
            <Button variant="primary" disabled={!draft.name.trim() || (!draft.content.trim() && !draft.imageSlot) || problems.length > 0} onClick={() => onDone(draft)}>{t("Done")}</Button>
          </div>
        )
      }
    >
      <div ref={topRef} />
      <AnimatePresence mode="wait" initial={false}>
        <motion.div key={picking ? "rules" : "form"} initial={{ opacity: 0, x: picking ? 12 : -12 }} animate={{ opacity: 1, x: 0 }} exit={{ opacity: 0 }} transition={m.quick}>
          {picking ? ruleList : form}
        </motion.div>
      </AnimatePresence>
    </Sheet>
  );
}

function RuleList({
  label, empty, rules, joins, onChange, onAdd, addLabel,
}: {
  label: string;
  empty: string;
  rules: Rule[];
  joins?: boolean;
  onChange: (rules: Rule[]) => void;
  onAdd: () => void;
  addLabel: string;
}) {
  const { t } = usePromptText();
  const patch = (id: string, next: Partial<Rule>) => onChange(rules.map((r) => (r.id === id ? { ...r, ...next } : r)));
  const remove = (id: string) => onChange(rules.filter((r) => r.id !== id));

  return (
    <ListGroup label={t(label)}>
      {rules.length === 0 && <ListRow title={t(empty)} description={t(joins ? "Add a rule to send it only sometimes" : "Add one to leave it out in some chats")} />}
      <AnimatePresence initial={false}>
        {rules.map((rule, i) => {
          const def = RULE(rule.type);
          if (!def) return <ListRow key={rule.id} title={t("Grouped rules")} description={rule.condition ? describeCondition(rule.condition, t) : undefined} trailing={<IconButton label={t("Remove grouped rules")} size="sm" onClick={() => remove(rule.id)}><X size={icon.sm} /></IconButton>} />;
          const control =
            def.shape.kind === "flag" ? (
              <Segmented size="sm" aria-label={t(def.label)} value={rule.value ? "yes" : "no"} onChange={(v) => patch(rule.id, { value: v === "yes" })} options={[{ value: "yes", label: t("Yes") }, { value: "no", label: t("No") }]} />
            ) : def.shape.kind === "choice" ? (
              <Segmented size="sm" aria-label={t(def.label)} value={String(rule.value)} onChange={(v) => patch(rule.id, { value: v })} options={def.shape.options.map((option) => ({ ...option, label: t(option.label) }))} />
            ) : def.shape.kind === "number" ? (
              <NumberInput aria-label={t(def.label)} value={Number(rule.value)} min={def.shape.min} suffix={t(def.shape.unit)} onChange={(v) => patch(rule.id, { value: v })} />
            ) : null;
          return (
            <motion.div key={rule.id} initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: "auto" }} exit={{ opacity: 0, height: 0 }} transition={m.settle} className="overflow-hidden">
              <ListRow
                title={
                  <span className="flex min-w-0 items-center gap-2">
                    {joins && i > 0 && (
                      <Chip onClick={() => patch(rule.id, { join: rule.join === "and" ? "or" : "and" })} aria-label={t("Joined with {{join}}, switch", { join: t(rule.join) })} className="shrink-0">
                        {t(rule.join)}
                      </Chip>
                    )}
                    {!joins && i > 0 && <span className="shrink-0 text-2xs font-medium uppercase tracking-wide text-fg-3">{t("or")}</span>}
                    <span className="truncate">{t(def.label)}</span>
                  </span>
                }
                description={
                  def.shape.kind === "words" ? (
                    <TagInput className="mt-2" value={rule.value as string[]} onChange={(v) => patch(rule.id, { value: v })} placeholder={t("{{example}}, press Enter after each", { example: t(def.shape.placeholder) })} />
                  ) : undefined
                }
                trailing={
                  <span className={cn("flex items-center gap-1", def.shape.kind === "words" && "self-start")}>
                    {control}
                    <IconButton label={t("Remove {{name}}", { name: t(def.label) })} size="sm" onClick={() => remove(rule.id)}><X size={icon.sm} /></IconButton>
                  </span>
                }
              />
            </motion.div>
          );
        })}
      </AnimatePresence>
      <ListRowButton icon={<Plus size={icon.lg} />} title={t(addLabel)} chevron={false} onClick={onAdd} />
    </ListGroup>
  );
}
