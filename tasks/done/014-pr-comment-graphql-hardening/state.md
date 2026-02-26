# State

## Status
COMPLETE. Merged to master via PR #8. Live-validated against 16 PR review threads.

## Files changed
- `scripts/resolve-pr-comments.sh`
- `tasks/014-pr-comment-graphql-hardening/research.md`
- `tasks/014-pr-comment-graphql-hardening/plan.md`
- `tasks/014-pr-comment-graphql-hardening/todo.md`
- `tasks/014-pr-comment-graphql-hardening/state.md`

## Verification
- `bash -n scripts/resolve-pr-comments.sh` passed
- `shellcheck scripts/resolve-pr-comments.sh` passed

## Behavior changes
- `reply` and `resolve` now target unresolved-thread indices (the same indices shown by `list`).
- Added `smoke-test` command for non-mutating integration checks.

## Notes
- Live API verification (against a real PR) was not executed in this environment.
