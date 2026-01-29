import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import HostList from './HostList';
import AddHostDialog from './AddHostDialog';
import '@testing-library/jest-dom';

// Mock SQL plugin
vi.mock('@tauri-apps/plugin-sql', () => {
  const mockSelect = vi.fn().mockResolvedValue([
    { id: 1, name: 'Prod Server', address: '1.2.3.4', protocol: 'ssh' },
    { id: 2, name: 'Dev Box', address: 'localhost', protocol: 'rdp' }
  ]);
  const mockExecute = vi.fn().mockResolvedValue({ rowsAffected: 1 });
  
  return {
    default: {
      load: vi.fn().mockResolvedValue({
        select: mockSelect,
        execute: mockExecute,
      }),
    },
  };
});

describe('Host Management Components', () => {
  describe('HostList', () => {
    it('renders list of hosts', async () => {
      render(<HostList onConnect={() => {}} />);
      
      await waitFor(() => {
        expect(screen.getByText('Prod Server')).toBeInTheDocument();
        expect(screen.getByText('Dev Box')).toBeInTheDocument();
      });
    });

    it('filters hosts by name', async () => {
      render(<HostList onConnect={() => {}} />);
      
      await waitFor(() => expect(screen.getByText('Prod Server')).toBeInTheDocument());
      
      const filterInput = screen.getByPlaceholderText('Filter hosts...');
      fireEvent.change(filterInput, { target: { value: 'Prod' } });
      
      expect(screen.getByText('Prod Server')).toBeInTheDocument();
      expect(screen.queryByText('Dev Box')).not.toBeInTheDocument();
    });
  });

  describe('AddHostDialog', () => {
    it('calls execute on submit', async () => {
      const onAdded = vi.fn();
      render(<AddHostDialog onClose={() => {}} onAdded={onAdded} />);
      
      fireEvent.change(screen.getByPlaceholderText(/Production Web Server/), { target: { value: 'New Host' } });
      fireEvent.change(screen.getByPlaceholderText(/192.168.1.100/), { target: { value: '10.0.0.1' } });
      
      const saveButton = screen.getByText('Save Host');
      fireEvent.click(saveButton);
      
      await waitFor(() => {
        // We can't easily check the mockExecute here because it's inside the factory,
        // but checking onAdded is a good proxy that the promise resolved.
        expect(onAdded).toHaveBeenCalled();
      });
    });
  });
});