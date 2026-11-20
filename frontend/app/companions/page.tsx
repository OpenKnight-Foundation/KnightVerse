'use client';

import React, { useState, useMemo, useEffect, useCallback } from 'react';
import { CompanionCard3D, AICompanion } from './CompanionCard3D';
import { useAppContext } from '@/context/walletContext';
import { CONTRACT_AI_NFT } from '@/lib/api';
import { AiNftContractClient } from '@/lib/contracts';
import { Alert, AlertTitle, AlertDescription } from '@/components/ui/alert';
import { AlertCircle, Sparkles, CheckCircle2, Info, X } from 'lucide-react';

export const DEFAULT_COMPANIONS: AICompanion[] = [
  { id: '101', name: 'Stockfish Titan v16', level: 42, exp: 85, tacticalStyle: 'Aggressive', winRate: 91, winStreak: 12, image: '/images/ai-1.png', priceXLM: 150 },
  { id: '102', name: 'Leela Zero Sentinel', level: 38, exp: 64, tacticalStyle: 'Defensive', winRate: 88, winStreak: 8, image: '/images/ai-2.png', priceXLM: 120 },
  { id: '103', name: 'Kasparov Gambit AI', level: 50, exp: 95, tacticalStyle: 'Tactical', winRate: 94, winStreak: 19, image: '/images/ai-3.png', priceXLM: 250 },
  { id: '104', name: 'Capablanca Endgames', level: 35, exp: 40, tacticalStyle: 'Endgame Master', winRate: 86, winStreak: 5, image: '/images/ai-4.png', priceXLM: 90 },
];

const STYLES = ['All', 'Aggressive', 'Defensive', 'Tactical', 'Endgame Master'] as const;

interface NotificationState {
  type: 'info' | 'success' | 'warning' | 'destructive';
  title: string;
  message: string;
}

