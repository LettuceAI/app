import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { I18nextProvider } from "react-i18next";
import { afterEach, expect, it, vi } from "vitest";
import { createApiClient } from "@/api/client";
import type { ModelView, ProviderAccountView } from "@/api/generated/bindings";
import { createMockTransport } from "@/api/mock-transport";
import { createTestI18n } from "@/shared/testing/i18n";
import { ModelChoiceSheet, type ModelChoiceSheetProps } from "./ModelChoiceSheet";

afterEach(cleanup);
const sheetTitle = "Model";
const ok = <T,>(data: T) => ({ status: "ok" as const, data });
const account = (id: string, provider_kind: string, enabled = true): ProviderAccountView => ({
  id,
  label: id,
  provider_kind,
  enabled,
  protocol: "open_ai_compatible",
  base_url: null,
  streaming_enabled: true,
  allow_invalid_tls: false,
  api_key_set: true,
  config: {},
  revision: 1,
  created_at: 1,
  updated_at: 1,
});
const model = (id: string, provider_account_id: string, kind: ModelView["kind"] = "chat"): ModelView => ({
  id,
  provider_account_id,
  display_name: id,
  external_model_id: id,
  kind,
  input_scopes: ["text"],
  output_scopes: kind === "image" ? ["image"] : ["text"],
  config: {},
  revision: 1,
  created_at: 1,
  updated_at: 1,
});
const accounts = [account("Cloud", "openai"), account("Device", "llamacpp"), account("Off", "openrouter", false)];
const models = [
  ...["Alpha", "Bravo", "Charlie", "Delta", "Echo", "Foxtrot"].map((id) => model(id, "Cloud")),
  model("Gamma local", "Device"),
  model("Hotel local", "Device"),
  model("Sleeper", "Off"),
  model("Painter", "Cloud", "image"),
];

async function mount(props: Partial<ModelChoiceSheetProps> = {}, ready = "Cloud") {
  const api = createApiClient(
    createMockTransport({
      calls: {
        modelsList: () => ok({ models, default_model_id: null, revision: 1 }),
        providerAccountsList: () => ok(accounts),
      },
    }),
  );
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const i18n = await createTestI18n();
  const onChange = vi.fn();
  const onClose = vi.fn();
  await act(async () => {
    render(
      <QueryClientProvider client={client}>
        <I18nextProvider i18n={i18n}>
          <ModelChoiceSheet
            api={api}
            open
            title={sheetTitle}
            value={null}
            requirement={{ kind: "chat" }}
            nullChoice={{ title: "Use the default" }}
            onChange={onChange}
            onClose={onClose}
            {...props}
          />
        </I18nextProvider>
      </QueryClientProvider>,
    );
  });
  await screen.findByRole("button", { name: ready });
  return { onChange, onClose };
}

it("groups on-device models first and hides models the requirement rules out", async () => {
  await mount();
  const titles = screen.getAllByRole("button", { expanded: true }).map((button) => button.textContent);
  expect(titles).toEqual(["On device", "Cloud", "Off"]);
  expect(screen.queryByText("Painter")).toBeNull();
});

it("collapses and expands a provider group when its title is tapped", async () => {
  await mount();
  const title = screen.getByRole("button", { name: "Cloud" });
  expect(screen.getByRole("button", { name: /^Alpha/ })).toBeTruthy();
  fireEvent.click(title);
  await waitFor(() => expect(screen.queryByRole("button", { name: /^Alpha/ })).toBeNull());
  expect(title.getAttribute("aria-expanded")).toBe("false");
  expect(screen.getByRole("button", { name: /^Gamma local/ })).toBeTruthy();
  fireEvent.click(title);
  expect(await screen.findByRole("button", { name: /^Alpha/ })).toBeTruthy();
});

it("searches forgivingly, expands every group and keeps toggles working", async () => {
  await mount();
  fireEvent.click(screen.getByRole("button", { name: "Cloud" }));
  await waitFor(() => expect(screen.queryByRole("button", { name: /^Alpha/ })).toBeNull());
  fireEvent.change(screen.getByRole("searchbox", { name: "Search models" }), { target: { value: "brvo" } });
  expect(await screen.findByRole("button", { name: /^Bravo/ })).toBeTruthy();
  expect(screen.queryByRole("button", { name: /^Alpha/ })).toBeNull();
  expect(screen.queryByRole("button", { name: "Off" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Cloud" }));
  await waitFor(() => expect(screen.queryByRole("button", { name: /^Bravo/ })).toBeNull());
  fireEvent.change(screen.getByRole("searchbox", { name: "Search models" }), { target: { value: "zzzz" } });
  expect(await screen.findByText("No matching results")).toBeTruthy();
});

it("marks the null row when nothing is chosen and the model row otherwise", async () => {
  await mount();
  expect(screen.getByRole("button", { name: /^Use the default/ }).getAttribute("aria-pressed")).toBe("true");
  expect(screen.getByRole("button", { name: /^Alpha/ }).getAttribute("aria-pressed")).toBe("false");
  cleanup();
  await mount({ value: "Alpha" });
  expect(screen.getByRole("button", { name: /^Use the default/ }).getAttribute("aria-pressed")).toBe("false");
  expect(screen.getByRole("button", { name: /^Alpha/ }).getAttribute("aria-pressed")).toBe("true");
});

it("lets the caller override the null row check", async () => {
  await mount({ value: "Alpha", nullChoice: { title: "Use the default", selected: true } });
  expect(screen.getByRole("button", { name: /^Use the default/ }).getAttribute("aria-pressed")).toBe("true");
});

it("disables a model of a provider that is turned off", async () => {
  await mount();
  const row = screen.getByRole("button", { name: /^Sleeper/ });
  expect((row as HTMLButtonElement).disabled).toBe(true);
  expect(row.textContent).toContain("Provider turned off");
});

it("closes after a choice unless the caller declines it", async () => {
  const { onChange, onClose } = await mount();
  fireEvent.click(screen.getByRole("button", { name: /^Alpha/ }));
  await waitFor(() => expect(onClose).toHaveBeenCalledTimes(1));
  expect(onChange).toHaveBeenCalledWith("Alpha");
  onChange.mockReturnValue(false);
  fireEvent.click(screen.getByRole("button", { name: /^Use the default/ }));
  await waitFor(() => expect(onChange).toHaveBeenLastCalledWith(null));
  expect(onClose).toHaveBeenCalledTimes(1);
});

it("offers adding a model when nothing meets the requirement", async () => {
  const onAddModel = vi.fn();
  await mount({ requirement: { kind: "speech" }, onAddModel }, "Add a model");
  expect(screen.getByText("No models for this yet")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Add a model" }));
  expect(onAddModel).toHaveBeenCalledTimes(1);
});
