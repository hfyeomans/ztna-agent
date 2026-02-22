#!/usr/bin/env bash
# H4: Pre-deploy validation — ensures TLS secrets exist before applying k8s manifests.
# Usage: ./validate-secrets.sh [namespace]

set -euo pipefail

NAMESPACE="${1:-ztna}"
REQUIRED_SECRETS=("ztna-intermediate-tls" "ztna-connector-tls")
ERRORS=0

echo "Validating TLS secrets in namespace '${NAMESPACE}'..."

for secret in "${REQUIRED_SECRETS[@]}"; do
    if kubectl get secret "$secret" -n "$NAMESPACE" >/dev/null 2>&1; then
        # Verify it has tls.crt and tls.key
        CRT=$(kubectl get secret "$secret" -n "$NAMESPACE" -o jsonpath='{.data.tls\.crt}' 2>/dev/null)
        KEY=$(kubectl get secret "$secret" -n "$NAMESPACE" -o jsonpath='{.data.tls\.key}' 2>/dev/null)
        if [ -z "$CRT" ] || [ -z "$KEY" ]; then
            echo "  FAIL: Secret '$secret' exists but is missing tls.crt or tls.key"
            ERRORS=$((ERRORS + 1))
        else
            echo "  OK:   Secret '$secret' exists with tls.crt and tls.key"
        fi
    else
        echo "  FAIL: Secret '$secret' not found in namespace '$NAMESPACE'"
        echo "        Create it with: kubectl create secret tls $secret -n $NAMESPACE --cert=<cert.pem> --key=<key.pem>"
        ERRORS=$((ERRORS + 1))
    fi
done

if [ "$ERRORS" -gt 0 ]; then
    echo ""
    echo "VALIDATION FAILED: $ERRORS secret(s) missing or incomplete."
    echo "Create the required TLS secrets before deploying."
    exit 1
fi

echo ""
echo "All TLS secrets validated successfully."
