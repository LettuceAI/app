import type { ApiClient } from "@/api/client";
import type { CharacterForm } from "../data/form";

export interface FormProps {
  form: CharacterForm;
  update: (change: (form: CharacterForm) => CharacterForm) => void;
}

export interface StepProps extends FormProps {
  api: ApiClient;
}
