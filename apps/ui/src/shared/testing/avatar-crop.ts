import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { vi } from "vitest";
import type { MockCallHandlers } from "@/api/mock-transport";

export function mockAvatarCropImages(failures = 0) {
  const Image = globalThis.Image;
  let remaining = failures;
  vi.spyOn(globalThis, "Image").mockImplementation(function () {
    const image = new Image();
    Object.defineProperties(image, {
      naturalWidth: { value: 800 },
      naturalHeight: { value: 600 },
      src: { get: () => image.getAttribute("src") ?? "", set: (value: string) => {
        image.setAttribute("src", value);
        const fail = image.crossOrigin !== null || remaining > 0;
        if (image.crossOrigin === null && remaining > 0) remaining -= 1;
        queueMicrotask(() => image.dispatchEvent(new Event(fail ? "error" : "load")));
      } },
    });
    return image;
  } as unknown as typeof Image);
}

export const mockAvatarCrop: NonNullable<MockCallHandlers["assetsCropAvatar"]> = ({ asset_id }) => ({
  status: "ok", data: { asset_id: `${asset_id}-cropped`, url: `asset://${asset_id}-cropped`, preview_url: null },
});

export async function confirmAvatarCrop(title = "Position avatar") {
  const dialog = await screen.findByRole("dialog", { name: title });
  const use = within(dialog).getByRole("button", { name: "Use this position" }) as HTMLButtonElement;
  await waitFor(() => { if (use.disabled) throw new Error("avatar image is not ready"); });
  fireEvent.click(use);
}
