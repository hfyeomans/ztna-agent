#!/usr/bin/env bash
# setup-certbot.sh — Install certbot with Route53 DNS-01 plugin and issue initial certificate
#
# Prerequisites:
#   - AWS CLI configured with Route53 permissions (route53:ChangeResourceRecordSets, route53:GetChange)
#   - Domain DNS managed by Route53
#
# Usage:
#   ./deploy/aws/setup-certbot.sh <domain> [email]
#
# Example:
#   ./deploy/aws/setup-certbot.sh ztna.example.com admin@example.com

set -euo pipefail

DOMAIN="${1:?Usage: setup-certbot.sh <domain> [email]}"
EMAIL="${2:-}"
CERT_DIR="/etc/letsencrypt/live/${DOMAIN}"

echo "=== ZTNA Certificate Setup with Certbot + Route53 ==="
echo "  Domain: ${DOMAIN}"
echo "  Email:  ${EMAIL:-<not provided, will use --register-unsafely-without-email>}"
echo

# ---- Install certbot + Route53 plugin ----
echo "--- Installing certbot and dns-route53 plugin ---"
if ! command -v certbot &>/dev/null; then
    sudo apt-get update -qq
    sudo apt-get install -y -qq certbot python3-certbot-dns-route53
    echo "Certbot installed."
else
    echo "Certbot already installed: $(certbot --version 2>&1)"
    # Ensure plugin is present
    if ! certbot plugins 2>/dev/null | grep -q dns-route53; then
        sudo apt-get install -y -qq python3-certbot-dns-route53
    fi
fi
echo

# ---- Issue initial certificate ----
echo "--- Issuing certificate for ${DOMAIN} via DNS-01 ---"

CERTBOT_ARGS=(
    certonly
    --dns-route53
    -d "${DOMAIN}"
    --non-interactive
    --agree-tos
)

if [ -n "${EMAIL}" ]; then
    CERTBOT_ARGS+=(--email "${EMAIL}")
else
    CERTBOT_ARGS+=(--register-unsafely-without-email)
fi

sudo certbot "${CERTBOT_ARGS[@]}"

echo
echo "--- Certificate issued ---"
echo "  Cert:      ${CERT_DIR}/fullchain.pem"
echo "  Key:       ${CERT_DIR}/privkey.pem"
echo "  CA Chain:  ${CERT_DIR}/chain.pem"
echo

# ---- Symlink for intermediate-server ----
ZTNA_CERT_DIR="/home/ubuntu/ztna-agent/certs"
echo "--- Creating symlinks in ${ZTNA_CERT_DIR} ---"
mkdir -p "${ZTNA_CERT_DIR}"

# Create symlinks (or update if they exist)
ln -sf "${CERT_DIR}/fullchain.pem" "${ZTNA_CERT_DIR}/cert.pem"
ln -sf "${CERT_DIR}/privkey.pem" "${ZTNA_CERT_DIR}/key.pem"
ln -sf "${CERT_DIR}/chain.pem" "${ZTNA_CERT_DIR}/ca-cert.pem"

echo "  ${ZTNA_CERT_DIR}/cert.pem -> ${CERT_DIR}/fullchain.pem"
echo "  ${ZTNA_CERT_DIR}/key.pem  -> ${CERT_DIR}/privkey.pem"
echo "  ${ZTNA_CERT_DIR}/ca-cert.pem -> ${CERT_DIR}/chain.pem"
echo

echo "=== Done ==="
echo
echo "Next steps:"
echo "  1. Install systemd timer for auto-renewal:"
echo "     sudo cp deploy/aws/ztna-cert-renew.{service,timer} /etc/systemd/system/"
echo "     sudo systemctl daemon-reload"
echo "     sudo systemctl enable --now ztna-cert-renew.timer"
echo
echo "  2. Start intermediate-server with the certificates:"
echo "     intermediate-server --cert ${ZTNA_CERT_DIR}/cert.pem \\"
echo "       --key ${ZTNA_CERT_DIR}/key.pem \\"
echo "       --ca-cert ${ZTNA_CERT_DIR}/ca-cert.pem"
