# Rollout And Recovery

For each stage, define the signal to proceed, the signal to stop, and what recovery can restore. Example signals include invariant mismatches, backfill lag, error rates on old contract paths, or failures in a supported version pair.

Rehearse recovery after partial application and after the new version has written data. Rolling back the binary does not restore the previous data shape. Record which version can read those writes, what a restore would discard, and when forward repair becomes the supported option.

Keep destructive cleanup in a distinct step when it changes the available recovery path. Carry unresolved checks into the delivery handoff with an owner and the next observable check.
