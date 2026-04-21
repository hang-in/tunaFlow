// Scenario 1: 프로젝트 첫 생성 → Main 대화 → 첫 응답
// API coverage: 100%. Validates POST /projects, /conversations, /send + WS streaming.
//
// Uses a test project path that is unique per run (timestamp suffix) so repeated
// executions don't collide. Does NOT clean up the project — leave it for manual
// inspection. Run in an isolated working tunaFlow instance.

import { api, ws, assert, log, runScenario } from "./lib.mjs";

runScenario("scenario-01 project-and-message", async () => {
  const suffix = Date.now();
  const projectKey = `e2e-p-${suffix}`;
  const projectPath = process.env.E2E_PROJECT_PATH ?? "/tmp/tunaflow-e2e-dummy";

  log.info(`creating project ${projectKey}`);
  await api("/api/v1/projects", {
    method: "POST",
    body: JSON.stringify({
      key: projectKey,
      name: `E2E Scenario 1 (${suffix})`,
      path: projectPath,
    }),
  });

  // List and verify presence
  const projects = await api("/api/v1/projects");
  assert(
    projects.some((p) => p.key === projectKey),
    `project ${projectKey} missing from list`
  );
  log.ok(`project ${projectKey} created`);

  // Create conversation
  const conv = await api("/api/v1/conversations", {
    method: "POST",
    body: JSON.stringify({ projectKey, label: "[E2E-1] Main", mode: "chat" }),
  });
  assert(conv.id, "conversation id missing from response");
  log.ok(`conversation ${conv.id} created`);

  // Subscribe WS before sending
  const socket = ws();
  // Give WS time to open
  await new Promise((r) => setTimeout(r, 500));

  const sendPromise = api(`/api/v1/conversations/${conv.id}/send`, {
    method: "POST",
    body: JSON.stringify({
      prompt: "안녕하세요. '테스트 완료' 라고만 답해주세요.",
      engine: process.env.E2E_ENGINE ?? "claude",
    }),
  });

  // Wait for `agent:completed` — emitted after the tool-request loop exits
  // (see src-tauri/src/http_api/agents.rs:294).
  const done = await socket.waitFor(
    (e) => e.type === "agent:completed" && e.conversationId === conv.id,
    120000
  );
  log.ok(`received completion event: ${done.type}`);

  await sendPromise.catch((e) => {
    // /send returns 202 (queued) with runId — not an actual failure
    log.info(`send returned: ${e.message ?? "ok"}`);
  });

  // Confirm via GET messages
  const msgs = await api(`/api/v1/conversations/${conv.id}/messages`);
  assert(msgs.length >= 2, `expected 2+ messages, got ${msgs.length}`);
  const assistantMsg = msgs.find((m) => m.role === "assistant" && m.content);
  assert(assistantMsg, "no assistant message found");
  log.ok(`assistant responded: "${assistantMsg.content.slice(0, 40)}…"`);

  socket.close();
});
