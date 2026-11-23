# Release Integrity

- Identify who can change release inputs and who can publish. Separate pull-request checks from trusted publishing jobs.
- Compare artifact build and publish steps to verify that the published bytes come from the reviewed revision. Check whether checksums, signatures, or attestations are verified by the consumer rather than simply produced.
- Review workflow token scopes, protected environments, approvals, dependency pinning, and installer integrity checks. Distinguish repository settings that were observed from settings assumed by the workflow.
- Do not run publishing jobs or inspect credential values. Record missing deployment facts as unresolved.
