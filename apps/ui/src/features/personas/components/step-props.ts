import type { ApiClient } from "@/api/client";
import type { PersonaForm } from "../data/form";

export interface StepProps {
  api: ApiClient;
  form: PersonaForm;
  update: (change: (form: PersonaForm) => PersonaForm) => void;
}
