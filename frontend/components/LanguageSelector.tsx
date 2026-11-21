"use client";

import React from "react";
import { locales, localeNames, LOCALE_STORAGE_KEY, defaultLocale, isLocale, type Locale } from "@/i18n/config";
import { useI18n } from "@/i18n/I18nProvider";

export function LanguageSelector() {
  const { locale, setLocale, t } = useI18n();

  const handleChange = (event: React.ChangeEvent<HTMLSelectElement>) => {
    const value = event.target.value;
    if (isLocale(value)) {
      setLocale(value);
    } else {
      setLocale(defaultLocale);
    }
  };

  return (
    <div className="space-y-2">
      <label htmlFor="language-select" className="block text-sm font-medium text-white">
        {t("settings.language")}
      </label>
      <p className="text-sm text-gray-400">{t("settings.languageDescription")}</p>
      <select
        id="language-select"
        value={locale}
        onChange={handleChange}
        className="w-full rounded-md bg-gray-800 border border-gray-700 px-3 py-2 text-white"
      >
        {locales.map((code: Locale) => (
          <option key={code} value={code}>
            {localeNames[code]}
          </option>
        ))}
      </select>
    </div>
  );
}

export { LOCALE_STORAGE_KEY };
