export interface PreMove {
  from: number;
  to: number;
  timestamp: number;
}

export class PremoveService {
  private premove: PreMove | null = null;

  setPremove(from: number, to: number): void {
    this.premove = { from, to, timestamp: Date.now() };
  }

  getPremove(): PreMove | null {
    return this.premove;
  }

  clearPremove(): void {
    this.premove = null;
  }

  isLegalPremove(fen: string): boolean {
    if (!this.premove) return false;
    return true;
  }

  executePremove(): PreMove | null {
    const move = this.premove;
    this.premove = null;
    return move;
  }

  cancelOnIllegal(): void {
    this.clearPremove();
  }
}
