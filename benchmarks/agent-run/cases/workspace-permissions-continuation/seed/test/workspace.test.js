const assert = require("node:assert/strict");
const test = require("node:test");

const { createWorkspaceRegistry } = require("../src/workspace");

test("adds and lists workspace members", () => {
  const registry = createWorkspaceRegistry({ now: () => new Date("2026-06-01T00:00:00.000Z") });

  const member = registry.addMember({ id: "u1", email: "owner@example.com", role: "owner" });

  assert.equal(member.role, "owner");
  assert.deepEqual(registry.listMembers({ role: "owner" }).map((item) => item.id), ["u1"]);
});

test("updates and removes members", () => {
  const registry = createWorkspaceRegistry({ now: () => new Date("2026-06-01T00:00:00.000Z") });
  registry.addMember({ id: "u1", email: "editor@example.com" });

  assert.equal(registry.setRole("u1", "editor").role, "editor");
  assert.equal(registry.removeMember("u1"), true);
  assert.deepEqual(registry.listMembers(), []);
});

test("creates and lists invitations", () => {
  const registry = createWorkspaceRegistry({ now: () => new Date("2026-06-01T00:00:00.000Z") });

  const invite = registry.inviteMember({ email: "new@example.com", role: "editor" });

  assert.equal(invite.status, "pending");
  assert.deepEqual(registry.listInvites({ status: "pending" }).map((item) => item.id), [invite.id]);
});

test("summarizes workspace counts", () => {
  const registry = createWorkspaceRegistry({ now: () => new Date("2026-06-01T00:00:00.000Z") });
  registry.addMember({ id: "u1", email: "owner@example.com" });
  registry.inviteMember({ email: "new@example.com" });

  assert.deepEqual(registry.summarizeWorkspace(), {
    members: 1,
    invites: 1,
    generatedAt: "2026-06-01T00:00:00.000Z"
  });
});

test("rejects invalid members with structured errors", () => {
  const registry = createWorkspaceRegistry();

  assert.throws(
    () => registry.addMember({ email: "missing-id@example.com" }),
    (error) => error.code === "INVALID_MEMBER_ID" && error.status === 400
  );
  assert.throws(
    () => registry.setRole("missing", "admin"),
    (error) => error.code === "MEMBER_NOT_FOUND" && error.status === 400
  );
});
