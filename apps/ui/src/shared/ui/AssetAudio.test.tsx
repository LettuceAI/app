import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { I18nextProvider } from "react-i18next";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { createTestI18n } from "@/shared/testing/i18n";
import { AssetAudio } from "./AssetAudio";

beforeEach(() => {
  vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response("wav")));
  vi.stubGlobal("URL", class extends URL {
    static override createObjectURL = vi.fn(() => "blob:recording");
    static override revokeObjectURL = vi.fn();
  });
  vi.spyOn(HTMLMediaElement.prototype, "play").mockImplementation(function (this: HTMLMediaElement) {
    fireEvent.play(this);
    return Promise.resolve();
  });
});
afterEach(() => { cleanup(); vi.unstubAllGlobals(); vi.restoreAllMocks(); });

async function mount(src = "lettuce-asset://localhost/recording") {
  const i18n = await createTestI18n();
  const title = "Recording";
  const view = render(<I18nextProvider i18n={i18n}><AssetAudio src={src} title={title} /></I18nextProvider>);
  return { ...view, show: (next: string) => view.rerender(<I18nextProvider i18n={i18n}><AssetAudio src={next} title={title} /></I18nextProvider>) };
}

it("fetches native audio on play, starts the Blob source and revokes it on unmount", async () => {
  const view = await mount();
  expect(fetch).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Play Recording" }));
  const audio = view.container.querySelector("audio")!;
  await waitFor(() => expect(audio.getAttribute("src")).toBe("blob:recording"));
  fireEvent.canPlay(audio);
  expect(await screen.findByRole("button", { name: "Pause Recording" })).toBeTruthy();
  view.unmount();
  expect(URL.revokeObjectURL).toHaveBeenCalledWith("blob:recording");
});

it("shows a failed asset load and retries from the play button", async () => {
  vi.mocked(fetch).mockRejectedValueOnce(new TypeError("offline")).mockResolvedValueOnce(new Response("wav"));
  const view = await mount();
  fireEvent.click(screen.getByRole("button", { name: "Play Recording" }));
  expect(await screen.findByText("It isn't available right now.")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Play Recording" }));
  await waitFor(() => expect(view.container.querySelector("audio")?.getAttribute("src")).toBe("blob:recording"));
  expect(screen.queryByText("It isn't available right now.")).toBeNull();
  expect(fetch).toHaveBeenCalledTimes(2);
});

it("surfaces play rejection, drops a failed Blob and permits a new attempt", async () => {
  vi.mocked(HTMLMediaElement.prototype.play).mockRejectedValueOnce(new DOMException("Unsupported", "NotSupportedError"));
  const view = await mount();
  fireEvent.click(screen.getByRole("button", { name: "Play Recording" }));
  const audio = view.container.querySelector("audio")!;
  await waitFor(() => expect(audio.getAttribute("src")).toBe("blob:recording"));
  await act(async () => { fireEvent.canPlay(audio); });
  expect(await screen.findByText("It isn't available right now.")).toBeTruthy();
  expect(URL.revokeObjectURL).toHaveBeenCalledWith("blob:recording");
  expect(audio.getAttribute("src")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Play Recording" }));
  await waitFor(() => expect(fetch).toHaveBeenCalledTimes(2));
});

it("releases the old source when another recording replaces it", async () => {
  const view = await mount();
  fireEvent.click(screen.getByRole("button", { name: "Play Recording" }));
  await waitFor(() => expect(view.container.querySelector("audio")?.getAttribute("src")).toBe("blob:recording"));
  view.show("lettuce-asset://localhost/another");
  expect(URL.revokeObjectURL).toHaveBeenCalledWith("blob:recording");
  expect(view.container.querySelector("audio")?.getAttribute("src")).toBeNull();
  expect(fetch).toHaveBeenCalledTimes(1);
});

it("reports decoder errors and clears the lazy player so play can retry", async () => {
  const view = await mount();
  fireEvent.click(screen.getByRole("button", { name: "Play Recording" }));
  const audio = view.container.querySelector("audio")!;
  await waitFor(() => expect(audio.getAttribute("src")).toBe("blob:recording"));
  Object.defineProperty(audio, "error", { value: { code: 4, message: "unsupported" }, configurable: true });
  fireEvent.error(audio);
  expect(await screen.findByText("It isn't available right now.")).toBeTruthy();
  expect(audio.getAttribute("src")).toBeNull();
  expect(URL.revokeObjectURL).toHaveBeenCalledWith("blob:recording");
  fireEvent.click(screen.getByRole("button", { name: "Play Recording" }));
  await waitFor(() => expect(fetch).toHaveBeenCalledTimes(2));
});
