const assert = require("node:assert/strict");

const { createWorkspaceRegistry } = require("./src/workspace");

const registry = createWorkspaceRegistry({ now: () => new Date("2026-06-01T12:00:00.000Z") });

registry.addMember({ id: "owner", email: "owner@example.com", role: "owner" });
registry.addMember({ id: "admin", email: "admin@example.com", role: "admin" });
registry.addMember({ id: "editor", email: "editor@example.com", role: "editor" });
registry.addMember({ id: "viewer", email: "viewer@example.com", role: "viewer" });

const activeInvite = registry.inviteMember({
  email: "active@example.com",
  role: "editor",
  expiresAt: "2026-06-02T12:00:00.000Z"
});
const expiredInvite = registry.inviteMember({
  email: "expired@example.com",
  role: "viewer",
  expiresAt: "2026-05-31T12:00:00.000Z"
});

assert.equal(registry.canMemberPerform("owner", "manage_members"), true);
assert.equal(registry.canMemberPerform("admin", "manage_members"), true);
assert.equal(registry.canMemberPerform("editor", "edit"), true);
assert.equal(registry.canMemberPerform("editor", "manage_members"), false);
assert.equal(registry.canMemberPerform("viewer", "view"), true);
assert.equal(registry.canMemberPerform("viewer", "edit"), false);

assert.deepEqual(registry.listActionableInvites().map((invite) => invite.id), [activeInvite.id]);
assert.equal(registry.listInvites().find((invite) => invite.id === activeInvite.id).expiresAt, "2026-06-02T12:00:00.000Z");
assert.equal(registry.listInvites().find((invite) => invite.id === expiredInvite.id).expiresAt, "2026-05-31T12:00:00.000Z");

assert.deepEqual(registry.summarizeAccess(), {
  members: 4,
  invites: 2,
  ownerCount: 1,
  adminCount: 1,
  pendingInviteCount: 1,
  expiredInviteCount: 1,
  reviewStatus: "needs_review",
  nextAction: "review_expired_invites",
  generatedAt: "2026-06-01T12:00:00.000Z"
});

assert.throws(
  () => registry.addMember({ id: "bad-role", email: "bad@example.com", role: "superadmin" }),
  (error) => error.code === "INVALID_ROLE" && error.status === 400
);
assert.throws(
  () => registry.canMemberPerform("owner", "delete_workspace"),
  (error) => error.code === "INVALID_ACTION" && error.status === 400
);
assert.throws(
  () => registry.inviteMember({ email: "bad-expiry@example.com", expiresAt: "tomorrow" }),
  (error) => error.code === "INVALID_INVITE_EXPIRY" && error.status === 400
);

console.log("workspace-permissions-continuation benchmark verification passed");
