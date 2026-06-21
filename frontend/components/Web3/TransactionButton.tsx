"use client";

import React, { useState } from "react";
import { Button } from "@/components/ui/button";
import { Spinner } from "@/components/ui/spinner";
import { cn } from "@/lib/utils";

interface TransactionButtonProps
  extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  onTransaction: () => Promise<void>;
  loadingText?: string;
  successText?: string;
  variant?: "default" | "destructive" | "outline" | "secondary" | "ghost" | "link";
  size?: "default" | "sm" | "lg" | "icon";
}

/**
 * TransactionButton — Smart button with built-in transaction state management
 * Handles loading, success, and error states automatically
 */
export function TransactionButton({
  onTransaction,
  loadingText = "Processing...",
  successText = "Success!",
  children,
  variant = "default",
  size = "default",
  className,
  disabled,
  ...props
}: TransactionButtonProps) {
  const [state, setState] = useState<"idle" | "loading" | "success" | "error">(
    "idle"
  );

  const handleClick = async () => {
    if (state === "loading") return;

    setState("loading");
    try {
      await onTransaction();
      setState("success");
      setTimeout(() => setState("idle"), 2000);
    } catch (error) {
      setState("error");
      setTimeout(() => setState("idle"), 3000);
    }
  };

  const isDisabled = disabled || state === "loading" || state === "success";

  return (
    <Button
      onClick={handleClick}
      disabled={isDisabled}
      variant={state === "error" ? "destructive" : variant}
      size={size}
      className={cn(
        "relative transition-all duration-300",
        state === "success" && "bg-emerald-600 hover:bg-emerald-600",
        className
      )}
      {...props}
    >
      <span
        className={cn(
          "flex items-center gap-2 transition-opacity duration-200",
          state !== "idle" && "opacity-0"
        )}
      >
        {children}
      </span>

      {state === "loading" && (
        <span className="absolute inset-0 flex items-center justify-center gap-2">
          <Spinner size="sm" />
          <span className="text-sm">{loadingText}</span>
        </span>
      )}

      {state === "success" && (
        <span className="absolute inset-0 flex items-center justify-center gap-2 animate-scale-in">
          <svg
            className="h-4 w-4"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth={2}
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              d="M5 13l4 4L19 7"
            />
          </svg>
          <span className="text-sm">{successText}</span>
        </span>
      )}

      {state === "error" && (
        <span className="absolute inset-0 flex items-center justify-center gap-2 animate-scale-in">
          <svg
            className="h-4 w-4"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth={2}
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              d="M6 18L18 6M6 6l12 12"
            />
          </svg>
          <span className="text-sm">Failed</span>
        </span>
      )}
    </Button>
  );
}
