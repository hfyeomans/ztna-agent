#!/bin/bash
# =============================================================================
# Check Pi k8s Cluster Prerequisites for ZTNA Deployment
# =============================================================================
# Run this script on a machine with kubectl access to verify the cluster
# is ready for ZTNA deployment.
#
# Usage: ./check-cluster.sh
# =============================================================================

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[✓]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[!]${NC} $1"; }
log_error() { echo -e "${RED}[✗]${NC} $1"; }
log_step() { echo -e "\n${BLUE}==> $1${NC}"; }

echo ""
echo -e "${BLUE}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║${NC}          ZTNA Pi k8s Cluster Check                            ${BLUE}║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""

# Track issues
ISSUES=()

# ============================================================================
log_step "Checking kubectl access..."
# ============================================================================
if kubectl cluster-info &> /dev/null; then
    log_success "kubectl can access cluster"
    kubectl cluster-info | head -1
else
    log_error "Cannot access cluster with kubectl"
    exit 1
fi

# ============================================================================
log_step "Checking Kubernetes version..."
# ============================================================================
K8S_VERSION=$(kubectl version --short 2>/dev/null | grep Server | awk '{print $3}' || kubectl version -o json | jq -r '.serverVersion.gitVersion')
log_info "Kubernetes version: $K8S_VERSION"

# ============================================================================
log_step "Checking node architecture..."
# ============================================================================
echo ""
kubectl get nodes -o custom-columns='NAME:.metadata.name,ARCH:.status.nodeInfo.architecture,OS:.status.nodeInfo.osImage' 2>/dev/null || \
kubectl get nodes -o wide

NODE_ARCHS=$(kubectl get nodes -o jsonpath='{.items[*].status.nodeInfo.architecture}' | tr ' ' '\n' | sort -u)
if echo "$NODE_ARCHS" | grep -q "arm64"; then
    log_success "Found arm64 nodes"
else
    log_warn "No arm64 nodes found - images may need amd64 build"
fi

# ============================================================================
log_step "Checking Cilium installation..."
# ============================================================================
if kubectl get pods -n kube-system -l k8s-app=cilium 2>/dev/null | grep -q Running; then
    log_success "Cilium is running"
    CILIUM_VERSION=$(kubectl get pods -n kube-system -l k8s-app=cilium -o jsonpath='{.items[0].spec.containers[0].image}' | grep -oE 'v[0-9]+\.[0-9]+\.[0-9]+' || echo "unknown")
    log_info "Cilium version: $CILIUM_VERSION"
else
    log_warn "Cilium pods not found in kube-system"
    ISSUES+=("Cilium not detected")
fi

# ============================================================================
log_step "Checking Cilium L2 Announcement support..."
# ============================================================================

# Check for CiliumL2AnnouncementPolicy CRD
if kubectl api-resources | grep -q ciliuml2announcementpolic; then
    log_success "CiliumL2AnnouncementPolicy CRD exists"

    # Check if any policies exist
    L2_POLICIES=$(kubectl get ciliuml2announcementpolicy -A 2>/dev/null | grep -v "No resources" | tail -n +2 || true)
    if [[ -n "$L2_POLICIES" ]]; then
        log_success "L2 announcement policies found:"
        echo "$L2_POLICIES" | sed 's/^/    /'
    else
        log_warn "No L2 announcement policies configured"
        log_info "You may need to create one for ZTNA LoadBalancer"
        ISSUES+=("No L2 announcement policy")
    fi
else
    log_warn "CiliumL2AnnouncementPolicy CRD not found"
    log_info "Cilium may not have L2 announcements enabled"
    ISSUES+=("Cilium L2 CRD missing")
fi

# Check for CiliumLoadBalancerIPPool CRD
if kubectl api-resources | grep -q ciliumloadbalancerippoo; then
    log_success "CiliumLoadBalancerIPPool CRD exists"

    IP_POOLS=$(kubectl get ciliumloadbalancerippool -A 2>/dev/null | grep -v "No resources" | tail -n +2 || true)
    if [[ -n "$IP_POOLS" ]]; then
        log_success "IP pools found:"
        echo "$IP_POOLS" | sed 's/^/    /'
    else
        log_warn "No IP pools configured"
        ISSUES+=("No IP pool configured")
    fi
else
    log_warn "CiliumLoadBalancerIPPool CRD not found"
    ISSUES+=("Cilium LB IPAM CRD missing")
fi

# ============================================================================
log_step "Checking existing LoadBalancer services..."
# ============================================================================
LB_SVCS=$(kubectl get svc -A --field-selector spec.type=LoadBalancer 2>/dev/null | tail -n +2 || true)
if [[ -n "$LB_SVCS" ]]; then
    log_info "Existing LoadBalancer services:"
    echo "$LB_SVCS" | sed 's/^/    /'

    # Check if any have external IPs
    PENDING=$(echo "$LB_SVCS" | grep -c "<pending>" || true)
    if [[ "$PENDING" -gt 0 ]]; then
        log_warn "$PENDING LoadBalancer service(s) have <pending> external IP"
        log_info "This suggests L2/BGP may not be fully configured"
    fi
else
    log_info "No existing LoadBalancer services"
fi

# ============================================================================
log_step "Checking for MetalLB (alternative)..."
# ============================================================================
if kubectl get pods -n metallb-system 2>/dev/null | grep -q Running; then
    log_info "MetalLB is running (alternative to Cilium L2)"
else
    log_info "MetalLB not detected"
fi

# ============================================================================
log_step "Checking ZTNA namespace..."
# ============================================================================
if kubectl get namespace ztna &>/dev/null; then
    log_info "ZTNA namespace already exists"
    kubectl get pods -n ztna 2>/dev/null || true
else
    log_info "ZTNA namespace does not exist (will be created on deploy)"
fi

# ============================================================================
log_step "Checking image pull capabilities..."
# ============================================================================
# Try to check if GHCR is accessible
if kubectl run ghcr-test --image=ghcr.io/github/super-linter:v4 --dry-run=client -o yaml &>/dev/null; then
    log_info "kubectl can specify GHCR images"
    log_info "Note: Actual pull will depend on registry credentials"
else
    log_warn "Issue with image specification"
fi

# ============================================================================
# Summary
# ============================================================================
echo ""
echo -e "${BLUE}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║${NC}                        Summary                               ${BLUE}║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""

if [[ ${#ISSUES[@]} -eq 0 ]]; then
    log_success "Cluster appears ready for ZTNA deployment!"
else
    log_warn "Issues found that may need attention:"
    for issue in "${ISSUES[@]}"; do
        echo -e "    ${YELLOW}•${NC} $issue"
    done
    echo ""
    log_info "See README.md for Cilium L2 configuration examples"
fi

echo ""
echo "Next steps:"
echo "  1. Build and push images:  ./build-push.sh"
echo "  2. Generate TLS certs:     (see README.md)"
echo "  3. Deploy:                 kubectl apply -k overlays/pi-home"
echo ""
