/// <reference types="vite/client" />

declare global {
  interface Window {
    __debug?: {
      tourResets?: Record<string, () => Promise<void>>;
      resetTour?: (id: string) => Promise<void>;
      resetAllTours?: () => Promise<void>;
      [key: string]: unknown;
    };
  }
}

export {};
