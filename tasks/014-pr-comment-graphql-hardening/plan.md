# Plan: GraphQL hardening for PR comment resolver

1. Add reusable API retry/backoff helpers for GraphQL and REST calls.
2. Convert GraphQL operations to variableized query/mutation forms.
3. Implement `reviewThreads` pagination.
4. Align unresolved-thread list indices with reply/resolve index mapping.
5. Add `smoke-test` command that validates fetch + resolve path prerequisites without performing mutations.
6. Validate with `bash -n` and `shellcheck`.
