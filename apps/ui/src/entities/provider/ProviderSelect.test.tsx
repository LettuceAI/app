import { useState } from "react";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { I18nextProvider } from "react-i18next";
import { Field } from "@lettuceai/crisp";
import { afterEach, describe, expect, it, vi } from "vitest";
import { createTestI18n } from "@/shared/testing/i18n";
import { ProviderSelect, type ProviderSelectProps } from "./ProviderSelect";

Object.defineProperty(Element.prototype, "scrollIntoView", { value: vi.fn(), writable: true, configurable: true });

const fieldLabel = "Provider";
const fieldHint = "Choose the account to use";
const providers = [
  { id: "router", name: "OpenRouter", kind: "openrouter", group: "Cloud" },
  { id: "local", name: "llama.cpp", kind: "llamacpp", group: "Local" },
  { id: "disabled", name: "Unavailable", kind: "custom", group: "Custom", disabled: true },
];

async function mount(props: Partial<ProviderSelectProps> = {}) {
  const onChange = vi.fn();
  function Picker() {
    const [value, setValue] = useState<string | null>(null);
    return <Field label={fieldLabel} hint={fieldHint}>
      <ProviderSelect providers={providers} value={value} onChange={(id) => { onChange(id); setValue(id); }} {...props} />
    </Field>;
  }
  render(<I18nextProvider i18n={await createTestI18n()}><Picker /></I18nextProvider>);
  return onChange;
}

afterEach(cleanup);

describe("ProviderSelect", () => {
  it("uses ModelSelect's desktop keyboard selection and skips disabled options", async () => {
    const change = await mount({ compact: false });
    const trigger = screen.getByRole("combobox", { name: "Provider" });
    fireEvent.keyDown(trigger, { key: "ArrowDown" });
    await screen.findByRole("listbox");
    fireEvent.keyDown(trigger, { key: "End" });
    fireEvent.keyDown(trigger, { key: "Enter" });
    expect(change).toHaveBeenCalledWith("local");
    expect(trigger.textContent).toContain("llama.cpp");
  });

  it("opens a grouped mobile sheet, selects an account, and shows the selection when reopened", async () => {
    const change = await mount({ compact: true });
    const trigger = screen.getByRole("button", { name: "Provider" });
    expect(trigger.getAttribute("aria-describedby")).toBeTruthy();
    fireEvent.click(trigger);
    const sheet = await screen.findByRole("dialog", { name: "Provider" });
    expect(within(sheet).getByRole("heading", { name: "Local" })).toBeTruthy();
    expect(within(sheet).getByRole("button", { name: "Unavailable" }).hasAttribute("disabled")).toBe(true);
    fireEvent.click(within(sheet).getByRole("button", { name: "llama.cpp" }));
    expect(change).toHaveBeenCalledWith("local");
    expect(trigger.getAttribute("aria-expanded")).toBe("false");
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    fireEvent.click(trigger);
    expect(within(await screen.findByRole("dialog")).getByRole("button", { name: "llama.cpp" }).getAttribute("aria-pressed")).toBe("true");
  });

  it("keeps disabled selectors closed and shows a placeholder for stale values", async () => {
    await mount({ compact: true, disabled: true, value: "deleted-account" });
    const trigger = screen.getByRole("button", { name: "Provider" });
    expect(trigger.textContent).toContain("Choose a provider");
    fireEvent.click(trigger);
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});
