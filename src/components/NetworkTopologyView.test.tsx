import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import NetworkTopologyView from './NetworkTopologyView';
import '@testing-library/jest-dom';

// Mock vis-network and vis-data
const mockNetworkOn = vi.fn();
const mockNetworkSetOptions = vi.fn();
const mockNetworkSelectNodes = vi.fn();
const mockNetworkFocus = vi.fn();
const mockNetworkGetScale = vi.fn().mockReturnValue(1.0);
const mockNetworkMoveTo = vi.fn();
const mockNetworkFit = vi.fn();
const mockNetworkDestroy = vi.fn();

vi.mock('vis-network', () => {
  return {
    Network: class {
      on = mockNetworkOn;
      setOptions = mockNetworkSetOptions;
      selectNodes = mockNetworkSelectNodes;
      focus = mockNetworkFocus;
      getScale = mockNetworkGetScale;
      moveTo = mockNetworkMoveTo;
      fit = mockNetworkFit;
      destroy = mockNetworkDestroy;
    }
  };
});

const mockDataSetAdd = vi.fn();
vi.mock('vis-data', () => {
  return {
    DataSet: class {
      add = mockDataSetAdd;
    }
  };
});

const mockHosts = [
  {
    ip: '192.168.1.5',
    hostname: 'web-server',
    is_alive: true,
    open_ports: [80, 443],
    device_type: 'server',
    services: [],
    last_seen: Date.now(),
    latency_ms: 10,
  },
  {
    ip: '192.168.1.10',
    hostname: 'my-workstation',
    is_alive: true,
    open_ports: [22],
    device_type: 'workstation',
    services: [],
    last_seen: Date.now(),
    latency_ms: 5,
  },
];

describe('NetworkTopologyView', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders controls and empty state when no hosts are provided', () => {
    render(<NetworkTopologyView hosts={[]} />);

    // Empty state text
    expect(screen.getByText('No hosts discovered yet')).toBeInTheDocument();
    expect(screen.getByText('Run a network scan to visualize your network')).toBeInTheDocument();

    // Controls are rendered (titles)
    expect(screen.getByTitle('Zoom In')).toBeInTheDocument();
    expect(screen.getByTitle('Zoom Out')).toBeInTheDocument();
    expect(screen.getByTitle('Fit to Screen')).toBeInTheDocument();
    expect(screen.getByTitle('Freeze Layout')).toBeInTheDocument(); // Defaults to true
  });

  it('renders correctly with hosts and adds nodes/edges', () => {
    render(<NetworkTopologyView hosts={mockHosts} />);

    // Empty state should not be visible
    expect(screen.queryByText('No hosts discovered yet')).not.toBeInTheDocument();

    // Check info text
    expect(screen.getByText('2')).toBeInTheDocument(); // 2 hosts discovered

    // DataSet.add is shared by the nodes and edges instances: the gateway node,
    // one node per host, and one edge per host connecting it to the gateway.
    expect(mockDataSetAdd).toHaveBeenCalledTimes(5);
    const addedIds = mockDataSetAdd.mock.calls.map(([arg]) => arg.id);
    expect(addedIds).toEqual(
      expect.arrayContaining(['gateway', '192.168.1.5', '192.168.1.10', '192.168.1.5-gateway', '192.168.1.10-gateway']),
    );
  });

  it('handles physics toggle', () => {
    render(<NetworkTopologyView hosts={mockHosts} />);

    const physicsBtn = screen.getByTitle('Freeze Layout');
    fireEvent.click(physicsBtn);

    // Button title changes
    expect(screen.getByTitle('Enable Physics')).toBeInTheDocument();

    // setOptions should be called to disable physics
    expect(mockNetworkSetOptions).toHaveBeenCalledWith({ physics: { enabled: false } });
  });

  it('handles zoom in, zoom out, and fit buttons', () => {
    // Model the network's actual scale so moveTo's computed value can be
    // checked against the *current* scale rather than a fixed 1.0 mock,
    // which would pass even if the zoom handlers stopped multiplying by it.
    let currentScale = 1.0;
    mockNetworkGetScale.mockImplementation(() => currentScale);
    mockNetworkMoveTo.mockImplementation(({ scale }: { scale: number }) => {
      currentScale = scale;
    });

    render(<NetworkTopologyView hosts={mockHosts} />);

    fireEvent.click(screen.getByTitle('Zoom In'));
    expect(mockNetworkMoveTo).toHaveBeenLastCalledWith({ scale: 1.2 });

    fireEvent.click(screen.getByTitle('Zoom In'));
    expect(mockNetworkMoveTo).toHaveBeenLastCalledWith({ scale: 1.2 * 1.2 });

    fireEvent.click(screen.getByTitle('Zoom Out'));
    expect(mockNetworkMoveTo).toHaveBeenLastCalledWith({ scale: 1.2 * 1.2 * 0.8 });

    fireEvent.click(screen.getByTitle('Fit to Screen'));
    expect(mockNetworkFit).toHaveBeenCalled();

    // Restore the default stub so later tests aren't affected by this test's
    // stateful mock implementation (vi.clearAllMocks() clears call history
    // but not custom implementations).
    mockNetworkGetScale.mockReturnValue(1.0);
    mockNetworkMoveTo.mockReset();
  });

  it('handles search functionality', () => {
    render(<NetworkTopologyView hosts={mockHosts} />);

    const searchInput = screen.getByPlaceholderText('Search hosts...');
    fireEvent.change(searchInput, { target: { value: 'web' } });

    expect(mockNetworkSelectNodes).toHaveBeenCalledWith(['192.168.1.5']);
    expect(mockNetworkFocus).toHaveBeenCalledWith('192.168.1.5', expect.any(Object));
  });

  it('registers click and double-click events and triggers callbacks', () => {
    const onHostClick = vi.fn();
    const onHostConnect = vi.fn();

    render(<NetworkTopologyView hosts={mockHosts} onHostClick={onHostClick} onHostConnect={onHostConnect} />);

    // Extract the registered event handlers
    const clickHandler = mockNetworkOn.mock.calls.find(call => call[0] === 'click')?.[1];
    const doubleClickHandler = mockNetworkOn.mock.calls.find(call => call[0] === 'doubleClick')?.[1];

    expect(clickHandler).toBeDefined();
    expect(doubleClickHandler).toBeDefined();

    // Simulate clicking on a host node
    clickHandler({ nodes: ['192.168.1.10'] });
    expect(onHostClick).toHaveBeenCalledWith(mockHosts[1]);

    // Simulate double-clicking on a host node
    doubleClickHandler({ nodes: ['192.168.1.5'] });
    expect(onHostConnect).toHaveBeenCalledWith(mockHosts[0]);

    // Clicking on gateway should not trigger callbacks
    clickHandler({ nodes: ['gateway'] });
    expect(onHostClick).toHaveBeenCalledTimes(1); // Still 1 from before
  });
});
