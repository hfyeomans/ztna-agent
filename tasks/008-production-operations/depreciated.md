# Depreciated: Production Operations

**Task ID:** 008-production-operations
**Status:** Complete
**Priority:** P2
**Depends On:** 007-security-hardening
**Branch:** feature/008-production-operations
**Last Updated:** 2026-02-27

---

## Purpose

Track depreciated or legacy code that is identified and removed during production operations work. Per project conventions, removed code is documented here instead of being marked with inline `// Depreciated` or `// Legacy` comments.

---

## Depreciated Code

| File | Lines | What Was Removed | Why | Date |
|------|-------|-----------------|-----|------|
| `app-connector/src/main.rs` | run() loop | Hard `break` on intermediate connection close | Replaced with reconnection loop + exponential backoff | 2026-02-27 |
| `app-connector/src/main.rs` | process_local_socket() | Per-poll `vec![0u8; 65535]` allocation | Replaced with `self.recv_buf` reuse + `to_vec()` copy (Oracle Finding 14) | 2026-02-27 |
| `deploy/ansible/roles/ztna/templates/echo-server.service.j2` | ExecStart | Inline Python one-liner `ExecStart=/usr/bin/python3 -c "...code..."` | Invalid Python syntax (def after semicolons) — replaced with separate echo-server.py.j2 script (Oracle review P1) | 2026-02-27 |
