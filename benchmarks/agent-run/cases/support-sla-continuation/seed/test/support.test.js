const assert = require("node:assert/strict");
const test = require("node:test");

const { createSupportQueue } = require("../src/support");

test("adds and lists open support tickets", () => {
  const queue = createSupportQueue({ now: () => new Date("2026-03-10T12:00:00.000Z") });

  const ticket = queue.addTicket({ title: "Cannot export invoices", customer: "Acme" });

  assert.equal(ticket.title, "Cannot export invoices");
  assert.equal(ticket.customer, "Acme");
  assert.equal(ticket.status, "open");
  assert.deepEqual(queue.listTickets({ status: "open" }).map((item) => item.id), [ticket.id]);
});

test("assigns and resolves tickets", () => {
  const queue = createSupportQueue({ now: () => new Date("2026-03-10T12:00:00.000Z") });
  const ticket = queue.addTicket({ title: "Webhook retries failing" });

  assert.equal(queue.assignTicket(ticket.id, "support-lead").assignedTo, "support-lead");
  assert.equal(queue.resolveTicket(ticket.id).status, "resolved");
  assert.deepEqual(queue.listTickets({ status: "resolved" }).map((item) => item.id), [ticket.id]);
});

test("summarizes support queue progress", () => {
  const queue = createSupportQueue({ now: () => new Date("2026-03-10T12:00:00.000Z") });
  const first = queue.addTicket({ title: "Cannot export invoices" });
  queue.addTicket({ title: "Billing page timeout" });

  queue.resolveTicket(first.id);

  assert.deepEqual(queue.summarizeQueue(), {
    total: 2,
    open: 1,
    resolved: 1,
    generatedAt: "2026-03-10T12:00:00.000Z"
  });
});

test("rejects invalid tickets and assignments with structured errors", () => {
  const queue = createSupportQueue();

  assert.throws(
    () => queue.addTicket({ title: "   " }),
    (error) => error.code === "INVALID_TITLE" && error.status === 400
  );
  assert.throws(
    () => queue.assignTicket("missing", "support-lead"),
    (error) => error.code === "TICKET_NOT_FOUND" && error.status === 400
  );
  assert.throws(
    () => queue.assignTicket("1", "   "),
    (error) => error.code === "INVALID_ASSIGNEE" && error.status === 400
  );
});
