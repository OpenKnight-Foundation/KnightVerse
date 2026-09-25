"use client";

/**
 * FeatureFlagBootstrap
 *
 * Thin client component that reads the authenticated user's ID from AuthContext
 * and supplies it to FeatureFlagProvider as the stable `userIdentifier`.
 * This keeps app/layout.tsx a Server Component while still giving the flag
 * system access to the current user.
 */

import React, { type ReactNode } from "react";
import { FeatureFlagProvider } from "@/context/featureFlagContext";
import { useAuth } from "@/context/authContext";

export default function FeatureFlagBootstrap({ children }: { children: ReactNode }) {
  const { user } = useAuth();
  // Use the numeric user_id cast to a string, or the wallet address if available.
  const userIdentifier = user ? String(user.user_id) : null;

  return (
    <FeatureFlagProvider userIdentifier={userIdentifier}>
      {children}
    </FeatureFlagProvider>
  );
}
