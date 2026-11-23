# Evidence And Review

A confirmed source finding names the principal, resource, expected control, reachable code path, and concrete effect supported by the source. Do not infer execution, compromise, or production exposure. If reachability or configuration is unknown, mark it needs validation.

Re-read the call chain and look for a guard that disproves the finding. For high-impact findings, seek independent source review when available; otherwise say that it was not independently reviewed. Consolidate paths that share one underlying defect and remediation.

Rank findings by demonstrated consequence and prerequisites. Recommend the narrowest enforcing boundary and describe a benign regression assertion, such as a denied fixture operation leaving state unchanged. Do not include exploit instructions. A defensive suggestion without an established defect is a hardening note, not a confirmed finding.
