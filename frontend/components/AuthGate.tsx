"use client";

import React, { useState } from "react";
import { useAuth } from "@/context/authContext";
import SignInModal from "@/components/SignInModal";

interface AuthGateProps {
  children: React.ReactNode;
  /** Content shown while the session is being restored. */
  fallback?: React.ReactNode;
}

/**
 * Renders its children only when the user is authenticated. Otherwise it
 * shows a sign-in prompt that opens the shared SignInModal. This is used by
 * pages that require auth (dashboard, PGN archive, session manager,
 * tournament registration).
 */
export default function AuthGate({ children, fallback }: AuthGateProps) {
  const { isAuthenticated, isLoading } = useAuth();
  const [modalOpen, setModalOpen] = useState(false);

  if (isLoading) {
    return (
      <>{fallback ?? <p className="p-4 text-center text-gray-500">Loading…</p>}</>
    );
  }

  if (isAuthenticated) {
    return <>{children}</>;
  }

  return (
    <>
      <div className="flex flex-col aitems-center justify-center gap-4 p-8">
        <p className="text-lg font-medium">You need to sign in to view this page.</p>
        <button
          type="button"
          className="rounded-md bg-blue-600 px-4 py-2 text-white"
          onClick={() => setModalOpen(true)}
        >
          Sign in
        </button>
      </div>
      <SignInModal isOpen={modalOpen} onClose={() => setModalOpen(false)} />
    </>
  );
}
