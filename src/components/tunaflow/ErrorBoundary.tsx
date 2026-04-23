import { Component, type ReactNode } from "react";

interface Props {
  children: ReactNode;
}

interface State {
  hasError: boolean;
  error: Error | null;
  componentStack: string | null;
}

export class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false, error: null, componentStack: null };
  }

  static getDerivedStateFromError(error: Error): Partial<State> {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: React.ErrorInfo) {
    const stack = errorInfo.componentStack ?? null;
    this.setState({ componentStack: stack });
    console.error("[ErrorBoundary] Caught:", error.message);
    console.error("[ErrorBoundary] Component stack:", stack);
  }

  render() {
    if (this.state.hasError) {
      // Extract the failing component chain from componentStack
      const failChain = this.state.componentStack
        ?.split("\n")
        .filter((l) => l.trim().startsWith("at "))
        .slice(0, 8)
        .map((l) => l.trim())
        .join("\n") ?? "";

      return (
        <div className="flex flex-col items-center justify-center h-screen w-screen bg-background text-foreground gap-4 p-8">
          <p className="text-[14px] font-medium text-destructive">A rendering error occurred</p>
          <p className="text-[12px] text-muted-foreground max-w-lg text-center">
            {this.state.error?.message ?? "Unknown error"}
          </p>
          {failChain && (
            <pre className="text-[10px] text-muted-foreground/50 bg-card/50 rounded-md p-3 max-w-lg overflow-x-auto whitespace-pre-wrap max-h-40 overflow-y-auto border border-border/20">
              {failChain}
            </pre>
          )}
          <div className="flex gap-2">
            <button
              onClick={() => this.setState({ hasError: false, error: null, componentStack: null })}
              className="px-4 py-2 rounded-md text-[12px] font-medium bg-primary/10 text-primary hover:bg-primary/20 transition-colors"
            >
              Try again
            </button>
            <button
              onClick={() => window.location.reload()}
              className="px-4 py-2 rounded-md text-[11px] text-muted-foreground hover:text-foreground transition-colors"
            >
              Reload app
            </button>
            <button
              onClick={() => {
                const info = `Error: ${this.state.error?.message}\n\nComponent Stack:\n${this.state.componentStack ?? "(none)"}`;
                navigator.clipboard.writeText(info).catch(() => {});
              }}
              className="px-4 py-2 rounded-md text-[11px] text-muted-foreground hover:text-foreground transition-colors"
            >
              Copy
            </button>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}
