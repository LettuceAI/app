import { useId } from "react";
import { SelectMenu, type SingleSelectMenuProps } from "@lettuceai/crisp";

export function SelectMenuField({ "aria-label": label, id, ...props }: SingleSelectMenuProps<string> & { "aria-label": string }) {
  const generatedId = useId();
  const controlId = id ?? generatedId;
  return <>
    <label htmlFor={controlId} className="sr-only">{label}</label>
    <SelectMenu {...props} id={controlId} className={`w-full ${props.className ?? ""}`} />
  </>;
}
