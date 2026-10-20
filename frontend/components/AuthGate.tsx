"use client";

import React, { useEffect, useState } from "react";
import { useAuth } from "@/context/authContext";
import SignInModal from "@/components/SignInModal";

export interface AuthGateProps {
  children: React.ReactNode;
  /** Rendered when the user is not signed in. Defaults to a sign-in prompt. */
  fallback?: React.ReactNode;
}

/**
 * Wards a subtree behind authentication. When the user is not signed in it
 * renders a prompt that opens the shared `SignInModal`, so every auth-gated
 * page gets a working sign-in entry point (#FE-96).
 */
export default function AuthGate({ children, fallback }: AuthGateProps) {
  const { isAuthenticated, isLoading } = useAuth();
  const [modalOpen, setModalOpen] = useState(false);

  useEffect(() => {
    if (isAuthenticated) {
      setModalOpen(false);
    }
  }, [isAuthenticated]);

  if (isLoading) {
    return (
      <div className="flex items-center justify-center py-12 text-slate-400" role="status">
        Loading…
      </div>
    );
  }

  if (isAuthenticated) {
    return <>{children}</>;
  }

  return (
    <>
      {fallback ?? (
        <div
          className="flex flex-col items-center justify-center gap-4 py-16 text-center"
          data-testid="auth-gate-prompt"
        >
          <p className="text-slate-300">You need to sign in to view this page.</p>
          <button
            type="button"
            onClick={() => setModalOpen(true)}
            data-testid="auth-gate-sign-in"
            className="rounded-md bg-indigo-600 px-4 py-2 font-semibold text-white hover:bg-indigo-500"
          >
            Sign in
          </button>
        </div>
      )}
      <SignInModal isOpen={modalOpen} onClose={() => setModalOpen(false)} />
    </>
  );
}
