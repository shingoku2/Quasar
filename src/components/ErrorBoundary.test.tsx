import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import ErrorBoundary from './ErrorBoundary';
import '@testing-library/jest-dom';

const ThrowingComponent = ({ shouldThrow = true }: { shouldThrow?: boolean }) => {
  if (shouldThrow) {
    throw new Error('Test crash error');
  }
  return <div>Child content</div>;
};

describe('ErrorBoundary', () => {
  let consoleErrorSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    // Suppress React's error boundary console output during tests
    consoleErrorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
  });

  afterEach(() => {
    consoleErrorSpy.mockRestore();
  });

  it('renders children when no error is thrown', () => {
    render(
      <ErrorBoundary>
        <ThrowingComponent shouldThrow={false} />
      </ErrorBoundary>
    );

    expect(screen.getByText('Child content')).toBeInTheDocument();
    expect(screen.queryByText('App Crashed')).not.toBeInTheDocument();
  });

  it('shows crash UI when a child component throws', () => {
    render(
      <ErrorBoundary>
        <ThrowingComponent shouldThrow={true} />
      </ErrorBoundary>
    );

    expect(screen.getByText('App Crashed')).toBeInTheDocument();
  });

  it('displays the error message in the crash UI', () => {
    render(
      <ErrorBoundary>
        <ThrowingComponent shouldThrow={true} />
      </ErrorBoundary>
    );

    // The error message appears in both the <p> and the <pre> stack trace — use getAllBy
    expect(screen.getAllByText(/Test crash error/).length).toBeGreaterThan(0);
  });

  it('renders a Reload App button in the crash UI', () => {
    render(
      <ErrorBoundary>
        <ThrowingComponent shouldThrow={true} />
      </ErrorBoundary>
    );

    expect(screen.getByRole('button', { name: /Reload App/i })).toBeInTheDocument();
  });

  it('calls window.location.reload when Reload App is clicked', () => {
    const reloadSpy = vi.fn();
    Object.defineProperty(window, 'location', {
      value: { reload: reloadSpy },
      writable: true,
    });

    render(
      <ErrorBoundary>
        <ThrowingComponent shouldThrow={true} />
      </ErrorBoundary>
    );

    fireEvent.click(screen.getByRole('button', { name: /Reload App/i }));
    expect(reloadSpy).toHaveBeenCalledOnce();
  });

  it('hides children after catching an error', () => {
    render(
      <ErrorBoundary>
        <ThrowingComponent shouldThrow={true} />
      </ErrorBoundary>
    );

    expect(screen.queryByText('Child content')).not.toBeInTheDocument();
  });

  it('logs the error via componentDidCatch', () => {
    render(
      <ErrorBoundary>
        <ThrowingComponent shouldThrow={true} />
      </ErrorBoundary>
    );

    expect(consoleErrorSpy).toHaveBeenCalled();
  });

  // FE-002: a scoped boundary contains the crash and can recover without a reload.
  it('scoped boundary shows an inline panel and recovers on Try again', () => {
    const reload = vi.fn();
    const original = window.location;
    Object.defineProperty(window, 'location', { value: { ...original, reload }, configurable: true });
    let shouldThrow = true;
    const Flaky = () => {
      if (shouldThrow) throw new Error('view broke');
      return <div>view ok</div>;
    };
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {});
    try {
      render(
        <div>
          <span>sibling view</span>
          <ErrorBoundary scope="Monitoring"><Flaky /></ErrorBoundary>
        </div>
      );
      expect(screen.getByRole('alert')).toHaveTextContent('The Monitoring view hit an error.');
      expect(screen.getByText('sibling view')).toBeInTheDocument();
      expect(screen.queryByText('App Crashed')).not.toBeInTheDocument();

      shouldThrow = false;
      fireEvent.click(screen.getByRole('button', { name: /Try again/i }));
      expect(screen.getByText('view ok')).toBeInTheDocument();
      expect(reload).not.toHaveBeenCalled();
    } finally {
      consoleError.mockRestore();
      Object.defineProperty(window, 'location', { value: original, configurable: true });
    }
  });
});
