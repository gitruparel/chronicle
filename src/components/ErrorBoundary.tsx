import { Component, type ReactNode, type ErrorInfo } from 'react';
import { AlertTriangle, RefreshCw } from 'lucide-react';

interface Props {
  children: ReactNode;
}

interface State {
  hasError: boolean;
  error: Error | null;
}

export class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false, error: null };
  }

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    console.error('Unhandled error in Chronicle UI:', error, errorInfo);
  }

  handleReset = () => {
    this.setState({ hasError: false, error: null });
    window.location.reload();
  };

  render() {
    if (this.state.hasError) {
      return (
        <div className="min-h-screen bg-background text-text-primary flex items-center justify-center p-6 select-none font-sans">
          <div className="max-w-md w-full bg-surface border border-border rounded-2xl p-6 shadow-xl space-y-4 text-center">
            <div className="w-12 h-12 rounded-xl bg-danger/10 border border-danger/20 flex items-center justify-center text-danger mx-auto">
              <AlertTriangle className="h-6 w-6" />
            </div>
            <div className="space-y-1">
              <h2 className="text-base font-semibold text-text-primary">Something went wrong</h2>
              <p className="text-xs text-text-secondary">
                Chronicle encountered an unexpected interface error.
              </p>
            </div>
            {this.state.error && (
              <div className="text-[11px] font-mono text-danger bg-background p-3 rounded-lg border border-border text-left break-words max-h-32 overflow-y-auto">
                {this.state.error.message || String(this.state.error)}
              </div>
            )}
            <button
              onClick={this.handleReset}
              className="w-full flex items-center justify-center gap-2 py-2 px-4 rounded-xl bg-accent hover:bg-accent-hover text-white text-xs font-medium transition-colors shadow-sm"
            >
              <RefreshCw className="h-3.5 w-3.5" />
              Restart Chronicle Interface
            </button>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}
