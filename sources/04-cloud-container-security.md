# Cloud & Container Security

## AWS Exploitation

### IAM Privilege Escalation
- Overly permissive IAM policies (`Action: "*"`, `Resource: "*"`)
- PassRole → Lambda/EC2 that can assume more privileged roles
- iam:CreatePolicyVersion with `SetAsDefault: false` bypass
- iam:UpdateAssumeRolePolicy → cross-account escalation
- sts:AssumeRole on untrusted accounts

### S3 Attacks
- Public bucket enumeration (S3Scanner, BucketLoot, GrayhatWarfare)
- Bucket policy misconfiguration for write/read without auth
- Bucket ACL confusion between bucket ACL and object ACL
- **Global Namespace Hijacking**: Claim unowned bucket names
- Terraform state files with plaintext secrets

### Metadata Service
- **IMDSv1**: Request `http://169.254.169.254/latest/meta-data/` → role credentials
- **IMDSv2**: Requires token but SSRF in container metadata service can leak
- Cloud provider metadata across AWS/Azure/GCP

### Lambda Exploitation
- Third-party dependency poisoning (dependency confusion)
- Overly permissive execution role
- Lambda persistence via event triggers

## Azure Exploitation
- **Passthrough token abuse**: Inherited managed identity from VM to resource
- **Key Vault misconfiguration**: Weak RBAC on secrets
- **ARM Template injection**: Command injection via deployment templates
- **Azure AD OAuth**: Consent grant attack, application impersonation

## GCP Exploitation
- **Service Account key leakage**: Public GCS buckets, container registries
- **Default Compute Engine SA**: Editor role on all VMs
- **Cloud Functions**: Function swapping with privileged SA
- **IAM role chains**: Transitive privilege via folder/project inheritance

## Container Escape

### Techniques
- **Capability abuse**: `SYS_ADMIN`, `SYS_PTRACE`, `SYS_MODULE`, `DAC_OVERRIDE`
- **Mount escape**:
  - Host `/var/run/docker.sock` mounted → `docker exec` on host
  - Host `/proc` mounted → `nsenter` into host namespaces
  - Host `/dev` devices → raw disk access
- **runC vulnerability** (CVE-2019-5736): Overwrite host runC binary
- **cgroup escape**: `notify_on_release` to execute on host
- **Linux syscall abuse**: `clone(CLONE_NEWNS\|CLONE_NEWPID)`, `unshare(CLONE_NEWNS)`
- **Kernel 0-days**: Dirty Pipe (CVE-2022-0847), Dirty COW (CVE-2016-5195)

### Research
- "From Container to Cluster: Chained Escape Attacks in Kubernetes" - IEEE 2025
- "Container Breakouts: Escape Techniques in Cloud Environments" - Unit42 (Palo Alto)
- "Blinding the Watchmen: Cloud Logging as an Attack Surface" - CSA 2026

## Kubernetes Attacks
- **Unsecured Dashboard**: `kubectl proxy` exposed externally
- **etcd access**: Stored cluster secrets
- **RBAC abuse**: Overly permissive ClusterRoles
- **Admission Controller bypass**: Mutating webhooks
- **CRD injection**: Custom resources for persistence

## Cloud Tools
- **Prowler**: AWS security scanning
- **ScoutSuite**: Multi-cloud auditing
- **CloudSploit**: Cloud security scanner
- **Hacking the Cloud** (github.com/Hacking-the-Cloud/hackingthe.cloud): Cloud security encyclopedia
- **kuberspray**: Kubernetes security assessment
- **Cloudimposer**: GCP dependency confusion (Tenable 2024)
