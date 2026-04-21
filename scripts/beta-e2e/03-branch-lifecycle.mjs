// Scenario 3: Branch 생성 → adopt / archive / rename / delete
// API coverage: 100%. All branch lifecycle endpoints.

import { api, assert, log, runScenario } from "./lib.mjs";

runScenario("scenario-03 branch-lifecycle", async () => {
  const suffix = Date.now();
  const projectKey = process.env.E2E_PROJECT_KEY ?? `e2e-p-${suffix}`;
  const existing = await api("/api/v1/projects");
  if (!existing.some((p) => p.key === projectKey)) {
    await api("/api/v1/projects", {
      method: "POST",
      body: JSON.stringify({
        key: projectKey,
        name: `E2E-3 (${suffix})`,
        path: process.env.E2E_PROJECT_PATH ?? "/tmp/tunaflow-e2e-dummy",
      }),
    });
  }

  const conv = await api("/api/v1/conversations", {
    method: "POST",
    body: JSON.stringify({ projectKey, label: "[E2E-3] parent", mode: "chat" }),
  });

  // Create branch
  const br = await api("/api/v1/branches", {
    method: "POST",
    body: JSON.stringify({
      conversationId: conv.id,
      label: "test-branch-a",
      mode: "chat",
    }),
  });
  assert(br.id, "branch id missing");
  log.ok(`branch created: ${br.id}`);

  // List branches on the conversation
  const list = await api(`/api/v1/conversations/${conv.id}/branches`);
  assert(
    list.some((b) => b.id === br.id),
    "branch missing from list"
  );
  log.ok(`branch listed`);

  // Get detail (new in 2-2)
  const detail = await api(`/api/v1/branches/${br.id}`);
  assert(detail.id === br.id, "detail id mismatch");
  log.ok(`branch detail ok`);

  // Rename
  await api(`/api/v1/branches/${br.id}/rename`, {
    method: "POST",
    body: JSON.stringify({ label: "test-branch-renamed" }),
  });
  const detail2 = await api(`/api/v1/branches/${br.id}`);
  assert(
    detail2.label === "test-branch-renamed",
    `rename failed, got ${detail2.label}`
  );
  log.ok(`rename ok`);

  // Archive
  await api(`/api/v1/branches/${br.id}/archive`, { method: "POST" });
  const detail3 = await api(`/api/v1/branches/${br.id}`);
  assert(
    detail3.status === "archived",
    `archive failed, got ${detail3.status}`
  );
  log.ok(`archive ok`);

  // Create a second branch to test adopt
  const br2 = await api("/api/v1/branches", {
    method: "POST",
    body: JSON.stringify({
      conversationId: conv.id,
      label: "adoptable",
      mode: "chat",
    }),
  });
  await api(`/api/v1/branches/${br2.id}/adopt`, {
    method: "POST",
    body: JSON.stringify({ targetConversationId: conv.id }),
  });
  const detail4 = await api(`/api/v1/branches/${br2.id}`);
  assert(detail4.status === "adopted", `adopt failed, got ${detail4.status}`);
  log.ok(`adopt ok`);

  // Delete the archived one
  await api(`/api/v1/branches/${br.id}`, { method: "DELETE" });
  log.ok(`delete ok`);
});
