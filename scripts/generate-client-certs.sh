#!/usr/bin/env bash
# generate-client-certs.sh — Generate CA + client certificates for mTLS testing
#
# Creates a test CA and client certificates with ZTNA SAN entries:
#   agent.<service>.ztna     — authorizes Agent for <service>
#   connector.<service>.ztna — authorizes Connector for <service>
#   agent.*.ztna             — wildcard Agent (all services)
#
# Usage:
#   ./scripts/generate-client-certs.sh [output-dir] [service-name]
#
# Defaults:
#   output-dir:   certs/mtls
#   service-name: test-service

set -euo pipefail

OUTPUT_DIR="${1:-certs/mtls}"
SERVICE="${2:-test-service}"

mkdir -p "$OUTPUT_DIR"

echo "=== Generating mTLS test certificates ==="
echo "  Output:  $OUTPUT_DIR"
echo "  Service: $SERVICE"
echo

# ---- CA ----
echo "--- Generating CA ---"
openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256 \
    -out "$OUTPUT_DIR/ca-key.pem" 2>/dev/null

openssl req -new -x509 -key "$OUTPUT_DIR/ca-key.pem" \
    -out "$OUTPUT_DIR/ca-cert.pem" \
    -days 365 \
    -subj "/CN=ZTNA Test CA/O=ZTNA/OU=Testing" \
    -sha256

echo "  CA cert:  $OUTPUT_DIR/ca-cert.pem"
echo "  CA key:   $OUTPUT_DIR/ca-key.pem"
echo

# ---- Agent client cert (authorized for specific service) ----
echo "--- Generating Agent client cert (service: $SERVICE) ---"
openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256 \
    -out "$OUTPUT_DIR/agent-key.pem" 2>/dev/null

openssl req -new -key "$OUTPUT_DIR/agent-key.pem" \
    -out "$OUTPUT_DIR/agent.csr" \
    -subj "/CN=test-agent/O=ZTNA/OU=Agent"

cat > "$OUTPUT_DIR/agent-ext.cnf" <<EOF
[v3_ext]
subjectAltName = DNS:agent.${SERVICE}.ztna
keyUsage = digitalSignature
extendedKeyUsage = clientAuth
EOF

openssl x509 -req -in "$OUTPUT_DIR/agent.csr" \
    -CA "$OUTPUT_DIR/ca-cert.pem" -CAkey "$OUTPUT_DIR/ca-key.pem" \
    -CAcreateserial \
    -out "$OUTPUT_DIR/agent-cert.pem" \
    -days 365 -sha256 \
    -extfile "$OUTPUT_DIR/agent-ext.cnf" -extensions v3_ext

echo "  Agent cert: $OUTPUT_DIR/agent-cert.pem"
echo "  Agent key:  $OUTPUT_DIR/agent-key.pem"
echo "  SAN:        agent.${SERVICE}.ztna"
echo

# ---- Connector client cert (authorized for specific service) ----
echo "--- Generating Connector client cert (service: $SERVICE) ---"
openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256 \
    -out "$OUTPUT_DIR/connector-key.pem" 2>/dev/null

openssl req -new -key "$OUTPUT_DIR/connector-key.pem" \
    -out "$OUTPUT_DIR/connector.csr" \
    -subj "/CN=test-connector/O=ZTNA/OU=Connector"

cat > "$OUTPUT_DIR/connector-ext.cnf" <<EOF
[v3_ext]
subjectAltName = DNS:connector.${SERVICE}.ztna
keyUsage = digitalSignature
extendedKeyUsage = clientAuth
EOF

openssl x509 -req -in "$OUTPUT_DIR/connector.csr" \
    -CA "$OUTPUT_DIR/ca-cert.pem" -CAkey "$OUTPUT_DIR/ca-key.pem" \
    -CAcreateserial \
    -out "$OUTPUT_DIR/connector-cert.pem" \
    -days 365 -sha256 \
    -extfile "$OUTPUT_DIR/connector-ext.cnf" -extensions v3_ext

