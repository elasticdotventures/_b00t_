#!/usr/bin/env bash
set -euo pipefail

: "${AWS_REGION:=us-east-1}"                # region used for all AWS CLI calls
: "${ECR_REPO:=b00t-sample-app}"            # ECR repository name
: "${ECS_CLUSTER:=b00t-sample-cluster}"     # ECS cluster where the task runs
: "${TASK_FAMILY:=b00t-sample-task}"        # task definition family
: "${CPU_UNITS:=256}"                       # Fargate CPU units (0.25 vCPU)
: "${MEMORY_MB:=512}"                       # Fargate memory (MiB)
: "${SUBNET_ID:=subnet-xxxxxxxx}"           # target subnet for awsvpc networking
: "${SECURITY_GROUP_ID:=sg-xxxxxxxx}"       # security group controlling ingress
: "${ECS_APP_CONTEXT:=ecs-app}"             # directory that contains a Dockerfile

if [ ! -d "$ECS_APP_CONTEXT" ]; then
  echo "⚠️  create $ECS_APP_CONTEXT with a Dockerfile before running" >&2
  exit 2
fi

AWS_ACCOUNT_ID=$(aws sts get-caller-identity --query 'Account' --output text)
IMAGE_TAG=${IMAGE_TAG:-$(date +%Y%m%d%H%M%S)}
IMAGE_URI="$AWS_ACCOUNT_ID.dkr.ecr.$AWS_REGION.amazonaws.com/$ECR_REPO:$IMAGE_TAG"

aws ecr describe-repositories --repository-names "$ECR_REPO" --region "$AWS_REGION" >/dev/null 2>&1 || \
  aws ecr create-repository --repository-name "$ECR_REPO" --region "$AWS_REGION" >/dev/null
# output: ensures the repository exists before pushing

aws ecr get-login-password --region "$AWS_REGION" | \
  docker login --username AWS --password-stdin "$AWS_ACCOUNT_ID.dkr.ecr.$AWS_REGION.amazonaws.com"
# output: docker reports "Login Succeeded" on success

docker build -t "$IMAGE_URI" "$ECS_APP_CONTEXT"
docker push "$IMAGE_URI"
# output: digest of the pushed image logged by docker

cat > /tmp/b00t-taskdef.json <<JSON
{
  "family": "$TASK_FAMILY",
  "networkMode": "awsvpc",
  "requiresCompatibilities": ["FARGATE"],
  "cpu": "$CPU_UNITS",
  "memory": "$MEMORY_MB",
  "containerDefinitions": [
    {
      "name": "app",
      "image": "$IMAGE_URI",
      "essential": true,
      "portMappings": [
        {"containerPort": 8080, "hostPort": 8080, "protocol": "tcp"}
      ],
      "logConfiguration": {
        "logDriver": "awslogs",
        "options": {
          "awslogs-group": "/ecs/$TASK_FAMILY",
          "awslogs-region": "$AWS_REGION",
          "awslogs-stream-prefix": "app"
        }
      }
    }
  ]
}
JSON

aws logs create-log-group --log-group-name "/ecs/$TASK_FAMILY" --region "$AWS_REGION" >/dev/null 2>&1 || true

TASK_DEF_ARN=$(aws ecs register-task-definition \
  --cli-input-json file:///tmp/b00t-taskdef.json \
  --query 'taskDefinition.taskDefinitionArn' \
  --output text \
  --region "$AWS_REGION")
# output: ARN of the registered task definition

TASK_ARN=$(aws ecs run-task \
  --cluster "$ECS_CLUSTER" \
  --launch-type FARGATE \
  --task-definition "$TASK_DEF_ARN" \
  --network-configuration "awsvpcConfiguration={subnets=[$SUBNET_ID],securityGroups=[$SECURITY_GROUP_ID],assignPublicIp=ENABLED}" \
  --query 'tasks[0].taskArn' \
  --output text \
  --region "$AWS_REGION")
# output: ARN of the started Fargate task

echo "Started task: $TASK_ARN"
aws logs tail "/ecs/$TASK_FAMILY" --region "$AWS_REGION" --since 1m --format short
# output: tail of the most recent log stream for quick health verification
