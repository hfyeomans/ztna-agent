# =============================================================================
# ZTNA Agent — Terraform Variables
# =============================================================================

variable "aws_region" {
  description = "AWS region to deploy into"
  type        = string
  default     = "us-east-2"
}

variable "instance_type" {
  description = "EC2 instance type"
  type        = string
  default     = "t3.micro"
}

variable "key_name" {
  description = "Name of the AWS SSH key pair (must already exist in the target region)"
  type        = string
}

variable "allowed_ssh_cidrs" {
  description = "CIDR blocks allowed to SSH into the instance"
  type        = list(string)
  default     = []
  # Example: ["203.0.113.0/32"] — never use 0.0.0.0/0 in production
}

variable "domain_name" {
  description = "Optional domain name for TLS certificate (used in certbot setup)"
  type        = string
  default     = ""
}

variable "vpc_cidr" {
  description = "CIDR block for the VPC"
  type        = string
  default     = "10.0.0.0/16"
}

variable "subnet_cidr" {
  description = "CIDR block for the public subnet"
  type        = string
  default     = "10.0.2.0/24"
}

variable "availability_zone" {
  description = "Availability zone for the subnet"
  type        = string
  default     = "us-east-2a"
}

variable "ami_id" {
  description = "AMI ID for the EC2 instance (default: latest Ubuntu 22.04 LTS via data source)"
  type        = string
  default     = ""
}

variable "enable_metrics_port" {
  description = "Whether to open TCP 9090 for Prometheus metrics scraping"
  type        = bool
  default     = false
}

variable "enable_web_ports" {
  description = "Whether to open TCP 80/443 for HTTP/HTTPS (certbot, web services)"
  type        = bool
  default     = false
}

variable "tags" {
  description = "Additional tags to apply to all resources"
  type        = map(string)
  default     = {}
}
