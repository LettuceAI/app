import { Component, type ErrorInfo, type ReactNode } from "react";
import { FatalErrorScreen } from "./FatalErrorScreen";

interface FatalErrorBoundaryProps {
  children: ReactNode;
}

interface FatalErrorBoundaryState {
  error: unknown;
  failed: boolean;
}

/** The last line: anything that escapes route boundaries replaces the app with a reload screen. */
export class FatalErrorBoundary extends Component<FatalErrorBoundaryProps, FatalErrorBoundaryState> {
  override state: FatalErrorBoundaryState = { error: null, failed: false };

  static getDerivedStateFromError(error: unknown): FatalErrorBoundaryState {
    return { error, failed: true };
  }

  override componentDidCatch(error: unknown, info: ErrorInfo) {
    console.error("fatal UI error", error, info.componentStack);
  }

  override render() {
    if (this.state.failed) return <FatalErrorScreen error={this.state.error} />;
    return this.props.children;
  }
}
