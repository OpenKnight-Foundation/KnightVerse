import React from "react";

import { ToastProvider } from "@/components/ui/toast";
import { TransactionProvider } from "@/context/transactionContext";
import { AppContext, type AppContextType } from "@/context/walletContext";

import { WalletButton } from "./WalletButton";

type WalletPreviewProps = {
  status: AppContextType["status"];
  address?: string;
};

type ThemeName = "light" | "dark";
type Story = {
  args?: Partial<WalletPreviewProps>;
  globals?: { theme?: ThemeName };
  parameters?: Record<string, unknown>;
};

const WALLET = "GAAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQDZ7H";

const unavailableClient = (() => {
  throw new Error("Contract client is unavailable in Storybook");
}) as AppContextType["getContractClient"];

function WalletButtonPreview({ status, address }: WalletPreviewProps) {
  const value: AppContextType = {
    address,
    status,
    balance: undefined,
    connectWallet: async () => undefined,
    disconnectWallet: async () => undefined,
    sendPayment: async () => undefined,
    sendXLM: async () => undefined,
    signTransaction: async () => "",
    invokeBoundContract: async () => undefined,
    invokeSorobanContract: async () => undefined,
    getContractClient: unavailableClient,
  };

  return (
    <AppContext.Provider value={value}>
      <ToastProvider>
        <TransactionProvider>
          <div className="p-4">
            <WalletButton />
          </div>
        </TransactionProvider>
      </ToastProvider>
    </AppContext.Provider>
  );
}

const meta = {
  title: "Wallet/WalletButton",
  component: WalletButtonPreview,
  args: { status: "disconnected" } satisfies WalletPreviewProps,
  parameters: { layout: "centered" },
};

export default meta;

export const Disconnected: Story = {};

export const Connecting: Story = {
  args: { status: "connecting" },
};

export const Connected: Story = {
  args: { status: "connected", address: WALLET },
};

export const ErrorState: Story = {
  args: { status: "error" },
};

export const DarkTheme: Story = {
  args: { status: "connected", address: WALLET },
  globals: { theme: "dark" },
};

export const LightTheme: Story = {
  args: { status: "connected", address: WALLET },
  globals: { theme: "light" },
};
