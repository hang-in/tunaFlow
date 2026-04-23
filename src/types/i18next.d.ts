/** i18next 타입 안전성 선언.
 *
 *  `resources` 의 ko 본을 기준으로 키 자동완성 + compile-time 검증. ko 는 SSOT —
 *  새 키는 먼저 ko 에 추가, en 은 번역으로 채움.
 *
 *  i18nPlan INV-5: `namespace.section.action` 3계층 키 강제.
 */
import "i18next";
import type koCommon from "../locales/ko/common.json";
import type koError from "../locales/ko/error.json";
import type koSettings from "../locales/ko/settings.json";
import type koSidebar from "../locales/ko/sidebar.json";
import type koChat from "../locales/ko/chat.json";
import type koDialog from "../locales/ko/dialog.json";
import type koWorkflow from "../locales/ko/workflow.json";
import type koBranch from "../locales/ko/branch.json";
import type koInsight from "../locales/ko/insight.json";

declare module "i18next" {
  interface CustomTypeOptions {
    defaultNS: "common";
    resources: {
      common: typeof koCommon;
      error: typeof koError;
      settings: typeof koSettings;
      sidebar: typeof koSidebar;
      chat: typeof koChat;
      dialog: typeof koDialog;
      workflow: typeof koWorkflow;
      branch: typeof koBranch;
      insight: typeof koInsight;
    };
  }
}
