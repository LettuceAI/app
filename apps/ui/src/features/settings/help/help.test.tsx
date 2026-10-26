import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { I18nextProvider } from "react-i18next";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { Toaster, toast } from "@lettuceai/crisp";
import { ApiFailure } from "@/api/client";
import { openExternalUrl } from "@/api/external-links";
import help from "@/shared/i18n/locales/en/help.json";
import { createTestI18n } from "@/shared/testing/i18n";
import { HelpPage } from "./HelpPage";

vi.mock("@/api/external-links", () => ({ openExternalUrl: vi.fn().mockResolvedValue(undefined) }));
vi.mock("../data", () => ({ useSettingsLayout: () => ({ back: {} }) }));

beforeEach(() => vi.mocked(openExternalUrl).mockReset().mockResolvedValue(undefined));
afterEach(() => {
  toast.dismiss();
  cleanup();
  vi.restoreAllMocks();
});

async function mount(language: "en" | "cimode" = "en") {
  const queryClient = new QueryClient({ defaultOptions: { mutations: { retry: false } } });
  const i18n = await createTestI18n(language);
  i18n.addResourceBundle("en", "help", help, true, true);
  render(
    <QueryClientProvider client={queryClient}>
      <I18nextProvider i18n={i18n}>
        <HelpPage />
        <Toaster />
      </I18nextProvider>
    </QueryClientProvider>,
  );
}

it("preserves every FAQ topic and opens only one answer across all sections", async () => {
  await mount();
  expect(
    screen.getAllByRole("button").filter((button) => button.hasAttribute("aria-expanded")),
  ).toHaveLength(21);
  for (const section of Object.values(help.sections))
    expect(screen.getByText(section)).toBeTruthy();
  const first = screen.getByRole("button", { name: help.faq.whatIsLettuceai.question });
  const privacy = screen.getByRole("button", { name: help.faq.chatsStored.question });
  expect(first.getAttribute("aria-expanded")).toBe("true");
  expect(privacy.getAttribute("aria-expanded")).toBe("false");
  fireEvent.click(privacy);
  expect(privacy.getAttribute("aria-expanded")).toBe("true");
  expect(first.getAttribute("aria-expanded")).toBe("false");
  expect(await screen.findByText(help.faq.chatsStored.answer)).toBeTruthy();
  fireEvent.click(privacy);
  expect(privacy.getAttribute("aria-expanded")).toBe("false");
});

it("opens docs and Discord through the shared opener and disables duplicate requests", async () => {
  let finish!: () => void;
  vi.mocked(openExternalUrl).mockImplementationOnce(
    () =>
      new Promise<void>((resolve) => {
        finish = resolve;
      }),
  );
  await mount();
  const docs = screen.getByRole("button", { name: /^Full documentation/ });
  const discord = screen.getByRole("button", { name: /^Join the Discord/ });
  fireEvent.click(docs);
  await waitFor(() => expect(docs.hasAttribute("disabled")).toBe(true));
  fireEvent.click(docs);
  expect(openExternalUrl).toHaveBeenCalledTimes(1);
  expect(openExternalUrl).toHaveBeenCalledWith("https://www.lettuceai.app/docs");
  await act(async () => finish());
  await waitFor(() => expect(discord.hasAttribute("disabled")).toBe(false));
  fireEvent.click(discord);
  await waitFor(() =>
    expect(openExternalUrl).toHaveBeenLastCalledWith("https://discord.gg/745bEttw2r"),
  );
});

it("shows a typed opener error and retries the same destination", async () => {
  vi.mocked(openExternalUrl).mockRejectedValueOnce(
    new ApiFailure("unavailable", "private opener diagnostic", null),
  );
  await mount();
  fireEvent.click(screen.getByRole("button", { name: /^Full documentation/ }));
  expect(await screen.findByText("Couldn't open the link")).toBeTruthy();
  expect(screen.queryByText("private opener diagnostic")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Try again" }));
  await waitFor(() => expect(openExternalUrl).toHaveBeenCalledTimes(2));
  expect(openExternalUrl).toHaveBeenLastCalledWith("https://www.lettuceai.app/docs");
});

it("localizes the page, questions, answers and external actions", async () => {
  await mount("cimode");
  expect(screen.getByRole("heading", { name: "title", level: 1 })).toBeTruthy();
  expect(screen.getByRole("button", { name: "faq.whatIsApiKey.question" })).toBeTruthy();
  expect(screen.getByText("faq.whatIsLettuceai.answer")).toBeTruthy();
  expect(screen.getByRole("button", { name: /stillStuck.docsTitle/ })).toBeTruthy();
  expect(screen.queryByText(help.faq.whatIsLettuceai.answer)).toBeNull();
});
