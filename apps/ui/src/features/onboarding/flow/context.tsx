import { createContext, useContext, type Dispatch } from "react";
import type { UseQueryResult } from "@tanstack/react-query";
import type { ProviderDescriptorContract } from "@/api/generated/bindings";
import type { ConnectError } from "../data/connect";
import type { FlowAction, FlowState, GuideLengths } from "./flow";

export interface ActionController<E = unknown> {
  busy: boolean;
  error: E | null;
  run: () => void;
}

export interface OnboardingContextValue {
  state: FlowState;
  dispatch: Dispatch<FlowAction>;
  phone: boolean;
  guides: GuideLengths;
  catalog: UseQueryResult<ProviderDescriptorContract[]>;
  connect: ActionController<ConnectError>;
  saveModel: ActionController;
  finish: ActionController;
}

export const OnboardingContext = createContext<OnboardingContextValue | null>(null);

export function useOnboarding(): OnboardingContextValue {
  const value = useContext(OnboardingContext);
  if (!value) throw new Error("useOnboarding outside OnboardingContext");
  return value;
}

export function useDescriptor(kind: string | null): ProviderDescriptorContract | null {
  const { catalog } = useOnboarding();
  return kind ? (catalog.data?.find((descriptor) => descriptor.kind === kind) ?? null) : null;
}
