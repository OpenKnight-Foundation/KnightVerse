"use client";

import { useEffect, useRef, useState } from "react";
import { fetchOnChainTitle, type OnChainTitle } from "@/contracts/client";
import type { ContractMethodInvoker } from "@/contracts/title_badge/src/index";
import { useAppContext } from "@/context/walletContext";
import { CONTRACT_TITLE_BADGE } from "@/lib/api";

export interface UseOnChainTitleResult {
  title: OnChainTitle | null;
  loading: boolean;
  refresh: () => void;
}

export function useOnChainTitle(address?: string | null): UseOnChainTitleResult {
  const { invokeSorobanContract } = useAppContext();
  const invokeRef = useRef(invokeSorobanContract);
  const [title, setTitle] = useState<OnChainTitle | null>(null);
  const [loading, setLoading] = useState(false);
  const [refreshKey, setRefreshKey] = useState(0);
  const previousAddressRef = useRef<string | null>(null);

  useEffect(() => {
    invokeRef.current = invokeSorobanContract;
  }, [invokeSorobanContract]);

  useEffect(() => {
    if (!address || !CONTRACT_TITLE_BADGE) {
      setTitle(null);
      setLoading(false);
      return;
    }

    let cancelled = false;
    setLoading(true);
    const invoke: ContractMethodInvoker = (method, args = [], options = {}) =>
      invokeRef.current(CONTRACT_TITLE_BADGE, method, args, options);
    const addressChanged = previousAddressRef.current !== address;
    previousAddressRef.current = address;

    fetchOnChainTitle({
      owner: address,
      invoke,
      forceRefresh: refreshKey > 0 || addressChanged,
    })
      .then((result) => {
        if (!cancelled) setTitle(result);
      })
      .catch(() => {
        if (!cancelled) setTitle(null);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [address, refreshKey]);

  return {
    title,
    loading,
    refresh: () => setRefreshKey((key) => key + 1),
  };
}
