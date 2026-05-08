export class SecurityGuard {
  private blockedPatterns = [
    /ignore previous instructions/i,
    /system override/i,
    /execute shell/i,
  ];

  validatePrompt(input: string): boolean {
    return !this.blockedPatterns.some((pattern) =>
      pattern.test(input)
    );
  }

  sanitize(input: string): string {
    return input.replace(/[<>$;]/g, "");
  }
}