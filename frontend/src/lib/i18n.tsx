// Lightweight i18n context and hook for AEGIS translations
import { createContext, useContext, useCallback } from 'react';
import type { ReactNode } from 'react';
import translations, { type Language } from './translations.js';

export type { Language };

interface I18nContextValue {
  lang: Language;
  t: (key: string) => string;
}

export const I18nContext = createContext<I18nContextValue>({
  lang: 'en',
  t: (key: string) => translations.en[key] ?? key,
});

export function useTranslate() {
  const ctx = useContext(I18nContext);
  return ctx;
}

export function useT() {
  const { t } = useContext(I18nContext);
  return t;
}

export function I18nProvider({ lang, children }: { lang: Language; children: ReactNode }) {
  const t = useCallback((key: string) => {
    return translations[lang]?.[key] ?? translations.en[key] ?? key;
  }, [lang]);

  return (
    <I18nContext.Provider value={{ lang, t }}>
      {children}
    </I18nContext.Provider>
  );
}
