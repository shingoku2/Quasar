import React, { Component, ReactNode } from 'react';

interface Props {
  children: ReactNode;
  /**
   * Name of the view this boundary isolates. With a scope, a crash shows an inline panel
   * with "Try again" (re-mounts the view) instead of the full-screen "Reload App": one
   * broken view no longer blanks the app or drops live SSH sessions by reloading (FE-002).
   */
  scope?: string;
}

interface State {
  hasError: boolean;
  error: Error | null;
}

class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false, error: null };
  }

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: React.ErrorInfo) {
    console.error('React Error Boundary caught:', error, errorInfo);
  }

  private reset = () => {
    this.setState({ hasError: false, error: null });
  };

  render() {
    if (this.state.hasError && this.props.scope) {
      return (
        <div role="alert" className="m-6 p-4 rounded-xl border border-red-500/40 bg-red-500/10 text-sm">
          <p className="font-bold text-red-400">The {this.props.scope} view hit an error.</p>
          <p className="mt-1 text-gray-300 font-mono break-all">{this.state.error?.message}</p>
          <p className="mt-1 text-gray-500">Other views and open sessions are unaffected.</p>
          <button
            onClick={this.reset}
            className="mt-3 px-3 py-1.5 rounded bg-accent/20 text-accent hover:bg-accent/30"
          >
            Try again
          </button>
        </div>
      );
    }
    if (this.state.hasError) {
      return (
        <div style={{ 
          padding: '20px', 
          color: '#ef4444', 
          background: '#1a1b21',
          fontFamily: 'monospace',
          minHeight: '100vh'
        }}>
          <h1 style={{ color: '#ef4444' }}>App Crashed</h1>
          <p style={{ color: '#fff' }}>Error: {this.state.error?.message}</p>
          <pre style={{ 
            background: '#27272a', 
            padding: '10px', 
            borderRadius: '4px',
            overflow: 'auto'
          }}>
            {this.state.error?.stack}
          </pre>
          <button 
            onClick={() => window.location.reload()}
            style={{
              marginTop: '20px',
              padding: '10px 20px',
              background: '#3b82f6',
              color: '#fff',
              border: 'none',
              borderRadius: '4px',
              cursor: 'pointer'
            }}
          >
            Reload App
          </button>
        </div>
      );
    }

    return this.props.children;
  }
}

export default ErrorBoundary;
