# Domain Discipline

Use this reference during Brainstorm, candidate generation, or task execution when domain terms, concepts, or user/code disagreements affect delivery.

## Resolve language early

- Treat overloaded terms as delivery risk. Ask or surface the ambiguity at the next user gate instead of guessing.
- Prefer the user's business language in summaries, candidates, task results, and test names.
- When the request provides `taskConceptGrounding`, satisfy each concept responsibility and record concrete `conceptEvidence`.

## Check against the project

- Read existing project docs, glossary-style files, issue context, or relevant code when Loom's read plan points to them.
- If code behavior disagrees with the user's description, state the conflict and preserve it in the candidate or result evidence.
- Do not rename domain concepts casually; preserve established names unless the task explicitly includes a naming change.

## Capture decisions

- Capture durable decisions in the candidate/result evidence or existing project docs when the task permits.
- Create new decision docs only when the repo convention or current request calls for them.
- Keep implementation details out of glossary-style notes; keep trade-offs and irreversible choices in decision-style notes.
