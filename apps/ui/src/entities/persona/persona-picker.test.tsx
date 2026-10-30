import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { I18nextProvider } from "react-i18next";
import { afterEach, expect, it, vi } from "vitest";
import type { PersonaSummary } from "@/api/generated/bindings";
import { createTestI18n } from "@/shared/testing/i18n";
import { PersonaSheet, type PersonaSheetProps } from "./index";

afterEach(cleanup);

async function mount(patch: Partial<PersonaSheetProps> = {}) {
  const props: PersonaSheetProps = {
    open: true, personas: { items: [], defaultId: null }, status: "success",
    selectedId: null, onClose: vi.fn(), onRetry: vi.fn(), onSelect: vi.fn(),
    defaultChoice: { title: "Use the default", description: "Your default persona", selected: true, onSelect: vi.fn() },
    ...patch,
  };
  const i18n = await createTestI18n("en");
  const view = render(<I18nextProvider i18n={i18n}><PersonaSheet {...props} /></I18nextProvider>);
  return { props, update: (next: Partial<PersonaSheetProps>) => { Object.assign(props, next); view.rerender(<I18nextProvider i18n={i18n}><PersonaSheet {...props} /></I18nextProvider>); } };
}

it("keeps inherited and disabled persona choices separate even with an empty library", async () => {
  const { props } = await mount();
  const dialog = await screen.findByRole("dialog", { name: "Persona" });
  const inherited = within(dialog).getByRole("button", { name: /^Use the default/ });
  const none = within(dialog).getByRole("button", { name: "No persona" });
  expect(inherited.getAttribute("aria-pressed")).toBe("true");
  expect(none.getAttribute("aria-pressed")).toBe("false");
  fireEvent.click(none);
  expect(props.onSelect).toHaveBeenCalledWith(null);
  fireEvent.click(inherited);
  expect(props.defaultChoice!.onSelect).toHaveBeenCalledTimes(1);
});

it("waits for personas before offering a selection and retries a failed load", async () => {
  const { props, update } = await mount({ status: "pending", personas: undefined });
  expect(screen.queryByRole("button", { name: "No persona" })).toBeNull();
  update({ status: "error" });
  fireEvent.click(await screen.findByRole("button", { name: "Try again" }));
  expect(props.onRetry).toHaveBeenCalledTimes(1);
  update({ status: "success", personas: { items: [], defaultId: null } });
  expect(await screen.findByRole("button", { name: "No persona" })).toBeTruthy();
});

it("selects an explicit persona and clears the search when reopened", async () => {
  const items = ["Sailor", "Scholar", "Mechanic", "Traveler"].map((title, index): PersonaSummary => ({ id: String(index), title, nickname: "", description: "", avatar: null }));
  const { props, update } = await mount({ personas: { items, defaultId: "0" }, selectedId: "1" });
  fireEvent.change(await screen.findByRole("searchbox"), { target: { value: "Mechanic" } });
  fireEvent.click(screen.getByText("Mechanic").closest("button")!);
  expect(props.onSelect).toHaveBeenCalledWith("2");
  fireEvent.change(screen.getByRole("searchbox"), { target: { value: "Scholar" } });
  update({ open: false });
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  update({ open: true });
  expect((await screen.findByRole("searchbox")) as HTMLInputElement).toHaveProperty("value", "");
  expect(screen.getByText("Mechanic").closest("button")!).toBeTruthy();
});
