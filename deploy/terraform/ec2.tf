# =============================================================================
# ZTNA Agent — EC2, VPC, Security Groups, Elastic IP
# =============================================================================

# ---------------------------------------------------------------------------
# Data source: latest Ubuntu 22.04 LTS AMI (used when var.ami_id is empty)
# ---------------------------------------------------------------------------
data "aws_ami" "ubuntu" {
  most_recent = true
  owners      = ["099720109477"] # Canonical

  filter {
    name   = "name"
    values = ["ubuntu/images/hvm-ssd/ubuntu-jammy-22.04-amd64-server-*"]
  }

  filter {
    name   = "virtualization-type"
    values = ["hvm"]
  }
}

locals {
  ami_id = var.ami_id != "" ? var.ami_id : data.aws_ami.ubuntu.id

  common_tags = merge(var.tags, {
    Project   = "ztna-agent"
    ManagedBy = "terraform"
  })
}

# ---------------------------------------------------------------------------
# VPC
# ---------------------------------------------------------------------------
resource "aws_vpc" "ztna" {
  cidr_block           = var.vpc_cidr
  enable_dns_support   = true
  enable_dns_hostnames = true

  tags = merge(local.common_tags, {
    Name = "ztna-vpc"
  })
}

# ---------------------------------------------------------------------------
# Internet Gateway
# ---------------------------------------------------------------------------
resource "aws_internet_gateway" "ztna" {
  vpc_id = aws_vpc.ztna.id

  tags = merge(local.common_tags, {
    Name = "ztna-igw"
  })
}

# ---------------------------------------------------------------------------
# Public Subnet
# ---------------------------------------------------------------------------
resource "aws_subnet" "public" {
  vpc_id                  = aws_vpc.ztna.id
  cidr_block              = var.subnet_cidr
  availability_zone       = var.availability_zone
  map_public_ip_on_launch = true

  tags = merge(local.common_tags, {
    Name = "ztna-public-subnet"
  })
}

# ---------------------------------------------------------------------------
# Route Table (public → IGW)
# ---------------------------------------------------------------------------
resource "aws_route_table" "public" {
  vpc_id = aws_vpc.ztna.id

  route {
    cidr_block = "0.0.0.0/0"
    gateway_id = aws_internet_gateway.ztna.id
  }

  tags = merge(local.common_tags, {
    Name = "ztna-public-rt"
  })
}

resource "aws_route_table_association" "public" {
  subnet_id      = aws_subnet.public.id
  route_table_id = aws_route_table.public.id
}

# ---------------------------------------------------------------------------
# Security Group
# ---------------------------------------------------------------------------
resource "aws_security_group" "ztna" {
  name        = "ztna-intermediate"
  description = "ZTNA Intermediate Server — QUIC, SSH, optional metrics and web"
  vpc_id      = aws_vpc.ztna.id

  tags = merge(local.common_tags, {
    Name = "ztna-intermediate-sg"
  })
}

# Egress: allow all outbound
resource "aws_vpc_security_group_egress_rule" "all_outbound" {
  security_group_id = aws_security_group.ztna.id
  ip_protocol       = "-1"
  cidr_ipv4         = "0.0.0.0/0"
  description       = "Allow all outbound traffic"
}

# Ingress: QUIC (UDP 4433) from anywhere
resource "aws_vpc_security_group_ingress_rule" "quic" {
  security_group_id = aws_security_group.ztna.id
  ip_protocol       = "udp"
  from_port         = 4433
  to_port           = 4433
  cidr_ipv4         = "0.0.0.0/0"
  description       = "QUIC relay — Intermediate Server"
}

# Ingress: P2P (UDP 4434) from anywhere — for future P2P hole-punching
resource "aws_vpc_security_group_ingress_rule" "p2p" {
  security_group_id = aws_security_group.ztna.id
  ip_protocol       = "udp"
  from_port         = 4434
  to_port           = 4434
  cidr_ipv4         = "0.0.0.0/0"
  description       = "P2P hole-punching port (future)"
}

# Ingress: SSH from allowed CIDRs only
resource "aws_vpc_security_group_ingress_rule" "ssh" {
  for_each = toset(var.allowed_ssh_cidrs)

  security_group_id = aws_security_group.ztna.id
  ip_protocol       = "tcp"
  from_port         = 22
  to_port           = 22
  cidr_ipv4         = each.value
  description       = "SSH from ${each.value}"
}

# Ingress: Prometheus metrics (TCP 9090) — conditional
resource "aws_vpc_security_group_ingress_rule" "metrics" {
  count = var.enable_metrics_port ? 1 : 0

  security_group_id = aws_security_group.ztna.id
  ip_protocol       = "tcp"
  from_port         = 9090
  to_port           = 9090
  cidr_ipv4         = var.metrics_cidr
  description       = "Prometheus metrics endpoint"
}

# Ingress: HTTP/HTTPS (TCP 80/443) — conditional (for certbot, web services)
resource "aws_vpc_security_group_ingress_rule" "http" {
  count = var.enable_web_ports ? 1 : 0

  security_group_id = aws_security_group.ztna.id
  ip_protocol       = "tcp"
  from_port         = 80
  to_port           = 80
  cidr_ipv4         = "0.0.0.0/0"
  description       = "HTTP (certbot challenge, redirect)"
}

resource "aws_vpc_security_group_ingress_rule" "https" {
  count = var.enable_web_ports ? 1 : 0

  security_group_id = aws_security_group.ztna.id
  ip_protocol       = "tcp"
  from_port         = 443
  to_port           = 443
  cidr_ipv4         = "0.0.0.0/0"
  description       = "HTTPS"
}

# ---------------------------------------------------------------------------
# EC2 Instance
# ---------------------------------------------------------------------------
resource "aws_instance" "ztna" {
  ami                    = local.ami_id
  instance_type          = var.instance_type
  key_name               = var.key_name
  subnet_id              = aws_subnet.public.id
  vpc_security_group_ids = [aws_security_group.ztna.id]

  metadata_options {
    http_endpoint               = "enabled"
    http_tokens                 = "required"
    http_put_response_hop_limit = 1
  }

  root_block_device {
    volume_size = 20
    volume_type = "gp3"
    encrypted   = true
  }

  user_data = templatefile("${path.module}/user_data.sh.tftpl", {
    domain_name = var.domain_name
  })

  tags = merge(local.common_tags, {
    Name = "ztna-intermediate-server"
  })

  lifecycle {
    # Set to true for production; false during development for easy teardown
    prevent_destroy = false
  }
}

# ---------------------------------------------------------------------------
# Elastic IP
# ---------------------------------------------------------------------------
resource "aws_eip" "ztna" {
  domain = "vpc"

  tags = merge(local.common_tags, {
    Name = "ztna-eip"
  })
}

resource "aws_eip_association" "ztna" {
  instance_id   = aws_instance.ztna.id
  allocation_id = aws_eip.ztna.id
}