echo "  Connector cert: $OUTPUT_DIR/connector-cert.pem"
echo "  Connector key:  $OUTPUT_DIR/connector-key.pem"
echo "  SAN:            connector.${SERVICE}.ztna"
echo

# ---- Wildcard Agent cert (authorized for ALL services) ----
echo "--- Generating Wildcard Agent cert (all services) ---"
openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256 \
    -out "$OUTPUT_DIR/agent-wildcard-key.pem" 2>/dev/null

openssl req -new -key "$OUTPUT_DIR/agent-wildcard-key.pem" \
    -out "$OUTPUT_DIR/agent-wildcard.csr" \
    -subj "/CN=wildcard-agent/O=ZTNA/OU=Agent"

cat > "$OUTPUT_DIR/agent-wildcard-ext.cnf" <<EOF
[v3_ext]
subjectAltName = DNS:agent.*.ztna
keyUsage = digitalSignature
extendedKeyUsage = clientAuth
EOF

openssl x509 -req -in "$OUTPUT_DIR/agent-wildcard.csr" \
    -CA "$OUTPUT_DIR/ca-cert.pem" -CAkey "$OUTPUT_DIR/ca-key.pem" \
    -CAcreateserial \
    -out "$OUTPUT_DIR/agent-wildcard-cert.pem" \
    -days 365 -sha256 \
    -extfile "$OUTPUT_DIR/agent-wildcard-ext.cnf" -extensions v3_ext

echo "  Wildcard cert: $OUTPUT_DIR/agent-wildcard-cert.pem"
echo "  Wildcard key:  $OUTPUT_DIR/agent-wildcard-key.pem"
echo "  SAN:           agent.*.ztna"
echo

# ---- Unauthorized cert (no ZTNA SANs — backward compat) ----
echo "--- Generating Legacy cert (no ZTNA SANs) ---"
openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256 \
    -out "$OUTPUT_DIR/legacy-key.pem" 2>/dev/null

openssl req -new -key "$OUTPUT_DIR/legacy-key.pem" \
    -out "$OUTPUT_DIR/legacy.csr" \
    -subj "/CN=legacy-client/O=ZTNA/OU=Legacy"

cat > "$OUTPUT_DIR/legacy-ext.cnf" <<EOF
[v3_ext]
keyUsage = digitalSignature
extendedKeyUsage = clientAuth
EOF

openssl x509 -req -in "$OUTPUT_DIR/legacy.csr" \
    -CA "$OUTPUT_DIR/ca-cert.pem" -CAkey "$OUTPUT_DIR/ca-key.pem" \
    -CAcreateserial \
    -out "$OUTPUT_DIR/legacy-cert.pem" \
    -days 365 -sha256 \
    -extfile "$OUTPUT_DIR/legacy-ext.cnf" -extensions v3_ext

echo "  Legacy cert: $OUTPUT_DIR/legacy-cert.pem"
echo "  Legacy key:  $OUTPUT_DIR/legacy-key.pem"
echo "  SAN:         (none — backward compat, allows all)"
echo

# ---- Cleanup CSR and temp files ----
rm -f "$OUTPUT_DIR"/*.csr "$OUTPUT_DIR"/*.cnf "$OUTPUT_DIR"/*.srl

echo "=== Done ==="
echo
echo "Files generated:"
ls -la "$OUTPUT_DIR"/*.pem
echo
echo "Usage examples:"
echo "  # Server with mTLS enabled:"
echo "  intermediate-server --require-client-cert --ca-cert $OUTPUT_DIR/ca-cert.pem"
echo
echo "  # Agent client with mTLS:"
echo "  quic-test-client --client-cert $OUTPUT_DIR/agent-cert.pem \\"
echo "    --client-key $OUTPUT_DIR/agent-key.pem \\"
echo "    --ca-cert $OUTPUT_DIR/ca-cert.pem"
