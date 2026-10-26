let checks = 0;
const listeners = new Set<() => void>();

/** Asks the app to check, after a restore, which models the restored data needs. */
export function checkRestoredModels() {
  checks += 1;
  for (const listener of listeners) listener();
}

export function subscribeRestoredModels(listener: () => void) {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

export function restoredModelChecks() {
  return checks;
}
