"use client";

import React, { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useAuth } from "@/context/authContext";

/**
 * Sign-in / register modal wired to the auth API (#FE-96).
 *
 * Accepts either `open`/`onOpenChange` (Header) or `isOpen`/`onClose`
 * (AuthGate), so both callers share one modal.
 */
export interface SignInModalProps {
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
  isOpen?: boolean;
  onClose?: () => void;
}

type Mode = "login" | "register";

export function SignInModal({ open, onOpenChange, isOpen, onClose }: SignInModalProps) {
  const visible = open ?? isOpen ?? false;
  const { login, register } = useAuth();

  const [mode, setMode] = useState<Mode>("login");
  const [username, setUsername] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  const close = () => {
    onOpenChange?.(false);
    onClose?.();
  };

  // Reset transient state whenever the modal is reopened.
  useEffect(() => {
    if (visible) {
      setError(null);
      setPassword("");
    }
  }, [visible]);

  useEffect(() => {
    if (!visible) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") close();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [visible]);

  if (!visible) return null;

  const handleSubmit = async (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setError(null);
    setSubmitting(true);
    try {
      if (mode === "login") {
        await login(username, password);
      } else {
        await register(username, email, password);
      }
      close();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Sign in failed");
    } finally {
      setSubmitting(false);
    }
  };

  const title = mode === "login" ? "Sign in" : "Create account";

  return (
    <div
      className="fixed inset-0 z-[60] flex items-center justify-center bg-black/60 p-4"
      onClick={close}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby="sign-in-modal-title"
        data-testid="sign-in-modal"
        className="w-full max-w-sm rounded-lg border border-border bg-card p-6 shadow-xl"
        onClick={(event) => event.stopPropagation()}
      >
        <h2 id="sign-in-modal-title" className="mb-4 text-lg font-semibold">
          {title}
        </h2>

        <form onSubmit={handleSubmit} className="space-y-3">
          <div className="space-y-1">
            <label htmlFor="sign-in-username" className="text-sm font-medium">
              Username
            </label>
            <Input
              id="sign-in-username"
              autoComplete="username"
              value={username}
              onChange={(event) => setUsername(event.target.value)}
              required
            />
          </div>

          {mode === "register" && (
            <div className="space-y-1">
              <label htmlFor="sign-in-email" className="text-sm font-medium">
                Email
              </label>
              <Input
                id="sign-in-email"
                type="email"
                autoComplete="email"
                value={email}
                onChange={(event) => setEmail(event.target.value)}
                required
              />
            </div>
          )}

          <div className="space-y-1">
            <label htmlFor="sign-in-password" className="text-sm font-medium">
              Password
            </label>
            <Input
              id="sign-in-password"
              type="password"
              autoComplete={mode === "login" ? "current-password" : "new-password"}
              value={password}
              onChange={(event) => setPassword(event.target.value)}
              required
            />
          </div>

          {error && (
            <p role="alert" data-testid="sign-in-error" className="text-sm text-red-500">
              {error}
            </p>
          )}

          <Button
            type="submit"
            className="w-full"
            disabled={submitting}
            data-testid="sign-in-submit"
          >
            {submitting ? "Please wait…" : title}
          </Button>
        </form>

        <div className="mt-4 flex items-center justify-between text-sm">
          <button
            type="button"
            className="text-blue-500 hover:underline"
            onClick={() => {
              setMode(mode === "login" ? "register" : "login");
              setError(null);
            }}
          >
            {mode === "login" ? "Create an account" : "Have an account? Sign in"}
          </button>
          <button type="button" className="text-muted-foreground hover:underline" onClick={close}>
            Cancel
          </button>
        </div>
      </div>
    </div>
  );
}

export default SignInModal;
