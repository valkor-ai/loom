# Access Control

- Follow identity from the authenticated request or job context to the server-side policy decision. Confirm that callers cannot select their own role or tenant by setting ordinary business fields.
- Compare authorization across read, write, export, background job, and bulk paths for the same resource. Check that the resource lookup and policy use the same tenant and ownership attributes.
- Review session expiry, revocation, and role changes against caches and long-lived jobs. Record where consistency depends on configuration or an external identity provider.
- Check whether side effects occur before authorization, and whether denied requests leave durable state or publish events. Use existing tests as evidence only if the outcome is observable at the relevant boundary.
