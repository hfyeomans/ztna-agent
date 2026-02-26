//! mTLS client authentication and authorization
//!
//! Extracts client identity from DER-encoded X.509 certificates presented
//! during the QUIC/TLS handshake. Uses SAN (Subject Alternative Name) DNS
//! entries to determine service-level authorization.
//!
//! SAN convention:
//!   DNS:agent.<service>.ztna   → authorized as Agent for <service>
//!   DNS:connector.<service>.ztna → authorized as Connector for <service>
//!   DNS:agent.*.ztna           → wildcard Agent (all services)
//!   DNS:connector.*.ztna       → wildcard Connector (all services)
//!   (no ZTNA SAN entries)      → allow all (backward compatibility)

use std::collections::HashSet;
use std::fmt;

use x509_parser::prelude::*;

use crate::client::ClientType;

// ============================================================================
// Types
// ============================================================================

/// Identity extracted from a client's X.509 certificate
#[derive(Debug, Clone)]
pub struct ClientIdentity {
    /// Common Name from the certificate subject
    pub common_name: String,
    /// Services this client is authorized for (from SAN DNS entries).
    /// None means no ZTNA SAN entries were found → allow all (backward compat).
    pub authorized_services: Option<HashSet<String>>,
}

/// Errors during certificate parsing and identity extraction
#[derive(Debug)]
pub enum AuthError {
    /// Failed to parse X.509 DER certificate
    ParseError(String),
    /// Certificate has no Common Name in subject
    MissingCommonName,
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthError::ParseError(msg) => write!(f, "certificate parse error: {}", msg),
            AuthError::MissingCommonName => write!(f, "certificate has no Common Name"),
        }
    }
}

impl std::error::Error for AuthError {}

/// ZTNA SAN domain suffix
const ZTNA_SAN_SUFFIX: &str = ".ztna";

// ============================================================================
// Identity Extraction
// ============================================================================

/// Extract client identity from a DER-encoded X.509 certificate.
///
/// Parses the certificate to extract:
/// - Common Name (CN) from the subject
/// - Service authorization from SAN DNS entries matching `*.ztna` pattern
pub fn extract_identity(der_cert: &[u8]) -> Result<ClientIdentity, AuthError> {
    let (_, cert) =
        X509Certificate::from_der(der_cert).map_err(|e| AuthError::ParseError(format!("{}", e)))?;

    // Extract Common Name from subject
    let common_name = cert
        .subject()
        .iter_common_name()
        .next()
        .and_then(|cn| cn.as_str().ok())
        .map(|s| s.to_string())
        .ok_or(AuthError::MissingCommonName)?;

    // Extract authorized services from SAN DNS entries
    let authorized_services = extract_authorized_services(&cert);

    Ok(ClientIdentity {
        common_name,
        authorized_services,
    })
}

/// Extract authorized services from SAN DNS entries.
///
/// Looks for DNS entries matching:
///   agent.<service>.ztna     → adds "agent:<service>"
///   connector.<service>.ztna → adds "connector:<service>"
///   agent.*.ztna             → adds "agent:*"
///   connector.*.ztna         → adds "connector:*"
///
/// Returns None if no ZTNA SAN entries found (backward compat = allow all).
fn extract_authorized_services(cert: &X509Certificate<'_>) -> Option<HashSet<String>> {
    let san_ext = cert
        .extensions()
        .iter()
        .find(|ext| ext.oid == oid_registry::OID_X509_EXT_SUBJECT_ALT_NAME);

    let san_ext = san_ext?;

    let san = match san_ext.parsed_extension() {
        ParsedExtension::SubjectAlternativeName(san) => san,
        _ => return None,
    };

    let mut services = HashSet::new();

    for name in &san.general_names {
        if let GeneralName::DNSName(dns) = name {
            if let Some(entry) = parse_ztna_san(dns) {
                services.insert(entry);
            }
        }
    }

    if services.is_empty() {
        None // No ZTNA SAN entries → backward compat (allow all)
    } else {
        Some(services)
    }
}

/// Parse a single SAN DNS entry for ZTNA service authorization.
///
/// Expected format: `<role>.<service>.ztna`
/// Returns `"<role>:<service>"` string for matching, or None if not a ZTNA entry.
fn parse_ztna_san(dns: &str) -> Option<String> {
    let dns = dns.to_lowercase();
    if !dns.ends_with(ZTNA_SAN_SUFFIX) {
        return None;
    }

    // Strip ".ztna" suffix
    let prefix = &dns[..dns.len() - ZTNA_SAN_SUFFIX.len()];

    // Split into role.service
    let dot_pos = prefix.find('.')?;
    let role = &prefix[..dot_pos];
    let service = &prefix[dot_pos + 1..];

    if service.is_empty() {
        return None;
    }

    match role {
        "agent" | "connector" => Some(format!("{}:{}", role, service)),
        _ => None,
    }
}

// ============================================================================
// Authorization Check
// ============================================================================

