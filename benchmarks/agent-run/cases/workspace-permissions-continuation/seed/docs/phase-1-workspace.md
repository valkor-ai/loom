# Phase 1 Workspace Registry Notes

The workspace registry currently supports a small in-memory workflow: add
members, update roles, remove members, create invitations, list members or
invites, and summarize basic counts.

Important boundaries:

- Keep the injectable `now()` clock for deterministic tests.
- Keep structured validation errors with `code` and `status`.
- Do not add auth, persistence, email delivery, or package dependencies.
- Existing callers rely on `addMember`, `setRole`, `removeMember`,
  `inviteMember`, `listMembers`, `listInvites`, and `summarizeWorkspace`.
