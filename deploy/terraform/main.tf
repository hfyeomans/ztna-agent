# =============================================================================
# ZTNA Agent — Terraform Root Module
# =============================================================================
# Provisions AWS infrastructure for the ZTNA Intermediate Server and
# App Connector: VPC, public subnet, security groups, EC2 instance with
# Elastic IP, and a user-data bootstrap script.
#
# Usage:
#   cp terraform.tfvars.example terraform.tfvars   # edit values
#   terraform init
#   terraform plan
#   terraform apply
# =============================================================================

terraform {
  required_version = ">= 1.5"

  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
  }
}

provider "aws" {
  region = var.aws_region
}
