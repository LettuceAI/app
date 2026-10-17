import type { ApiClient } from "@/api/client";
import type { CharacterForm } from "../data/form";

export interface StepProps {
  api: ApiClient;
  form: CharacterForm;
  update: (change: (form: CharacterForm) => CharacterForm) => void;
}
