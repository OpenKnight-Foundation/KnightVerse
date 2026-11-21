import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import { CompanionCard3D, AICompanion } from '../CompanionCard3D';

const mockCompanion: AICompanion = {
  id: '999',
  name: 'Test Grandmaster AI',
  level: 25,
  exp: 50,
  tacticalStyle: 'Aggressive',
  winRate: 85,
  winStreak: 4,
  image: '/test-image.png',
  priceXLM: 100,
};

describe('CompanionCard3D Component', () => {
  it('renders companion details correctly', () => {
    const handleMint = vi.fn();
    const handleRent = vi.fn();

    render(<CompanionCard3D companion={mockCompanion} onMint={handleMint} onRent={handleRent} />);

    expect(screen.getByText('Test Grandmaster AI')).toBeInTheDocument();
    expect(screen.getByText('Level 25 • 4 Win Streak')).toBeInTheDocument();
    expect(screen.getByText('Mint ID: #999')).toBeInTheDocument();
    expect(screen.getByText('Aggressive')).toBeInTheDocument();
  });

  it('triggers onMint and onRent callback handlers correctly', () => {
    const handleMint = vi.fn();
    const handleRent = vi.fn();

    render(<CompanionCard3D companion={mockCompanion} onMint={handleMint} onRent={handleRent} />);

    fireEvent.click(screen.getByText('Mint (100 XLM)'));
    expect(handleMint).toHaveBeenCalledWith('999');

    fireEvent.click(screen.getByText('Rent Agent'));
    expect(handleRent).toHaveBeenCalledWith('999');
  });

  it('disables mint and rent buttons when disabled prop is true and does not trigger callbacks', () => {
    const handleMint = vi.fn();
    const handleRent = vi.fn();

    render(
      <CompanionCard3D
        companion={mockCompanion}
        onMint={handleMint}
        onRent={handleRent}
        disabled={true}
        rentLabel="Rent (Coming Soon)"
      />
    );

    const rentBtn = screen.getByRole('button', { name: /Rent \(Coming Soon\)/i });
    const mintBtn = screen.getByRole('button', { name: /Mint \(100 XLM\)/i });

    expect(rentBtn).toBeDisabled();
    expect(mintBtn).toBeDisabled();

    fireEvent.click(rentBtn);
    fireEvent.click(mintBtn);

    expect(handleRent).not.toHaveBeenCalled();
    expect(handleMint).not.toHaveBeenCalled();
  });

  it('renders minting in-progress state correctly', () => {
    const handleMint = vi.fn();
    const handleRent = vi.fn();

    render(
      <CompanionCard3D
        companion={mockCompanion}
        onMint={handleMint}
        onRent={handleRent}
        isMinting={true}
      />
    );

    const mintBtn = screen.getByRole('button', { name: /Minting\.\.\./i });
    expect(mintBtn).toBeDisabled();
  });
});
