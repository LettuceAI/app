import { Field, Segmented, Select, SelectMenu } from "@lettuceai/crisp";

export type PickOptions<T extends string> = readonly { value: T; label: string; description?: string }[];

/** One choice of a settings row: a small segmented control for up to three options, a menu for more; `native` uses the platform select (phones), `block` fills the row. */
export function SettingsPick<T extends string>({
  value,
  options,
  onChange,
  label,
  menu,
  block,
  native,
}: {
  value: T;
  options: PickOptions<T>;
  onChange: (value: T) => void;
  label: string;
  menu?: boolean;
  block?: boolean;
  native?: boolean;
}) {
  if (menu || options.length > 3) {
    if (native) {
      return (
        <Field label={<span className="sr-only">{label}</span>} className="w-full space-y-0">
          <Select
            className="min-h-11 w-full text-ellipsis"
            value={value}
            options={options}
            onChange={(event) => onChange(event.target.value as T)}
          />
        </Field>
      );
    }
    return (
      <Field label={<span className="sr-only">{label}</span>} className="space-y-0">
        <SelectMenu<T>
          className={block ? "w-full" : "w-40"}
          value={value}
          options={options}
          onChange={(v) => v && onChange(v)}
        />
      </Field>
    );
  }
  return (
    <Segmented<T>
      size="sm"
      block={block ?? false}
      aria-label={label}
      value={value}
      options={options}
      onChange={onChange}
    />
  );
}

