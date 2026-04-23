import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import LanguageDetector from "i18next-browser-languagedetector";

import koCommon from "./ko/common.json";
import koError from "./ko/error.json";
import koSettings from "./ko/settings.json";
import koSidebar from "./ko/sidebar.json";
import koChat from "./ko/chat.json";
import koDialog from "./ko/dialog.json";
import enCommon from "./en/common.json";
import enError from "./en/error.json";
import enSettings from "./en/settings.json";
import enSidebar from "./en/sidebar.json";
import enChat from "./en/chat.json";
import enDialog from "./en/dialog.json";

/** Supported locales. Add 'ja', 'zh', etc. in later PRs. */
export type SupportedLocale = "ko" | "en";
export const SUPPORTED_LOCALES: SupportedLocale[] = ["ko", "en"];
export const DEFAULT_LOCALE: SupportedLocale = "ko";

/** appStore key — persisted across app restarts. */
const LOCALE_STORAGE_KEY = "tunaflow.locale";

export const resources = {
  ko: {
    common: koCommon,
    error: koError,
    settings: koSettings,
    sidebar: koSidebar,
    chat: koChat,
    dialog: koDialog,
  },
  en: {
    common: enCommon,
    error: enError,
    settings: enSettings,
    sidebar: enSidebar,
    chat: enChat,
    dialog: enDialog,
  },
} as const;

i18n
  .use(LanguageDetector)
  .use(initReactI18next)
  .init({
    resources,
    // Fallback: English. LLM 출력 / 기술 용어가 영어라 기본 정합성이 높음.
    fallbackLng: "en",
    // 누락 키는 빈 문자열 대신 키 그대로 표시 → 누락 즉시 인지.
    returnEmptyString: false,
    defaultNS: "common",
    ns: ["common", "error", "settings", "sidebar", "chat", "dialog"],
    detection: {
      // appStore (IndexedDB) 우선, 그 다음 localStorage/navigator.
      order: ["localStorage", "navigator"],
      lookupLocalStorage: LOCALE_STORAGE_KEY,
      caches: ["localStorage"],
    },
    interpolation: {
      escapeValue: false, // React 가 자체 이스케이프
    },
  });

/** 현재 locale. React 외부 (Rust invoke 응답 포맷팅 등) 에서 쓸 때 사용. */
export function getCurrentLocale(): SupportedLocale {
  const raw = i18n.language;
  const base = raw?.split("-")[0] ?? DEFAULT_LOCALE;
  return (SUPPORTED_LOCALES as string[]).includes(base) ? (base as SupportedLocale) : DEFAULT_LOCALE;
}

/** appStore 와 i18n 양쪽에 locale 저장. Settings 드롭다운이 호출. */
export async function setLocale(locale: SupportedLocale): Promise<void> {
  try {
    localStorage.setItem(LOCALE_STORAGE_KEY, locale);
  } catch (e) {
    console.warn("[i18n] localStorage.setItem failed", e);
  }
  await i18n.changeLanguage(locale);
}

export default i18n;
