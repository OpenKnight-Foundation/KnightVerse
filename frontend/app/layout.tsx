import type { Metadata } from "next";
import "./globals.css";
import ClientRoot from "@/components/ClientRoot";
import { AppProvider } from "@/context/walletContext";
import { MatchmakingProvider } from "@/context/matchmakingContext";
import { ToastProvider } from "@/components/ui/toast";
import { TransactionProvider } from "@/context/transactionContext";
import { ThemeProvider } from "next-themes";
import { EnhancedHeader } from "@/components/EnhancedHeader";
import { EnhancedTransactionStatus } from "@/components/Web3/EnhancedTransactionStatus";

export const metadata: Metadata = {
  title: "XLMate — Chess on Stellar",
  description: "Play chess on the Stellar blockchain. Compete, earn rewards, and climb the leaderboard.",
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en" suppressHydrationWarning>
      <body className="antialiased font-rowdies bg-background text-foreground">
        <a
          href="#main-content"
          className="sr-only focus:not-sr-only focus:fixed focus:top-4 focus:left-4 focus:z-[200] focus:px-4 focus:py-2 focus:rounded-lg focus:bg-teal-600 focus:text-white focus:shadow-lg focus:outline-none focus:ring-2 focus:ring-teal-400"
        >
          Skip to main content
        </a>
        <ThemeProvider attribute="class" defaultTheme="dark" enableSystem disableTransitionOnChange>
          <AppProvider>
            <MatchmakingProvider>
              <ToastProvider>
                <TransactionProvider>
                  <EnhancedHeader />
                  <main id="main-content" className="min-h-[calc(100vh-4rem)]">
                    <ClientRoot>{children}</ClientRoot>
                  </main>
                  <EnhancedTransactionStatus />
                </TransactionProvider>
              </ToastProvider>
            </MatchmakingProvider>
          </AppProvider>
        </ThemeProvider>
      </body>
    </html>
  );
}
