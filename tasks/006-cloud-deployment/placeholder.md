# Placeholder Documentation: Cloud Deployment

**Task ID:** 006-cloud-deployment

---

## Purpose

Document intentional placeholder/scaffolding code that is part of planned features. Per project conventions, we use centralized `placeholder.md` files instead of in-code TODO comments.

---

## Active Placeholders

### ~~Hardcoded Server Addresses in macOS Agent~~ RESOLVED

**File:** `ios-macos/ZtnaAgent/Extension/PacketTunnelProvider.swift`
**Added:** 2026-01-26
**Resolved:** 2026-01-31
**Status:** RESOLVED

**Resolution (Task #2: Config File Mechanism):**
- `serverHost`, `serverPort`, `targetServiceId` are now mutable `var` properties with defaults
- `loadConfiguration()` reads from `NETunnelProviderProtocol.providerConfiguration` at tunnel start
- `parseIPv4()` derives `serverIPBytes` from `serverHost` (single source of truth)
- macOS app UI (`ContentView.swift`) exposes config fields persisted via `UserDefaults`
- Config passed to extension via `providerConfiguration` dictionary

---

### ~~Hardcoded Service ID~~ RESOLVED

**File:** `ios-macos/ZtnaAgent/Extension/PacketTunnelProvider.swift`
**Added:** 2026-01-26
**Resolved:** 2026-01-31
**Status:** RESOLVED

**Resolution:** Service ID now configurable via `providerConfiguration["serviceId"]`, with UI field in macOS app

---

### IPv4 Enforcement - Remove When IPv6 Supported

**File:** `ios-macos/ZtnaAgent/Extension/PacketTunnelProvider.swift`
**Line:** ~138-141 (in `setupUdpConnection()`)
**Added:** 2026-01-26
**Status:** PENDING REMOVAL
**Blocking Production:** No

**Current Implementation:**
```swift
// Force IPv4 to avoid IPv6 preference on dual-stack networks
if let ipOptions = params.defaultProtocolStack.internetProtocol as? NWProtocolIP.Options {
    ipOptions.version = .v4
}
```

**Why It Was Added:**
During AWS E2E debugging, we suspected IPv6 preference was causing connection failures. This code forces IPv4-only connections.

**Why It Should Be Removed:**
- Testing confirmed IPv6 was NOT the root cause (hardcoded IP was)
- IPv6 support is desirable for production
- Unnecessary restriction on network connectivity

**Target Implementation:**
Remove this code block entirely when ready to support IPv6 tunnel endpoints. Ensure:
1. Server infrastructure supports IPv6
2. `ipBytes` parsing handles both IPv4 and IPv6 addresses
3. Test with IPv6-only networks

**Action:**
Remove when implementing proper dual-stack support.

---

## Template

When adding placeholder code, document it here:

```markdown
### [Short Description]

**File:** `path/to/file`
**Line:** 123
**Added:** YYYY-MM-DD
**Status:** Pending / In Progress / Resolved

**Purpose:**
Why this placeholder exists and what it should eventually do.

**Current Implementation:**
What the code does now (stub, hardcoded value, etc.)

**Target Implementation:**
What it should do when complete.

**Blocked By:**
Any dependencies or decisions needed.
```

---

## Anticipated Placeholders

### Terraform Configuration

**Purpose:** Infrastructure as Code for cloud resources.

**Likely placeholder:**
- Manual provisioning initially
- IaC automation added later

### Ansible Deployment

**Purpose:** Automated component deployment and configuration.

**Likely placeholder:**
- Manual deployment initially
- Ansible playbooks added later

### Monitoring/Alerting

**Purpose:** Production observability for cloud components.

**Likely placeholder:**
- Basic logging initially
- Full monitoring stack later (Prometheus, Grafana)

### Certificate Automation

**Purpose:** Automated Let's Encrypt certificate renewal.

**Likely placeholder:**
- Manual certificate management initially
- Certbot cron job or systemd timer later

### Multi-Region Support

**Purpose:** Deploy to multiple geographic regions.

**Likely placeholder:**
- Single region initially
- Multi-region expansion later

---

## Resolved Placeholders

### Hardcoded Server Addresses (2026-01-31)
- `serverHost`, `serverPort`, `targetServiceId` in PacketTunnelProvider.swift
- Resolved by Task #2: Config File Mechanism
- Now loaded from `providerConfiguration` at tunnel start

### Hardcoded Service ID (2026-01-31)
- `targetServiceId` in PacketTunnelProvider.swift
- Resolved by Task #2: Config File Mechanism
- Now configurable via app UI and `providerConfiguration`
