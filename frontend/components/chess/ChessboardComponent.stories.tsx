import React from "react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { BoardSkeleton } from "@/components/ui/LoadingSkeleton";
import { AuthProvider } from "@/context/authContext";
import { BoardThemeProvider } from "@/context/ThemeContext";
import { GamePreferencesProvider } from "@/context/GamePreferencesContext";

import ChessboardComponent from "./ChessboardComponent";

type ChessboardProps = React.ComponentProps<typeof ChessboardComponent>;
type StoryComponent = React.ComponentType;
type ThemeName = "light" | "dark";
type Story = {
  args?: Partial<ChessboardProps>;
  globals?: { theme?: ThemeName };
  parameters?: Record<string, unknown>;
  render?: () => React.ReactElement;
};

const START_FEN = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
const MIDGAME_FEN =
  "r1bqkbnr/pppp1ppp/2n5/1B2p3/4P3/5N2/PPPP1PPP/RNBQK2R w KQkq - 0 4";

const withProviders = (Story: StoryComponent) => (
  <AuthProvider>
    <BoardThemeProvider>
      <GamePreferencesProvider>
        <div className="max-w-[560px] p-4">
          <Story />
        </div>
      </GamePreferencesProvider>
    </BoardThemeProvider>
  </AuthProvider>
);

const meta = {
  title: "Chess/Chessboard",
  component: ChessboardComponent,
  decorators: [withProviders],
  args: { position: START_FEN, onDrop: () => true } satisfies ChessboardProps,
  parameters: { layout: "centered" },
};

export default meta;

export const StartPosition: Story = {};

export const MidgamePosition: Story = {
  args: { position: MIDGAME_FEN, lastMove: { from: "e2", to: "e4" } },
};

export const DarkTheme: Story = {
  args: { position: MIDGAME_FEN },
  globals: { theme: "dark" },
};

export const LightTheme: Story = {
  args: { position: MIDGAME_FEN },
  globals: { theme: "light" },
};

export const Loading: Story = {
  render: () => <BoardSkeleton />,
};

export const ErrorState: Story = {
  render: () => (
    <Alert variant="destructive">
      <AlertTitle>Chessboard unavailable</AlertTitle>
      <AlertDescription>The board could not be loaded.</AlertDescription>
    </Alert>
  ),
};
