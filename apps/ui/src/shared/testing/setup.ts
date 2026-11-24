import { cleanup, configure } from "@testing-library/react";
import { notifyManager } from "@tanstack/react-query";
import { afterEach } from "vitest";

configure({ asyncUtilTimeout: 5_000 });

let pendingNotifications = 0;
notifyManager.setScheduler((notify) => {
  pendingNotifications += 1;
  setTimeout(() => {
    try {
      notify();
    } finally {
      pendingNotifications -= 1;
    }
  }, 0);
});

afterEach(async () => {
  cleanup();
  // Drain queued observers while the jsdom window is still available.
  while (pendingNotifications > 0) {
    await new Promise<void>((resolve) => setTimeout(resolve, 0));
  }
});

Object.defineProperty(window, "scrollTo", { value: () => {}, writable: true, configurable: true });

Object.defineProperty(window, "matchMedia", {
  writable: true,
  configurable: true,
  value: (query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addEventListener: () => {},
    removeEventListener: () => {},
    addListener: () => {},
    removeListener: () => {},
    dispatchEvent: () => false,
  }),
});
Object.defineProperty(Element.prototype, "scrollTo", { value: () => {}, writable: true, configurable: true });
Object.defineProperty(Element.prototype, "scrollIntoView", { value: () => {}, writable: true, configurable: true });

class ResizeObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
}
Object.defineProperty(window, "ResizeObserver", { value: ResizeObserverStub, writable: true, configurable: true });
Object.defineProperty(globalThis, "ResizeObserver", { value: ResizeObserverStub, writable: true, configurable: true });
