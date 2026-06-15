# Operations Notes

The operations team wants to see which packed orders can actually ship before
carrier pickup. They discussed future barcode scans, warehouse APIs, carrier
labels, and split shipments.

This phase is narrower: keep the local queue, allocate from an injectable stock
map, and expose deterministic readiness and shortage fields for the pickup
checkpoint.
