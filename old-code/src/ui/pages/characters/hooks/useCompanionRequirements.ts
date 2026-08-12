import { useCallback, useEffect, useState } from "react";

import {
  getMissingModelRequirements,
  type ModelRequirement,
} from "../../../modelRequirements";

export interface CompanionRequirementsState {
  loading: boolean;
  missing: ModelRequirement[];
  refresh: () => Promise<ModelRequirement[]>;
}

export function useCompanionRequirements(): CompanionRequirementsState {
  const [loading, setLoading] = useState(true);
  const [missing, setMissing] = useState<ModelRequirement[]>([]);

  const refresh = useCallback(async (): Promise<ModelRequirement[]> => {
    try {
      const list = await getMissingModelRequirements({ requireCompanion: true });
      setMissing(list);
      return list;
    } catch (err) {
      console.error("Failed to check companion requirements:", err);
      return [];
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return { loading, missing, refresh };
}
