import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import React from 'react';
import AICompanionsPage from '../page';

// Mock useAppContext
const mockGetContractClient = vi.fn();
const mockConnectWallet = vi.fn();
const mockAddress = 'GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF';

vi.mock('@/context/walletContext', () => ({
  useAppContext: () => ({
    address: mockAddress,
    status: 'connected',
    connectWallet: mockConnectWallet,
    getContractClient: mockGetContractClient,
    invokeSorobanContract: vi.fn(),
  }),
}));

describe('AICompanionsPage (FE-109)', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.spyOn(window, 'alert').mockImplementation(() => {});
  });

  it('renders the preview banner and preview badge when in preview mode', () => {
    render(<AICompanionsPage />);

    expect(
      screen.getByText(/AI Companion Gallery — Preview Mode/i)
    ).toBeInTheDocument();
    expect(screen.getByTestId('preview-badge')).toHaveTextContent('Preview');
    expect(screen.getByText('AI Companion NFT Gallery')).toBeInTheDocument();
  });

  it('renders action buttons in disabled state during preview without calling window.alert', () => {
    render(<AICompanionsPage />);

    const rentButtons = screen.getAllByRole('button', { name: /Rent \(Coming Soon\)/i });
    expect(rentButtons.length).toBeGreaterThan(0);
    expect(rentButtons[0]).toBeDisabled();

    // Clicking rent should NOT trigger window.alert
    fireEvent.click(rentButtons[0]);
    expect(window.alert).not.toHaveBeenCalled();

    const mintButtons = screen.getAllByRole('button', { name: /Mint \(\d+ XLM\)/i });
    expect(mintButtons.length).toBeGreaterThan(0);
    expect(mintButtons[0]).toBeDisabled();

    // Clicking mint should NOT trigger window.alert
    fireEvent.click(mintButtons[0]);
    expect(window.alert).not.toHaveBeenCalled();
  });

  it('filters companions by tactical style', () => {
    render(<AICompanionsPage />);

    // Click 'Aggressive' filter
    const aggressiveFilter = screen.getByRole('button', { name: 'Aggressive' });
    fireEvent.click(aggressiveFilter);

    expect(screen.getByText('Stockfish Titan v16')).toBeInTheDocument();
    expect(screen.queryByText('Leela Zero Sentinel')).not.toBeInTheDocument();
  });

  it('filters companions by search query', () => {
    render(<AICompanionsPage />);

    const searchInput = screen.getByPlaceholderText('Search agents by name...');
    fireEvent.change(searchInput, { target: { value: 'Kasparov' } });

    expect(screen.getByText('Kasparov Gambit AI')).toBeInTheDocument();
    expect(screen.queryByText('Stockfish Titan v16')).not.toBeInTheDocument();
  });
});
