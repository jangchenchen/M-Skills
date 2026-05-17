import i18n from "i18next";
import { initReactI18next } from "react-i18next";

import enCommon from "./locales/en/common.json";
import enArtifact from "./locales/en/artifact.json";
import enWizard from "./locales/en/wizard.json";
import enErrors from "./locales/en/errors.json";
import enSettings from "./locales/en/settings.json";

import zhCommon from "./locales/zh/common.json";
import zhArtifact from "./locales/zh/artifact.json";
import zhWizard from "./locales/zh/wizard.json";
import zhErrors from "./locales/zh/errors.json";
import zhSettings from "./locales/zh/settings.json";

const stored = localStorage.getItem("m-skills-lang");
const browserLang = navigator.language.startsWith("zh") ? "zh" : "en";
const initialLang = stored ?? browserLang;

i18n.use(initReactI18next).init({
  resources: {
    en: {
      common: enCommon,
      artifact: enArtifact,
      wizard: enWizard,
      errors: enErrors,
      settings: enSettings,
    },
    zh: {
      common: zhCommon,
      artifact: zhArtifact,
      wizard: zhWizard,
      errors: zhErrors,
      settings: zhSettings,
    },
  },
  lng: initialLang,
  fallbackLng: "en",
  defaultNS: "common",
  interpolation: { escapeValue: false },
});

export default i18n;
