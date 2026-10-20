export interface OperationIds {
  idFor: (key: string) => string;
  settle: (key: string) => void;
}

/** One operation id per user action: asking again for the same action reuses its id until the action settles. */
export function createOperationIds(): OperationIds {
  const ids = new Map<string, string>();
  return {
    idFor(key) {
      let id = ids.get(key);
      if (id === undefined) {
        id = crypto.randomUUID();
        ids.set(key, id);
      }
      return id;
    },
    settle(key) {
      ids.delete(key);
    },
  };
}
