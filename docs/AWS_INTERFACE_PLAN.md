# AWS Interface Plan (CLI-First)

## Lessons From the Proto `b00t-aws-tools`
1. **Context drift**: the package lived outside `.☁️`, so it ignored the hive convention where all cloud scaffolding resides. Agents never discovered it organically.
2. **Stubbed config**: `AwsHelper` hard-coded `us-east-1` with a fake config loader, so it could not run in production contexts or respect datums/secrets.
3. **No tangible tools**: `AWSToolGenerator.generate_tools()` returned `[]`, meaning no MCP tools or CLI workflows existed. The docs promised a generator but shipped none.
4. **SDK duplication**: the helper simply wrapped `boto3` without adding hive value, violating NRtW since AWS already publishes MCP servers + SDK tooling.
5. **Missing ceremony**: there was no archival note, no `.☁️` linkage, no shared skill to teach agents how to use AWS safely.

## Strategic Direction
- **CLI-first execution**: rely on the official `aws` CLI (already installed + authorized via SSO) for all first-phase workflows (Lambda deploy/invoke, ECS, S3). This keeps the surface area small and DRY.
- **`.☁️` canon**: keep every AWS script/runbook under `_b00t_/clouds.云☁️/aws.🦉.云☁️/`, mirroring Azure. Scripts are sketches that can be wrapped by `just` once proven.
- **Skills as entry points**: introduce `skills/aws-cli` so agents consciously load AWS context (b00t learn aws) before touching credentials.
- **Progressive enhancement**: after CLI flows earn cake, wrap them in MCP tool shims or leverage the official `awslabs/mcp` servers instead of inventing new helpers.
- **Credential gating**: never bake secrets; require operators to export `AWS_REGION`, bucket names, subnet IDs, etc., so the same scripts run in any account with the right IAM role.

## Immediate Artifacts
1. `_b00t_/clouds.云☁️/aws.🦉.云☁️/README.md` – runbook + references.
2. `lambda.invoke.sketch.sh` – package + deploy + invoke loop for Lambda.
3. `ecs.fargate.sketch.sh` – publish container to ECR and launch on ECS/Fargate.
4. `s3.bucket.sketch.sh` – high-value utility to list/download objects from S3 buckets.
5. `skills/aws-cli/SKILL.md` – documents how/when to operate the AWS CLI workflows.

## Roadmap
- **Add `just aws-lambda` / `just aws-ecs`** wrappers once the sketches are validated in a target account.
- **Surface MCP bridge**: evaluate AWS official MCP servers (https://github.com/awslabs/mcp) and document how to enable them per account.
- **Artifacts as datums**: capture bucket names, account IDs, and network parameters in datums or `.toml` config so the CLI commands remain parameterized.
- **Testing harness**: add integration tests that mock the CLI via `aws --endpoint-url=http://localstack` for safe CI validation.
