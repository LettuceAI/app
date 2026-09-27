export const queryKeys = {
  app: {
    all: ["app"] as const,
    status: () => [...queryKeys.app.all, "status"] as const,
  },
};
