import { Component, type ReactNode } from "react";
import { AlertTriangle, RotateCcw } from "lucide-react";

interface Props {
  children: ReactNode;
}

interface State {
  hasError: boolean;
  error?: Error;
}

class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false };
  }

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: React.ErrorInfo) {
    console.error("ErrorBoundary caught:", error, errorInfo);
  }

  render() {
    if (this.state.hasError) {
      return (
        <div
          style={{
            display: "flex",
            flexDirection: "column",
            alignItems: "center",
            justifyContent: "center",
            height: "100vh",
            gap: "16px",
            padding: "24px",
            textAlign: "center",
            background: "var(--bg-primary, #0d1117)",
            color: "var(--text-primary, #c9d1d9)",
          }}
        >
          <AlertTriangle size={48} color="#f85149" />
          <h1 style={{ fontSize: "1.25rem", fontWeight: 600 }}>
            Something went wrong
          </h1>
          <p
            style={{
              fontSize: "0.875rem",
              color: "var(--text-secondary, #8b949e)",
              maxWidth: "400px",
            }}
          >
            {this.state.error?.message ?? "An unexpected error occurred."}
          </p>
          <button
            onClick={() => window.location.reload()}
            style={{
              display: "flex",
              alignItems: "center",
              gap: "8px",
              padding: "8px 16px",
              borderRadius: "6px",
              border: "none",
              background: "var(--accent, #58a6ff)",
              color: "#fff",
              fontSize: "0.875rem",
              fontWeight: 500,
              cursor: "pointer",
            }}
          >
            <RotateCcw size={14} />
            Reload Clarity
          </button>
        </div>
      );
    }
    return this.props.children;
  }
}

export default ErrorBoundary;
