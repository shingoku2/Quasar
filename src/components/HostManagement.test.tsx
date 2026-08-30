import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { useState } from 'react';
import HostList from './HostList';
import AddHostDialog, { AddHostInitialValues } from './AddHostDialog';
import '@testing-library/jest-dom';

// Mock Tauri invoke and event
const { mockInvoke, mockSelect, mockExecute } = vi.hoisted(() => {
  const mockSelect = vi.fn().mockResolvedValue([
    { id: '1', name: 'Prod Server', address: '1.2.3.4', protocol: 'ssh' },
    { id: '2', name: 'Dev Box', address: 'localhost', protocol: 'rdp' },
  ]);
  const mockExecute = vi.fn().mockResolvedValue({ rowsAffected: 1 });
  const mockInvoke = vi.fn((command: string, args?: unknown) => {
    if (command === 'get_discovered_hosts') {
      return Promise.resolve([
        {
          ip: '10.0.0.5',
          hostname: 'NAS',
          services: [{ port: 22 }],
        },
      ]);
    }
    if (command === 'get_saved_hosts') return mockSelect();
    if (
      command === 'upsert_saved_host' ||
      command === 'update_saved_host' ||
      command === 'remove_saved_hosts'
    ) {
      return mockExecute(command, args);
    }
    return Promise.resolve();
  });
  return { mockInvoke, mockSelect, mockExecute };
});
vi.mock('@tauri-apps/api/core', () => ({
  invoke: mockInvoke,
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

beforeEach(() => {
  mockExecute.mockClear();
  mockSelect.mockResolvedValue([
    { id: '1', name: 'Prod Server', address: '1.2.3.4', protocol: 'ssh' },
    { id: '2', name: 'Dev Box', address: 'localhost', protocol: 'rdp' },
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

    it('passes an existing host to the edit dialog callback', async () => {
      const onAddHost = vi.fn();
      render(<HostList onConnect={() => {}} onSftp={() => {}} onAddHost={onAddHost} />);

      await waitFor(() => expect(screen.getByText('Prod Server')).toBeInTheDocument());
      fireEvent.click(screen.getAllByRole('button', { name: 'Edit' })[0]);

      expect(onAddHost).toHaveBeenCalledWith(expect.objectContaining({
        id: '1',
        name: 'Prod Server',
        address: '1.2.3.4',
        protocol: 'ssh',
      }));
    });

    it('opens add dialog prefilled when clicking Discovery Add', async () => {
      const HostListHarness = () => {
        const [showAddHost, setShowAddHost] = useState(false);
        const [initialValues, setInitialValues] = useState<AddHostInitialValues | undefined>(undefined);

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
    it('offers all host protocol options', () => {
      render(<AddHostDialog onClose={() => {}} onAdded={() => {}} />);

      const protocolSelect = screen.getByLabelText(/Protocol/i);
      const options = Array.from(protocolSelect.querySelectorAll('option')).map(
        (o) => o.getAttribute('value'),
      );
      expect(options).toEqual(['ssh', 'rdp', 'database', 'api', 'other']);
    });

    it('requires a port for protocols without a default port', () => {
      render(<AddHostDialog onClose={() => {}} onAdded={() => {}} />);

      const portInput = screen.getByLabelText(/Port/i);
      expect(portInput).not.toBeRequired();

      fireEvent.change(screen.getByLabelText(/Protocol/i), { target: { value: 'database' } });
      expect(portInput).toBeRequired();
      expect(portInput).toHaveAttribute('placeholder', '5432');
    });

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

    it('updates an existing host port in place', async () => {
      const onAdded = vi.fn();
      render(
        <AddHostDialog
          initialValues={{
            id: 'vps-1',
            name: 'VPS',
            address: '15.204.11.162',
            protocol: 'ssh',
            port: 22,
            username: 'edward',
          }}
          onClose={() => {}}
          onAdded={onAdded}
        />,
      );

      expect(screen.getByText('Edit Host')).toBeInTheDocument();
      fireEvent.change(screen.getByLabelText(/Port/i), { target: { value: '6969' } });
      fireEvent.click(screen.getByText('Save Host'));

      await waitFor(() => {
        expect(mockExecute).toHaveBeenCalledWith(
          'update_saved_host',
          expect.objectContaining({ hostId: 'vps-1', port: 6969 }),
        );
        expect(onAdded).toHaveBeenCalled();
      });
    });

    it('calls onConnect callback when Connect is clicked', async () => {
      const onConnect = vi.fn();
      render(<HostList onConnect={onConnect} onSftp={() => {}} />);

      await waitFor(() => expect(screen.getByText('Prod Server')).toBeInTheDocument());

      const connectBtns = screen.getAllByRole('button', { name: /Connect/i });
      fireEvent.click(connectBtns[0]);

      expect(onConnect).toHaveBeenCalledWith(
        expect.objectContaining({ name: 'Prod Server', address: '1.2.3.4' })
      );
    });

    it('calls onSftp callback when SFTP is clicked on an SSH host', async () => {
      const onSftp = vi.fn();
      render(<HostList onConnect={() => {}} onSftp={onSftp} />);

      await waitFor(() => expect(screen.getByText('Prod Server')).toBeInTheDocument());

      const sftpBtns = screen.getAllByRole('button', { name: /SFTP/i });
      fireEvent.click(sftpBtns[0]);

      expect(onSftp).toHaveBeenCalledWith(
        expect.objectContaining({ name: 'Prod Server', protocol: 'ssh' })
      );
    });

    it('removes a host after Remove confirmation', async () => {
      const confirmSpy = vi.spyOn(window, 'confirm').mockReturnValue(true);
      mockExecute.mockResolvedValue({ rowsAffected: 1 });
      mockSelect
        .mockResolvedValueOnce([
          { id: '1', name: 'Prod Server', address: '1.2.3.4', protocol: 'ssh' },
          { id: '2', name: 'Dev Box', address: 'localhost', protocol: 'rdp' },
        ])
        .mockResolvedValueOnce([
          { id: '2', name: 'Dev Box', address: 'localhost', protocol: 'rdp' },
        ]);

      render(<HostList onConnect={() => {}} onSftp={() => {}} />);

      await waitFor(() => expect(screen.getByText('Prod Server')).toBeInTheDocument());

      // Use exact name 'Remove' to avoid matching the "Remove duplicates" button
      const removeBtns = screen.getAllByRole('button', { name: 'Remove' });
      fireEvent.click(removeBtns[0]);

      await waitFor(() => {
        expect(mockExecute).toHaveBeenCalledWith(
          'remove_saved_hosts',
          { ids: ['1'] }
        );
      });
      confirmSpy.mockRestore();
    });

    it('does not remove host when Remove confirmation is cancelled', async () => {
      const confirmSpy = vi.spyOn(window, 'confirm').mockReturnValue(false);

      render(<HostList onConnect={() => {}} onSftp={() => {}} />);
      await waitFor(() => expect(screen.getByText('Prod Server')).toBeInTheDocument());

      // Use exact name 'Remove' to avoid matching the "Remove duplicates" button
      const removeBtns = screen.getAllByRole('button', { name: 'Remove' });
      fireEvent.click(removeBtns[0]);

      expect(mockExecute).not.toHaveBeenCalled();
      confirmSpy.mockRestore();
    });

    it('detects and shows duplicate host count', async () => {
      mockSelect.mockResolvedValueOnce([
        { id: '1', name: 'Server A', address: '1.2.3.4', protocol: 'ssh', port: 22 },
        { id: '2', name: 'Server A copy', address: '1.2.3.4', protocol: 'ssh', port: 22 },
      ]);

      render(<HostList onConnect={() => {}} onSftp={() => {}} />);

      await waitFor(() => {
        expect(screen.getByText(/1 duplicate/)).toBeInTheDocument();
      });
    });

    it('shows no-hosts message when list is empty', async () => {
      mockSelect.mockResolvedValueOnce([]);

      render(<HostList onConnect={() => {}} onSftp={() => {}} />);

      await waitFor(() => {
        expect(screen.getByText(/No hosts added yet/i)).toBeInTheDocument();
      });
    });

    it('shows filter no-match message when filter has no results', async () => {
      render(<HostList onConnect={() => {}} onSftp={() => {}} />);
      await waitFor(() => expect(screen.getByText('Prod Server')).toBeInTheDocument());

      const filterInput = screen.getByPlaceholderText('Filter hosts...');
      fireEvent.change(filterInput, { target: { value: 'zzz-nomatch-zzz' } });

      expect(screen.getByText(/No hosts match your filter/i)).toBeInTheDocument();
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

      expect(mockExecute).toHaveBeenCalledWith(
        'upsert_saved_host',
        expect.objectContaining({ port: undefined })
      );
    });
  });
});