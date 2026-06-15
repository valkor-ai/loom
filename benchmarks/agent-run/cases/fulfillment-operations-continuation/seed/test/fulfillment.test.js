const assert = require("node:assert/strict");
const test = require("node:test");

const { createFulfillmentQueue } = require("../src/fulfillment");

test("adds and lists orders", () => {
  const queue = createFulfillmentQueue({ now: () => new Date("2026-07-01T10:00:00.000Z") });

  const order = queue.addOrder({ id: "order-1", customer: "Acme" });

  assert.equal(order.id, "order-1");
  assert.equal(order.status, "open");
  assert.deepEqual(queue.listOrders({ status: "open" }).map((item) => item.id), ["order-1"]);
});

test("marks orders packed and shipped", () => {
  const queue = createFulfillmentQueue({ now: () => new Date("2026-07-01T10:00:00.000Z") });
  queue.addOrder({ id: "order-2", customer: "Globex" });

  assert.equal(queue.markPacked("order-2").status, "packed");
  assert.equal(queue.markShipped("order-2").status, "shipped");
});

test("summarizes fulfillment status counts", () => {
  const queue = createFulfillmentQueue({ now: () => new Date("2026-07-01T10:00:00.000Z") });
  queue.addOrder({ id: "order-1", customer: "Acme" });
  queue.addOrder({ id: "order-2", customer: "Globex" });
  queue.markPacked("order-2");

  assert.deepEqual(queue.summarizeFulfillment(), {
    total: 2,
    open: 1,
    packed: 1,
    shipped: 0,
    generatedAt: "2026-07-01T10:00:00.000Z"
  });
});

test("rejects invalid orders with structured errors", () => {
  const queue = createFulfillmentQueue();

  assert.throws(
    () => queue.addOrder({ customer: "Missing id" }),
    (error) => error.code === "INVALID_ORDER_ID" && error.status === 400
  );
  assert.throws(
    () => queue.markPacked("missing"),
    (error) => error.code === "ORDER_NOT_FOUND" && error.status === 400
  );
});
