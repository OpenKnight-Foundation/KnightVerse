"use client";

import { Suspense, useState } from "react";
import Link from "next/link";
import { useSearchParams } from "next/navigation";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { endpoints } from "@/lib/api";

const MIN_PASSWORD_LENGTH = 8;

/**
 * Password reset landing page (BE-159). The emailed link points here with
 * `?token=…`; the new password is posted to `/v1/auth/reset-password`.
 */
function ResetPasswordForm() {
  const token = useSearchParams().get("token") ?? "";
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [done, setDone] = useState(false);
  const [submitting, setSubmitting] = useState(false);

  const handleSubmit = async (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setError(null);
    if (password.length < MIN_PASSWORD_LENGTH) {
      setError(`Password must be at least ${MIN_PASSWORD_LENGTH} characters.`);
      return;
    }
    if (password !== confirm) {
      setError("Passwords do not match.");
      return;
    }
    setSubmitting(true);
    try {
      const res = await fetch(endpoints.auth.resetPassword(), {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ token, password }),
      });
      if (res.status === 400 || res.status === 401 || res.status === 410) {
        throw new Error("This reset link is invalid or has expired. Request a new one.");
      }
      if (!res.ok) throw new Error(`Could not reset password (${res.status}).`);
      setDone(true);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not reset password.");
    } finally {
      setSubmitting(false);
    }
  };

  if (!token) {
    return (
      <p role="alert" className="text-sm text-red-400">
        This reset link is missing its token. Use &quot;Forgot password?&quot; in the sign-in
        dialog to request a new one.
      </p>
    );
  }

  if (done) {
    return (
      <div role="status" className="space-y-3">
        <p className="text-sm text-emerald-400">Your password has been reset.</p>
        <Link href="/" className="text-sm text-blue-400 hover:underline">
          Return to KnightVerse and sign in
        </Link>
      </div>
    );
  }

  return (
    <form onSubmit={handleSubmit} className="space-y-3">
      <div className="space-y-1">
        <label htmlFor="reset-password" className="text-sm font-medium">
          New password
        </label>
        <Input
          id="reset-password"
          type="password"
          autoComplete="new-password"
          minLength={MIN_PASSWORD_LENGTH}
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          required
        />
      </div>
      <div className="space-y-1">
        <label htmlFor="reset-password-confirm" className="text-sm font-medium">
          Confirm new password
        </label>
        <Input
          id="reset-password-confirm"
          type="password"
          autoComplete="new-password"
          value={confirm}
          onChange={(e) => setConfirm(e.target.value)}
          required
        />
      </div>
      {error && (
        <p role="alert" className="text-sm text-red-400">
          {error}
        </p>
      )}
      <Button type="submit" className="w-full" disabled={submitting}>
        {submitting ? "Please wait…" : "Reset password"}
      </Button>
    </form>
  );
}

export default function ResetPasswordPage() {
  return (
    <div className="mx-auto max-w-sm py-16">
      <h1 className="mb-4 text-2xl font-bold text-white">Reset your password</h1>
      <div className="rounded-lg border border-gray-700/50 bg-gray-900/60 p-6">
        <Suspense fallback={<p className="text-sm text-gray-400">Loading…</p>}>
          <ResetPasswordForm />
        </Suspense>
      </div>
    </div>
  );
}
