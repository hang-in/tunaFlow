// Scenario 8: WS 구독 → disconnect → ?since=<ms> replay
// Validates `ws_event_log` append-and-replay contract via branch events
// (which actually get broadcast, unlike conversation:deleted).

import { api, ws, assert, log, runScenario } from "./lib.mjs";

runScenario("scenario-08 ws-replay", async () => {
  const suffix = Date.now();
  const projectKey = process.env.E2E_PROJECT_KEY ?? `e2e-p-${suffix}`;
  const existing = await api("/api/v1/projects");
  if (!existing.some((p) => p.key === projectKey)) {
    await api("/api/v1/projects", {
      method: "POST",
      body: JSON.stringify({
        key: projectKey,
        name: `E2E-8 (${suffix})`,
        path: process.env.E2E_PROJECT_PATH ?? "/tmp/tunaflow-e2e-dummy",
      }),
    });
  }

  const conv = await api("/api/v1/conversations", {
    method: "POST",
    body: JSON.stringify({
      projectKey,
      label: "[E2E-8] replay test",
      mode: "chat",
    }),
  });

  // ── 1. First WS connects, receives branch:created event
  const socket1 = ws();
  await new Promise((r) => setTimeout(r, 500));
  log.ok(`first WS connected`);

  const br = await api("/api/v1/branches", {
    method: "POST",
    body: JSON.stringify({
      conversationId: conv.id,
      label: "e2e8-branch",
      mode: "chat",
    }),
  });

  await socket1
    .waitFor(
      (e) => e.type === "branch:created" && e.branchId === br.id,
      5000
    )
    .catch(() => {
      throw new Error("branch:created not seen on first socket");
    });
  log.ok(`first socket received branch:created`);

  // ── 2. Capture since cursor, close first socket
  const sinceMs = Date.now();
  socket1.close();
  log.info(`first socket closed at ${sinceMs}`);

  // Small delay so second archive event is unambiguously after sinceMs.
  await new Promise((r) => setTimeout(r, 100));

  // ── 3. Trigger events while disconnected
  await api(`/api/v1/branches/${br.id}/archive`, { method: "POST" });
  await new Promise((r) => setTimeout(r, 300)); // let event_log flush

  // ── 4. Reconnect with since → replay should include branch:archived
  const socket2 = ws({ since: sinceMs });
  await socket2
    .waitFor(
      (e) => e.type === "branch:archived" && e.branchId === br.id,
      5000
    )
    .catch((err) => {
      log.fail(
        `replay missing branch:archived; got ${socket2.events.length} events: ${socket2.events.map((e) => e.type).join(", ")}`
      );
      throw err;
    });
  log.ok(`replay delivered branch:archived`);

  socket2.close();
  assert(socket2.events.length > 0, "replay socket received nothing");
  log.ok(`ws replay verified`);
});
