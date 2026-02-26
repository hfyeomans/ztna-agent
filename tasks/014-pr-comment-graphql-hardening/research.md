# Research: resolve-pr-comments GraphQL reliability

## Request
Investigate intermittent GraphQL failures in `scripts/resolve-pr-comments.sh` and improve reliability.

## Existing behavior before hardening
- GraphQL operations were built via inline interpolation.
- `reviewThreads(first: 100)` had no pagination.
- No retry/backoff existed for GraphQL or REST reply calls.
- `list` showed sparse indices (from full list) when filtering unresolved threads.
- No dry-run command existed to validate API/query wiring without mutating state.

## Failure modes addressed
1. Transient GitHub/API network errors (timeouts, 502/503/504, rate limits).
2. GraphQL payload-level `errors` returned with HTTP success.
3. Incomplete thread visibility on large PRs due to missing pagination.
4. Operational mistakes from non-contiguous unresolved indices.
5. No low-risk smoke path to verify query/auth/schema behavior.
