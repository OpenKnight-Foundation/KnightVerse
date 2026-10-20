"use client";

import React, { useState } from "react";
import { Button } from "@/components/ui/button";
import { SignInModal } from "@/components/SignInModal";
import { useAuth } from "@/context/authContext";

/**
 * Header sign-in control (#FE-96): opens the sign-in modal when signed out and
 * shows the username once signed in.
 */
export function AuthButton({ className = "" }: { className?: string }) {
  const { user } = useAuth();
  const [open, setOpen] = useState(false);

  if (user) {
    return (
      <span
        data-testid="header-username"
        className={`text-sm font-medium text-gray-200 ${className}`}
      >
        {user.username}
      </span>
    );
  }

  return (
    <>
      <Button
        variant="ghost"
        size="sm"
        onClick={() => setOpen(true)}
        data-testid="header-sign-in"
        className={className}
      >
        Sign In
      </Button>
      <SignInModal open={open} onOpenChange={setOpen} />
    </>
  );
}

export default AuthButton;
