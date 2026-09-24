export type CompanionEmotion = "confident" | "panicked" | "calculating" | "celebratory" | "thoughtful";

export function getCompanionEmotion(evaluation: number | null, isVictory = false, isTactical = false): CompanionEmotion {
  if (isVictory) return "celebratory";
  if (evaluation !== null && evaluation > 2) return "confident";
  if (evaluation !== null && evaluation < -2) return "panicked";
  if (isTactical) return "calculating";
  return "thoughtful";
}