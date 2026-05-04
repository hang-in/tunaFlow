/**
 * MetaFloatingChat — askMeta UX 폐지 (PR-1, T04) 회귀 가드.
 *
 * 사용자가 *"메타에게 물어보기"* 버튼 / `askMetaAbout` callback / 관련 i18n 키
 * (`action_ask_meta`, `ask_about_*`) 가 다시 추가되지 않도록 source-level 검증.
 *
 * Plan: docs/plans/reviewerVerdictDirectArchitectPlan_2026-05-04.md (T07)
 */
import { describe, it, expect } from "vitest";
// @ts-expect-error — node built-in available under vitest runtime, types optional here
import { readFileSync } from "node:fs";
// @ts-expect-error — node built-in available under vitest runtime
import { resolve, dirname } from "node:path";
// @ts-expect-error — node built-in available under vitest runtime
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const componentPath = resolve(__dirname, "MetaFloatingChat.tsx");
const koDialogPath = resolve(__dirname, "../../locales/ko/dialog.json");
const enDialogPath = resolve(__dirname, "../../locales/en/dialog.json");

describe("MetaFloatingChat — askMeta UX removed", () => {
  it("source does not reference askMetaAbout callback", () => {
    const src = readFileSync(componentPath, "utf-8");
    expect(src).not.toMatch(/askMetaAbout/);
  });

  it("source does not reference action_ask_meta i18n key", () => {
    const src = readFileSync(componentPath, "utf-8");
    expect(src).not.toMatch(/action_ask_meta/);
  });

  it("source does not reference ask_about_* i18n keys", () => {
    const src = readFileSync(componentPath, "utf-8");
    expect(src).not.toMatch(/ask_about_/);
  });

  it("source still provides routeTo callback (INV-RVA-7: route navigation 보존)", () => {
    const src = readFileSync(componentPath, "utf-8");
    expect(src).toMatch(/const routeTo = useCallback/);
  });

  it("source still provides Inbox tab + Chat tab (Meta floating chat 자체 보존)", () => {
    const src = readFileSync(componentPath, "utf-8");
    expect(src).toMatch(/tab_inbox/);
    expect(src).toMatch(/tab_chat/);
  });
});

describe("locales/dialog.json — askMeta keys removed", () => {
  it("ko/dialog.json has no action_ask_meta or ask_about_* keys", () => {
    const ko = JSON.parse(readFileSync(koDialogPath, "utf-8"));
    const meta = ko.meta_chat;
    expect(meta).toBeDefined();
    expect(meta.action_ask_meta).toBeUndefined();
    expect(meta.ask_about_header).toBeUndefined();
    expect(meta.ask_about_summary).toBeUndefined();
    expect(meta.ask_about_instruction).toBeUndefined();
  });

  it("en/dialog.json has no action_ask_meta or ask_about_* keys", () => {
    const en = JSON.parse(readFileSync(enDialogPath, "utf-8"));
    const meta = en.meta_chat;
    expect(meta).toBeDefined();
    expect(meta.action_ask_meta).toBeUndefined();
    expect(meta.ask_about_header).toBeUndefined();
    expect(meta.ask_about_summary).toBeUndefined();
    expect(meta.ask_about_instruction).toBeUndefined();
  });

  it("ko/dialog.json still has action_navigate and action_dismiss (route + dismiss UX 보존)", () => {
    const ko = JSON.parse(readFileSync(koDialogPath, "utf-8"));
    expect(ko.meta_chat.action_navigate).toBeDefined();
    expect(ko.meta_chat.action_dismiss).toBeDefined();
  });
});
