# AWS 🦉 云☁️ Playbook

This runbook keeps AWS-specific knowledge in the canonical `.☁️` tree, mirroring the Azure layout but focused on CLI-centric workflows b00t already trusts (no bespoke SDK wrappers).

## Prerequisites
- `aws` CLI configured via SSO or keys (`aws configure sso` recommended).
- `jq`, `tar`, `zip`, and `docker` available locally.
- Application artifacts: Lambda zip payloads, OCI images, or data files.

## Sketches
Each script emits machine-readable receipts (JSON/TSV) and human-readable hints; wire them into `just` once validated in a target AWS account.

### Lambda Deploy + Invoke
`lambda.invoke.sketch.sh` builds a payload zip, updates the function, and performs a smoke invoke.
```bash
AWS_LAMBDA_FUNCTION=my-fn AWS_REGION=ap-southeast-2 ./lambda.invoke.sketch.sh
# output: JSON deploy receipt + invocation result payload path
```
Highlights:
- Packages handler code into `/tmp/lambda_payload.zip`.
- Runs `aws lambda update-function-code` and `aws lambda invoke` with logging enabled.

### ECS/Fargate Container Rollout
`ecs.fargate.sketch.sh` publishes an image to ECR and runs it on ECS Fargate.
```bash
ECR_REPO=b00t-svc ECS_CLUSTER=hive AWS_REGION=us-east-1 SUBNET_ID=subnet-123 SECURITY_GROUP_ID=sg-456 ./ecs.fargate.sketch.sh
# output: ARN of the started task + CloudWatch log tail
```
Workflow:
- Builds/pushes Docker image.
- Registers task definition and runs it with `awsvpc` networking.
- Streams logs from `/ecs/<family>` for fast validation.

### S3 Bucket Access
`s3.bucket.sketch.sh` lists keys and optionally downloads an object.
```bash
AWS_S3_BUCKET=my-bucket AWS_S3_PREFIX=artifacts/ ./s3.bucket.sketch.sh
AWS_S3_BUCKET=my-bucket AWS_S3_DOWNLOAD_KEY=logs/run.json ./s3.bucket.sketch.sh
# output: TSV listing + optional downloaded file path
```
Outputs:
- TSV list stored at `/tmp/aws_s3_listing.txt`.
- Downloaded object stored under `$AWS_S3_DOWNLOAD_DEST` (default `/tmp`).

## Next Steps
- Promote these sketches into `just aws-lambda`, `just aws-ecs`, and `just aws-s3` once credentials + target infra are standardized.
- Adopt AWS MCP servers (https://github.com/awslabs/mcp) for richer tool surfaces after CLI flows prove reliable.
- Keep networking/account IDs in datums or secure config, never hardcode them inside scripts.
🤓  Keep AWS orchestration DRY by leaning on the CLI + shared `.☁️` patterns, never ad-hoc single-use scripts.
