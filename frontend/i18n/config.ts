export const locales = ["en", "es"] as const;

export type Locale = (typeof locales)[number];

export const defaultLocale: Locale = "en";

export const LOCALE_STORAGE_KEY = "chess-locale";

export const localeNames: Record<Locale, string> = {
  en: "English",
  es: "Español",
};

export function isLocale(value: string | null | undefined): value is Locale {
  return !!value && (locales as readonly string[]).includes(value);
}
