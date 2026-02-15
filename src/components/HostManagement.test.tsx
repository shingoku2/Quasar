import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { useState } from 'react';
import HostList from './HostList';
import AddHostDialog from './AddHostDialog';
import '@testing-library/jest-dom';

// Mock Tauri invoke and event
const { mockInvoke, mockSelect, mockExecute } = vi.hoisted(() => ({
  mockInvoke: vi.fn((command: string) => {
    if (command === 'get_discovered_hosts') {
      return Promise.resolve([
        {
          ip: '10.0.0.5',
          hostname: 'NAS',
          services: [{ port: 22 }],
        },
      ]);
    }
    return Promise.resolve();
  }),
  mockSelect: vi.fn().mockResolvedValue([
    { id: 1, name: 'Prod Server', address: '1.2.3.4', protocol: 'ssh' },
    { id: 2, name: 'Dev Box', address: 'localhost', protocol: 'rdp' },
  ]),
  mockExecute: vi.fn().mockResolvedValue({ rowsAffected: 1 }),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: mockInvoke,
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

// Mock SQL plugin
vi.mock('@tauri-apps/plugin-sql', () => {
  return {
    default: {
      load: vi.fn().mockResolvedValue({
        select: mockSelect,
        execute: mockExecute,
      }),
    },
  };
});

beforeEach(() => {
  mockExecute.mockClear();
  mockSelect.mockResolvedValue([
    { id: 1, name: 'Prod Server', address: '1.2.3.4', protocol: 'ssh' },
    { id: 2, name: 'Dev Box', address: 'localhost', protocol: 'rdp' },
  ]);
});

describe('Host Management Components', () => {
  describe('HostList', () => {
    it('renders list of hosts', async () => {
      render(<HostList onConnect={() => {}} onSftp={() => {}} />);
      
      await waitFor(() => {
        expect(screen.getByText('Prod Server')).toBeInTheDocument();
        expect(screen.getByText('Dev Box')).toBeInTheDocument();
      });
    });

    it('filters hosts by name', async () => {
      render(<HostList onConnect={() => {}} onSftp={() => {}} />);
      
      await waitFor(() => expect(screen.getByText('Prod Server')).toBeInTheDocument());
      
      const filterInput = screen.getByPlaceholderText('Filter hosts...');
      fireEvent.change(filterInput, { target: { value: 'Prod' } });
      
      expect(screen.getByText('Prod Server')).toBeInTheDocument();
      expect(screen.queryByText('Dev Box')).not.toBeInTheDocument();
    });

    it('opens add dialog prefilled when clicking Discovery Add', async () => {
      const HostListHarness = () => {
        const [showAddHost, setShowAddHost] = useState(false);
        const [initialValues, setInitialValues] = useState<{
          name?: string;
          address?: string;
          protocol?: 'ssh' | 'rdp';
          port?: number | null;
          username?: string;
        } | undefined>(undefined);

        return (
          <>
            <HostList
              onConnect={() => {}}
              onSftp={() => {}}
              onAddHost={(values) => {
                setInitialValues(values);
                setShowAddHost(true);
              }}
            />
            {showAddHost && (
              <AddHostDialog
                initialValues={initialValues}
                onClose={() => setShowAddHost(false)}
                onAdded={() => {}}
              />
            )}
          </>
        );
      };

      render(<HostListHarness />);

      await waitFor(() => {
        expect(screen.getByText('NAS')).toBeInTheDocument();
      });

      fireEvent.click(screen.getByRole('button', { name: 'Add' }));

      expect(screen.getByPlaceholderText(/Production Web Server/)).toHaveValue('NAS');
      expect(screen.getByPlaceholderText(/192.168.1.100/)).toHaveValue('10.0.0.5');
      expect(screen.getByPlaceholderText('22')).toHaveValue(22);
    });
  });

  describe('AddHostDialog', () => {
    it('calls execute on submit', async () => {
      const onAdded = vi.fn();
      mockSelect.mockResolvedValue([]);
      render(<AddHostDialog onClose={() => {}} onAdded={onAdded} />);
      
      fireEvent.change(screen.getByPlaceholderText(/Production Web Server/), { target: { value: 'New Host' } });
      fireEvent.change(screen.getByPlaceholderText(/192.168.1.100/), { target: { value: '10.0.0.1' } });
      
      const saveButton = screen.getByText('Save Host');
      fireEvent.click(saveButton);
      
      await waitFor(() => {
        expect(onAdded).toHaveBeenCalled();
      });
    });

    it('uses default SSH port when port input is left blank', async () => {
      const onAdded = vi.fn();
      mockSelect.mockResolvedValue([]);
      render(<AddHostDialog onClose={() => {}} onAdded={onAdded} />);

      fireEvent.change(screen.getByPlaceholderText(/Production Web Server/), { target: { value: 'No Port Host' } });
      fireEvent.change(screen.getByPlaceholderText(/192.168.1.100/), { target: { value: '10.0.0.44' } });

      fireEvent.click(screen.getByText('Save Host'));

      await waitFor(() => {
        expect(mockExecute).toHaveBeenCalled();
        expect(onAdded).toHaveBeenCalled();
      });

      const insertCall = mockExecute.mock.calls.find(([sql]) =>
        typeof sql === 'string' && sql.includes('INSERT INTO hosts')
      );
      expect(insertCall).toBeDefined();

      const executeArgs = insertCall?.[1] as unknown[];
      expect(executeArgs[4]).toBe(22);
    });
  });
});