export default function AICompanionsPage() {
  const { address, connectWallet, getContractClient } = useAppContext();

  const [selectedStyle, setSelectedStyle] = useState<string>('All');
  const [searchQuery, setSearchQuery] = useState('');
  const [companions, setCompanions] = useState<AICompanion[]>(DEFAULT_COMPANIONS);
  const [isLoading, setIsLoading] = useState(false);
  const [isMinting, setIsMinting] = useState<string | null>(null);
  const [notification, setNotification] = useState<NotificationState | null>(null);

  // When CONTRACT_AI_NFT is not set or rental contracts are pending, page operates in Preview mode
  const isContractConfigured = Boolean(CONTRACT_AI_NFT && CONTRACT_AI_NFT.trim().length > 0);
  const isPreview = !isContractConfigured;

  // Attempt to read companions from the ai_nft contract if configured
  const loadCompanionsFromContract = useCallback(async () => {
    if (!isContractConfigured) return;

    try {
      setIsLoading(true);
      const nftClient = getContractClient<AiNftContractClient>('nft', CONTRACT_AI_NFT);
      const supplyBigInt = await nftClient.totalSupply();
      const supply = Number(supplyBigInt);

      if (supply > 0) {
        const loaded: AICompanion[] = [];
        for (let i = 1; i <= Math.min(supply, 20); i++) {
          try {
            const meta = await nftClient.metadata(i);
            let style: AICompanion['tacticalStyle'] = 'Tactical';
            let parsedTraits: { tacticalStyle?: AICompanion['tacticalStyle']; level?: number; winRate?: number } = {};

            try {
              if (meta && typeof meta === 'object' && 'personality_traits' in meta) {
                parsedTraits = JSON.parse((meta as { personality_traits: string }).personality_traits);
              }
            } catch {
              // Non-JSON personality traits fallback
            }

            if (parsedTraits.tacticalStyle && STYLES.includes(parsedTraits.tacticalStyle as any)) {
              style = parsedTraits.tacticalStyle;
            }

            loaded.push({
              id: String(i),
              name: (meta as any)?.name ?? `AI Companion #${i}`,
              level: parsedTraits.level ?? 20,
              exp: 50,
              tacticalStyle: style,
              winRate: parsedTraits.winRate ?? 80,
              winStreak: 3,
              image: (meta as any)?.uri ?? `/images/ai-${((i - 1) % 4) + 1}.png`,
              priceXLM: 100,
            });
          } catch (e) {
            console.warn(`Failed to load metadata for NFT #${i}:`, e);
          }
        }

        if (loaded.length > 0) {
          setCompanions(loaded);
        }
      }
    } catch (err) {
      console.warn('Error reading from ai_nft contract, falling back to catalog:', err);
    } finally {
      setIsLoading(false);
    }
  }, [isContractConfigured, getContractClient]);

  useEffect(() => {
    loadCompanionsFromContract();
  }, [loadCompanionsFromContract]);

  const filteredCompanions = useMemo(() => {
    return companions.filter((c) => {
      const matchesStyle = selectedStyle === 'All' || c.tacticalStyle === selectedStyle;
      const matchesSearch = c.name.toLowerCase().includes(searchQuery.toLowerCase());
      return matchesStyle && matchesSearch;
    });
  }, [companions, selectedStyle, searchQuery]);

  const handleMint = async (id: string) => {
    if (isPreview) {
      setNotification({
        type: 'info',
        title: 'Preview Mode',
        message: 'On-chain minting is disabled while the AI Companion NFT contract is in preview mode.',
      });
      return;
    }

    if (!address) {
      try {
        await connectWallet();
      } catch (err: any) {
        setNotification({
          type: 'destructive',
          title: 'Wallet Connection Required',
          message: err?.message || 'Please connect your Freighter wallet to mint an AI Companion.',
        });
        return;
      }
    }

    try {
      setIsMinting(id);
      const target = companions.find((c) => c.id === id);
      const nftClient = getContractClient<AiNftContractClient>('nft', CONTRACT_AI_NFT);

      const res = await nftClient.mint(
        address!,
        target?.image || 'ipfs://placeholder',
        target?.name || `AI Companion #${id}`,
        'KNIGHT',
      );

      setNotification({
        type: 'success',
        title: 'Mint Submitted',
        message: `AI Companion #${id} mint transaction submitted successfully! Hash: ${res?.hash ?? 'Confirmed'}`,
      });
    } catch (err: any) {
      setNotification({
        type: 'destructive',
        title: 'Mint Failed',
        message: err?.message || 'Failed to submit mint transaction via Freighter.',
      });
    } finally {
      setIsMinting(null);
    }
  };

  const handleRent = (_id: string) => {
    setNotification({
      type: 'info',
      title: 'Decentralized Rental Market',
      message: 'Soroban AI companion rental market contracts are currently in development. Agent rentals will be enabled once deployed.',
    });
  };

  return (
    <main className="min-h-screen bg-slate-950 text-slate-100 p-8 md:p-12">
      <div className="max-w-7xl mx-auto">
        {/* Preview Mode Alert Banner */}
        {isPreview && (
          <Alert variant="warning" className="mb-6 bg-amber-500/10 border-amber-500/30 text-amber-300">
            <AlertCircle className="h-5 w-5 text-amber-400" />
            <div>
              <AlertTitle className="text-amber-200 font-bold flex items-center gap-2">
                <Sparkles className="h-4 w-4" />
                AI Companion Gallery — Preview Mode
              </AlertTitle>
              <AlertDescription className="text-amber-300/90 text-xs md:text-sm mt-1">
                The decentralized AI Companion NFT contract (<code className="bg-amber-950/60 px-1.5 py-0.5 rounded text-amber-200 font-mono text-xs">contracts/ai_nft</code>) and rental market are in active testnet development. Catalog browsing is enabled, but on-chain minting and renting are disabled until smart contracts are deployed.
              </AlertDescription>
            </div>
          </Alert>
        )}

        {/* Dynamic Notification Bar */}
        {notification && (
          <Alert
            variant={notification.type}
            className="mb-6 flex items-start justify-between relative transition-all"
          >
            <div className="flex items-start gap-3">
              {notification.type === 'success' && <CheckCircle2 className="h-5 w-5 text-emerald-400 mt-0.5" />}
              {notification.type === 'info' && <Info className="h-5 w-5 text-blue-400 mt-0.5" />}
              {notification.type === 'warning' && <AlertCircle className="h-5 w-5 text-yellow-400 mt-0.5" />}
              {notification.type === 'destructive' && <AlertCircle className="h-5 w-5 text-red-400 mt-0.5" />}
              <div>
                <AlertTitle className="font-semibold text-sm">{notification.title}</AlertTitle>
                <AlertDescription className="text-xs mt-0.5">{notification.message}</AlertDescription>
              </div>
            </div>
            <button
              type="button"
              onClick={() => setNotification(null)}
              className="text-slate-400 hover:text-slate-200 transition-colors p-1"
              aria-label="Dismiss notification"
            >
              <X className="h-4 w-4" />
            </button>
          </Alert>
        )}

        <div className="flex flex-col md:flex-row md:items-center md:justify-between mb-8 gap-4">
          <div>
            <div className="flex items-center gap-3">
              <h1 className="text-3xl md:text-4xl font-extrabold tracking-tight bg-gradient-to-r from-indigo-400 to-cyan-400 bg-clip-text text-transparent">
                AI Companion NFT Gallery
              </h1>
              {isPreview && (
                <span
                  data-testid="preview-badge"
                  className="px-2.5 py-0.5 rounded-full text-xs font-semibold uppercase tracking-wider bg-amber-500/10 text-amber-400 border border-amber-500/30"
                >
                  Preview
                </span>
              )}
            </div>
            <p className="text-slate-400 text-sm mt-1">
              Recruit, rent, and own decentralized chess AI agents powered by Soroban & PyTorch.
            </p>
          </div>
          <input
            type="text"
            placeholder="Search agents by name..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="px-4 py-2 bg-slate-900 border border-slate-800 rounded-xl focus:outline-none focus:border-indigo-500 text-sm w-full md:w-72"
          />
        </div>

        <div className="flex flex-wrap gap-2 mb-8">
          {STYLES.map((style) => (
            <button
              key={style}
              type="button"
              onClick={() => setSelectedStyle(style)}
              className={`px-4 py-2 rounded-xl text-xs font-semibold transition-all ${
                selectedStyle === style
                  ? 'bg-indigo-600 text-white shadow-lg shadow-indigo-600/30'
                  : 'bg-slate-900 text-slate-400 hover:bg-slate-800 border border-slate-800'
              }`}
            >
              {style}
            </button>
          ))}
        </div>

        {isLoading ? (
          <div className="text-center py-20 text-slate-400 flex flex-col items-center gap-3">
            <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-indigo-500" />
            <p className="text-sm">Querying Soroban ai_nft contract...</p>
          </div>
        ) : filteredCompanions.length > 0 ? (
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-6">
            {filteredCompanions.map((companion) => (
              <CompanionCard3D
                key={companion.id}
                companion={companion}
                onMint={handleMint}
                onRent={handleRent}
                disabled={isPreview}
                isMintDisabled={isPreview}
                isRentDisabled={true} // Renting remains disabled until rental contract is deployed
                mintLabel={isPreview ? `Mint (${companion.priceXLM} XLM)` : undefined}
                rentLabel="Rent (Coming Soon)"
                isMinting={isMinting === companion.id}
              />
            ))}
          </div>
        ) : (
          <div className="text-center py-20 text-slate-500">
            No AI companions found matching your criteria.
          </div>
        )}
      </div>
    </main>
  );
}
