# =============================================================================
# ZTNA Agent — Terraform Outputs
# =============================================================================

output "public_ip" {
  description = "Elastic IP address of the ZTNA server"
  value       = aws_eip.ztna.public_ip
}

output "instance_id" {
  description = "EC2 instance ID"
  value       = aws_instance.ztna.id
}

output "instance_private_ip" {
  description = "Private IP of the EC2 instance within the VPC"
  value       = aws_instance.ztna.private_ip
}

output "security_group_id" {
  description = "Security group ID for the ZTNA instance"
  value       = aws_security_group.ztna.id
}

output "vpc_id" {
  description = "VPC ID"
  value       = aws_vpc.ztna.id
}

output "subnet_id" {
  description = "Public subnet ID"
  value       = aws_subnet.public.id
}

output "ssh_command" {
  description = "SSH command to connect to the instance"
  value       = "ssh -i <key-file> ubuntu@${aws_eip.ztna.public_ip}"
}

output "eip_allocation_id" {
  description = "Elastic IP allocation ID"
  value       = aws_eip.ztna.id
}