/// Check whether a client identity is authorized for a specific service.
///
/// Authorization rules:
/// 1. If `identity.authorized_services` is None → allow all (backward compat)
/// 2. Check for exact match: `<role>:<service_id>`
/// 3. Check for wildcard: `<role>:*`
/// 4. Otherwise → deny
pub fn is_authorized_for_service(
    identity: &ClientIdentity,
    service_id: &str,
    client_type: &ClientType,
) -> bool {
    let services = match &identity.authorized_services {
        None => return true, // No ZTNA SANs = allow all (backward compat)
        Some(s) => s,
    };

    let role = match client_type {
        ClientType::Agent => "agent",
        ClientType::Connector => "connector",
    };

    // Check exact match
    let exact_key = format!("{}:{}", role, service_id);
    if services.contains(&exact_key) {
        return true;
    }

    // Check wildcard
    let wildcard_key = format!("{}:*", role);
    services.contains(&wildcard_key)
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Helper: build a self-signed X.509 cert with given CN and SAN DNS entries
    fn build_test_cert(cn: &str, san_dns: &[&str]) -> Vec<u8> {
        use rcgen::{CertificateParams, DnType, KeyPair, SanType};

        let mut params = CertificateParams::default();
        params.distinguished_name.push(DnType::CommonName, cn);

        for dns in san_dns {
            params
                .subject_alt_names
                .push(SanType::DnsName(dns.to_string().try_into().unwrap()));
        }

        let key_pair = KeyPair::generate().unwrap();
        let cert = params.self_signed(&key_pair).unwrap();
        cert.der().to_vec()
    }

    // Helper: build a cert with no CN (use org name only)
    fn build_cert_no_cn(san_dns: &[&str]) -> Vec<u8> {
        use rcgen::{CertificateParams, DistinguishedName, DnType, KeyPair, SanType};

        let mut params = CertificateParams::default();
        // Replace the default distinguished name (which may have a default CN)
        // with one that only has Organization
        let mut dn = DistinguishedName::new();
        dn.push(DnType::OrganizationName, "TestOrg");
        params.distinguished_name = dn;

        for dns in san_dns {
            params
                .subject_alt_names
                .push(SanType::DnsName(dns.to_string().try_into().unwrap()));
        }

        let key_pair = KeyPair::generate().unwrap();
        let cert = params.self_signed(&key_pair).unwrap();
        cert.der().to_vec()
    }

    // ---- extract_identity tests ----

    #[test]
    fn test_extract_identity_with_agent_san() {
        let der = build_test_cert("test-agent", &["agent.myservice.ztna"]);
        let identity = extract_identity(&der).unwrap();

        assert_eq!(identity.common_name, "test-agent");
        let services = identity.authorized_services.as_ref().unwrap();
        assert!(services.contains("agent:myservice"));
        assert_eq!(services.len(), 1);
    }

    #[test]
    fn test_extract_identity_with_connector_san() {
        let der = build_test_cert("test-connector", &["connector.web-app.ztna"]);
        let identity = extract_identity(&der).unwrap();

        assert_eq!(identity.common_name, "test-connector");
        let services = identity.authorized_services.as_ref().unwrap();
        assert!(services.contains("connector:web-app"));
    }

    #[test]
    fn test_extract_identity_with_multiple_sans() {
        let der = build_test_cert(
            "multi-service-agent",
            &[
                "agent.svc-a.ztna",
                "agent.svc-b.ztna",
                "connector.svc-c.ztna",
            ],
        );
        let identity = extract_identity(&der).unwrap();

        let services = identity.authorized_services.as_ref().unwrap();
        assert!(services.contains("agent:svc-a"));
        assert!(services.contains("agent:svc-b"));
        assert!(services.contains("connector:svc-c"));
        assert_eq!(services.len(), 3);
    }

    #[test]
    fn test_extract_identity_with_wildcard() {
        let der = build_test_cert("wildcard-agent", &["agent.*.ztna"]);
        let identity = extract_identity(&der).unwrap();

        let services = identity.authorized_services.as_ref().unwrap();
        assert!(services.contains("agent:*"));
    }

    #[test]
    fn test_extract_identity_no_ztna_san_backward_compat() {
        // Certificate with non-ZTNA SAN entries → allow all
        let der = build_test_cert("legacy-client", &["www.example.com"]);
        let identity = extract_identity(&der).unwrap();

        assert_eq!(identity.common_name, "legacy-client");
        assert!(identity.authorized_services.is_none());
    }

    #[test]
    fn test_extract_identity_no_san_at_all() {
        // rcgen always adds SANs from subject_alt_names, but with empty list
        // the SAN extension won't be present if no entries
        let der = build_test_cert("no-san-client", &[]);
        let identity = extract_identity(&der).unwrap();

        assert_eq!(identity.common_name, "no-san-client");
        assert!(identity.authorized_services.is_none());
    }

    #[test]
    fn test_extract_identity_missing_cn() {
        let der = build_cert_no_cn(&["agent.svc.ztna"]);
        let result = extract_identity(&der);
        assert!(matches!(result, Err(AuthError::MissingCommonName)));
    }

    #[test]
    fn test_extract_identity_invalid_der() {
        let result = extract_identity(b"not a certificate");
        assert!(matches!(result, Err(AuthError::ParseError(_))));
    }

    // ---- parse_ztna_san tests ----

    #[test]
    fn test_parse_ztna_san_agent() {
        assert_eq!(
            parse_ztna_san("agent.myservice.ztna"),
            Some("agent:myservice".to_string())
        );
    }

    #[test]
    fn test_parse_ztna_san_connector() {
        assert_eq!(
            parse_ztna_san("connector.web-app.ztna"),
            Some("connector:web-app".to_string())
        );
    }

    #[test]
    fn test_parse_ztna_san_wildcard() {
        assert_eq!(parse_ztna_san("agent.*.ztna"), Some("agent:*".to_string()));
    }

    #[test]
    fn test_parse_ztna_san_case_insensitive() {
        assert_eq!(
            parse_ztna_san("Agent.MyService.ZTNA"),
            Some("agent:myservice".to_string())
        );
    }

    #[test]
    fn test_parse_ztna_san_non_ztna() {
        assert_eq!(parse_ztna_san("www.example.com"), None);
    }

    #[test]
    fn test_parse_ztna_san_unknown_role() {
        assert_eq!(parse_ztna_san("admin.myservice.ztna"), None);
    }

    #[test]
    fn test_parse_ztna_san_empty_service() {
        assert_eq!(parse_ztna_san("agent..ztna"), None);
    }

    #[test]
    fn test_parse_ztna_san_no_role() {
        // "myservice.ztna" has no dot before service
        assert_eq!(parse_ztna_san("myservice.ztna"), None);
    }

    // ---- is_authorized_for_service tests ----

    #[test]
    fn test_authorized_exact_match() {
        let identity = ClientIdentity {
            common_name: "test".to_string(),
            authorized_services: Some(["agent:myservice".to_string()].into_iter().collect()),
        };

        assert!(is_authorized_for_service(
            &identity,
            "myservice",
            &ClientType::Agent
        ));
        assert!(!is_authorized_for_service(
            &identity,
            "other",
            &ClientType::Agent
        ));
        assert!(!is_authorized_for_service(
            &identity,
            "myservice",
            &ClientType::Connector
        ));
    }

    #[test]
    fn test_authorized_wildcard() {
        let identity = ClientIdentity {
            common_name: "test".to_string(),
            authorized_services: Some(["agent:*".to_string()].into_iter().collect()),
        };

        assert!(is_authorized_for_service(
            &identity,
            "any-service",
            &ClientType::Agent
        ));
        assert!(is_authorized_for_service(
            &identity,
            "another",
            &ClientType::Agent
        ));
        assert!(!is_authorized_for_service(
            &identity,
            "any-service",
            &ClientType::Connector
        ));
    }

    #[test]
    fn test_authorized_backward_compat_no_san() {
        let identity = ClientIdentity {
            common_name: "legacy".to_string(),
            authorized_services: None,
        };

        // No ZTNA SAN = allow all
        assert!(is_authorized_for_service(
            &identity,
            "any",
            &ClientType::Agent
        ));
        assert!(is_authorized_for_service(
            &identity,
            "any",
            &ClientType::Connector
        ));
    }

    #[test]
    fn test_authorized_connector_exact() {
        let identity = ClientIdentity {
            common_name: "test-connector".to_string(),
            authorized_services: Some(["connector:web-app".to_string()].into_iter().collect()),
        };

        assert!(is_authorized_for_service(
            &identity,
            "web-app",
            &ClientType::Connector
        ));
        assert!(!is_authorized_for_service(
            &identity,
            "web-app",
            &ClientType::Agent
        ));
        assert!(!is_authorized_for_service(
            &identity,
            "other",
            &ClientType::Connector
        ));
    }

    #[test]
    fn test_authorized_multiple_services() {
        let identity = ClientIdentity {
            common_name: "multi".to_string(),
            authorized_services: Some(
                ["agent:svc-a".to_string(), "agent:svc-b".to_string()]
                    .into_iter()
                    .collect(),
            ),
        };

        assert!(is_authorized_for_service(
            &identity,
            "svc-a",
            &ClientType::Agent
        ));
        assert!(is_authorized_for_service(
            &identity,
            "svc-b",
            &ClientType::Agent
        ));
        assert!(!is_authorized_for_service(
            &identity,
            "svc-c",
            &ClientType::Agent
        ));
    }

    #[test]
    fn test_authorized_empty_services_set_denies_all() {
        // Empty set (different from None) → deny all
        let identity = ClientIdentity {
            common_name: "empty".to_string(),
            authorized_services: Some(HashSet::new()),
        };

        assert!(!is_authorized_for_service(
            &identity,
            "any",
            &ClientType::Agent
        ));
        assert!(!is_authorized_for_service(
            &identity,
            "any",
            &ClientType::Connector
        ));
    }
}
