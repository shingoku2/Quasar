import { render, screen } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import RemoteHostsList from './RemoteHostsList';
import type { RemoteHostMetric } from './types';
import '@testing-library/jest-dom';

const host = (id: string): RemoteHostMetric => ({
  id, name: id, address: `10.0.0.${id}`, port: 22, reachable: true, latency_ms: 1, error: null,
});

describe('RemoteHostsList', () => {
  // FE-021 review: each host's label must name its own picker, not the first one's.
  it('gives every host its own labelled credential picker', () => {
    render(
      <RemoteHostsList
        remoteHosts={[host('1'), host('2')]}
        savedHosts={[]}
        credentials={[]}
        setHostCredential={vi.fn()}
      />,
    );
    const pickers = screen.getAllByLabelText('SSH metrics credential');
    expect(pickers).toHaveLength(2);
    expect(new Set(pickers.map((p) => p.id)).size).toBe(2);
  });
});
