import { render, screen } from '@testing-library/react';
import { describe, it, expect } from 'vitest';
import TopProcessesCard from './TopProcessesCard';
import type { ProcessInfo } from './types';
import '@testing-library/jest-dom';

const processes: ProcessInfo[] = [
  { pid: 42, name: 'chrome', cpu_usage: 12.34, memory_mb: 900 },
  { pid: 7, name: 'init', cpu_usage: 1.0, memory_mb: 10 },
];

describe('TopProcessesCard', () => {
  it('ranks CPU processes, showing one-decimal percentages', () => {
    render(<TopProcessesCard kind="cpu" processes={processes} />);
    expect(screen.getByText('Top CPU Processes')).toBeInTheDocument();
    expect(screen.getByText('12.3%')).toBeInTheDocument(); // toFixed(1)
    expect(screen.getByText('1.0%')).toBeInTheDocument();
    expect(screen.queryByText('900 MB')).not.toBeInTheDocument();
    for (const label of ['#1', '#2', 'chrome', 'init', 'PID 42', 'PID 7']) {
      expect(screen.getByText(label)).toBeInTheDocument();
    }
  });

  it('ranks memory processes in MB, not percent', () => {
    render(<TopProcessesCard kind="memory" processes={processes} />);
    expect(screen.getByText('Top Memory Processes')).toBeInTheDocument();
    expect(screen.getByText('900 MB')).toBeInTheDocument();
    expect(screen.getByText('10 MB')).toBeInTheDocument();
    expect(screen.queryByText('12.3%')).not.toBeInTheDocument();
  });

  it('renders an empty list without a crash', () => {
    render(<TopProcessesCard kind="cpu" processes={[]} />);
    expect(screen.getByText('Top CPU Processes')).toBeInTheDocument();
    expect(screen.queryByText('#1')).not.toBeInTheDocument();
  });
});
