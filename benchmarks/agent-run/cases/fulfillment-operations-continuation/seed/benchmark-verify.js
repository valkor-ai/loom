const assert = require("node:assert/strict");

const { createFulfillmentQueue } = require("./src/fulfillment");

const queue = createFulfillmentQueue({
  now: () => new Date("2026-07-01T10:00:00.000Z"),
  inventory: {
    "starter-kit": 3,
    "pro-kit": 0
  }
});

const first = queue.addOrder({
  id: "order-1",
  customer: "Acme",
  lineItems: [{ sku: "starter-kit", quantity: 2 }]
});
const second = queue.addOrder({
  id: "order-2",
  customer: "Globex",
  lineItems: [{ sku: "starter-kit", quantity: 2 }]
});
const third = queue.addOrder({
  id: "order-3",
  customer: "Initech",
  lineItems: [{ sku: "pro-kit", quantity: 1 }]
});
queue.addOrder({ id: "order-4", customer: "Umbrella" });

assert.deepEqual(first.lineItems, [{ sku: "starter-kit", quantity: 2 }]);
first.lineItems[0].quantity = 99;
assert.equal(
  queue.listOrders().find((order) => order.id === "order-1").lineItems[0].quantity,
  2,
  "returned order objects must be deeply cloned"
);

queue.markPacked("order-1");
queue.markPacked("order-2");
queue.markPacked("order-3");
assert.deepEqual(queue.reserveReadyOrders().map((order) => order.id), ["order-1"]);

const orders = queue.listOrders();
assert.equal(orders.find((order) => order.id === "order-1").reservation.status, "reserved");
assert.deepEqual(orders.find((order) => order.id === "order-2").shortages, [
  { sku: "starter-kit", required: 2, available: 1 }
]);
assert.deepEqual(orders.find((order) => order.id === "order-3").shortages, [
  { sku: "pro-kit", required: 1, available: 0 }
]);

assert.deepEqual(queue.summarizeFulfillment(), {
  total: 4,
  open: 1,
  packed: 3,
  shipped: 0,
  readyCount: 1,
  reservedCount: 1,
  backorderedCount: 2,
  shortageCount: 2,
  fulfillmentStatus: "blocked",
  nextAction: "replenish_inventory",
  generatedAt: "2026-07-01T10:00:00.000Z"
});

assert.throws(
  () => queue.addOrder({ id: "bad-line", customer: "Bad", lineItems: [{ sku: "", quantity: 1 }] }),
  (error) => error.code === "INVALID_LINE_ITEM" && error.status === 400
);
assert.throws(
  () => queue.addOrder({ id: "bad-qty", customer: "Bad", lineItems: [{ sku: "starter-kit", quantity: 0 }] }),
  (error) => error.code === "INVALID_LINE_ITEM" && error.status === 400
);

console.log("fulfillment-operations-continuation benchmark verification passed");
