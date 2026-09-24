import { Chess } from "chess.js";

export interface PreMove {
  from: string;
  to: string;
  piece: string;
}

export class PremoveService {
  private premoves: PreMove[] = [];
  private readonly MAX_PREMOVES = 3;

  public addPremove(from: string, to: string, piece: string): void {
    if (this.premoves.length < this.MAX_PREMOVES) {
      this.premoves.push({ from, to, piece });
    }
  }

  public getPremoves(): PreMove[] {
    return [...this.premoves];
  }

  public clearPremoves(): void {
    this.premoves = [];
  }

  public handleOpponentMove(fen: string): {
    executedMove: PreMove | null;
    illegal: boolean;
  } {
    if (this.premoves.length === 0) {
      return { executedMove: null, illegal: false };
    }

    const game = new Chess(fen);
    const nextPremove = this.premoves[0];

    const legalMoves = game.moves({ verbose: true });
    const isLegal = legalMoves.some(
      (move) => move.from === nextPremove.from && move.to === nextPremove.to,
    );

    if (isLegal) {
      this.premoves.shift();
      return { executedMove: nextPremove, illegal: false };
    } else {
      this.clearPremoves();
      return { executedMove: null, illegal: true };
    }
  }
}
