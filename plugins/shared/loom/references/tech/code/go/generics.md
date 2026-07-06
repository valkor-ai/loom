# Go Generics Quality

Use this topic reference when `tech/code/go/generics.md` is listed in `sourceContext.codeQualityRequirements[].referenceLoadPlan`.

Read it together with `tech/code/common.md`. Do not read sibling topic files unless they are also listed in the same load plan.

## When To Use

- The task changes Go type parameters, constraints, generic collections, reusable algorithms, typed adapters, or package APIs that support multiple concrete types.
- Use generics only when they reduce real duplication or preserve type safety across multiple concrete types.
- If one concrete type or one behavior-specific interface is enough, prefer ordinary Go code.

## Implementation Focus

- Start with call-site readability. A generic function should make callers clearer, not force explicit type arguments or complicated constraints for ordinary use.
- Keep constraints small and named by capability, such as `Ordered`, `ID`, `Codec`, or `Key`. Avoid broad constraints that effectively mean `any` plus reflection.
- Use `comparable` only for map keys, equality, de-duplication, or set membership. Do not use it as a generic "business object" constraint.
- Use approximate constraints with `~` only when named aliases should be accepted and operations are valid for the underlying type.
- Do not replace small interfaces with generics when behavior matters. If a dependency is defined by methods, a focused interface is usually clearer than a type parameter.
- Avoid generic domain repositories or CRUD wrappers that erase business operation names. Domain services should still expose meaningful methods and invariants.
- Keep zero-value behavior explicit for generic containers. Returning `(T, bool)` is usually better than returning only `T` when absence is possible.
- Avoid generic helpers that require runtime type switches, reflection, or unsafe code. If runtime dispatch is needed, a normal interface or explicit functions may be better.
- Keep exported generic APIs stable and simple; internal helpers can be narrower but should not leak unreadable constraints into package consumers.

## Verification Focus

- Run `go test ./...` and compile all affected packages.
- Test generic helpers with at least two meaningful concrete types when the abstraction claims type independence.
- Add absence/zero-value tests for generic containers or lookup helpers.
- Confirm the implementation did not introduce reflection, unsafe code, or type assertions as a shortcut around the generic contract.

## Evidence Notes

- Record `go.generics` in `codeQualityEvidence.referenceGroupsChecked`.
- Record `tech/code/go/generics.md` in `codeQualityEvidence.referenceFilesChecked` when this file influenced the implementation.
- In the evidence summary, name the generic decision: constraint design, reusable algorithm, typed collection, zero-value contract, or interface-vs-generic choice.
