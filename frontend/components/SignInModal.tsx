"use client";

import React, { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useAuth } from "@/context/authContext";
import { endpoints } from "@/lib/api";

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

type Mode = "login" | "register" | "forgot";

/**
 * Requests a password-reset email (BE-159). The backend should answer 2xx
 * whether or not the account exists, so the UI never reveals that.
 */
export async function requestPasswordReset(email: string): Promise<void> {
  const res = await fetch(endpoints.auth.forgotPassword(), {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ email }),
  });
  if (res.status === 429) throw new Error("Too many requests — please try again later.");
  if (!res.ok && res.status >= 500) {
    throw new Error("Could not send the reset email. Please try again.");
  }
}

export function SignInModal({ open, onOpenChange, isOpen, onClose }: SignInModalProps) {
  const visible = open ?? isOpen ?? false;
  const { login, register } = useAuth();

  const [mode, setMode] = useState<Mode>("login");
  const [username, setUsername] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  const close = () => {
    onOpenChange?.(false);
    onClose?.();
  };

  // Reset transient state whenever the modal is reopened.
  useEffect(() => {
    if (visible) {
      setError(null);
      setNotice(null);
      setPassword("");
      setMode((m) => (m === "forgot" ? "login" : m));
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
    setNotice(null);
    setSubmitting(true);
    try {
      if (mode === "forgot") {
        await requestPasswordReset(email.trim());
        setNotice(
          "If an account exists for that email, a password reset link is on its way. Check your inbox.",
        );
        return;
      }
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

  const switchMode = (next: Mode) => {
    setMode(next);
    setError(null);
    setNotice(null);
  };

  const title =
    mode === "login" ? "Sign in" : mode === "register" ? "Create account" : "Reset password";

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
          {mode === "forgot" && (
            <p className="text-sm text-muted-foreground">
              Enter the email on your account and we&apos;ll send you a link to reset your password.
            </p>
          )}

          {mode !== "forgot" && (
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
          )}

          {mode !== "login" && (
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

          {mode !== "forgot" && (
          <div className="space-y-1">
            <div className="flex items-center justify-between">
              <label htmlFor="sign-in-password" className="text-sm font-medium">
                Password
              </label>
              {mode === "login" && (
                <button
                  type="button"
                  className="text-xs text-blue-500 hover:underline"
                  onClick={() => switchMode("forgot")}
                  data-testid="forgot-password-link"
                >
                  Forgot password?
                </button>
              )}
            </div>
            <Input
              id="sign-in-password"
              type="password"
              autoComplete={mode === "login" ? "current-password" : "new-password"}
              value={password}
              onChange={(event) => setPassword(event.target.value)}
              required
            />
          </div>
          )}

          {error && (
            <p role="alert" data-testid="sign-in-error" className="text-sm text-red-500">
              {error}
            </p>
          )}

          {notice && (
            <p role="status" data-testid="sign-in-notice" className="text-sm text-emerald-500">
              {notice}
            </p>
          )}

          <Button
            type="submit"
            className="w-full"
            disabled={submitting}
            data-testid="sign-in-submit"
          >
            {submitting ? "Please wait…" : mode === "forgot" ? "Send reset link" : title}
          </Button>
        </form>

        <div className="mt-4 flex items-center justify-between text-sm">
          <button
            type="button"
            className="text-blue-500 hover:underline"
            onClick={() => switchMode(mode === "login" ? "register" : "login")}
          >
            {mode === "login" ? "Create an account" : mode === "forgot" ? "Back to sign in" : "Have an account? Sign in"}
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
