#!/bin/bash
set -e

# b00t-mcp Cloudflare Deployment Script
# Automates the deployment of b00t-mcp to Cloudflare Workers

echo "🌤️ b00t-mcp Cloudflare Deployment"
echo "=================================="

# Check dependencies
command -v wrangler >/dev/null 2>&1 || { echo "❌ wrangler CLI not found. Install with: npm install -g wrangler"; exit 1; }
command -v node >/dev/null 2>&1 || { echo "❌ Node.js not found. Install Node.js first."; exit 1; }

# Set environment (default to staging)
ENVIRONMENT=${1:-staging}
echo "📦 Deploying to: $ENVIRONMENT"

# Install dependencies
echo "📥 Installing dependencies..."
npm ci

# Type check
echo "🔍 Type checking..."
npm run type-check

# Create resources if they don't exist
echo "🛠️ Setting up Cloudflare resources..."

# Create R2 bucket
BUCKET_NAME="b00t-user-data-${ENVIRONMENT}"
echo "Creating R2 bucket: $BUCKET_NAME"
wrangler r2 bucket create "$BUCKET_NAME" 2>/dev/null || echo "Bucket already exists"

# Create KV namespaces
echo "Creating KV namespaces..."
if [ "$ENVIRONMENT" = "production" ]; then
    KV_OUTPUT=$(wrangler kv:namespace create "B00T_SESSIONS" 2>/dev/null || echo "exists")
    KV_PREVIEW_OUTPUT=$(wrangler kv:namespace create "B00T_SESSIONS" --preview 2>/dev/null || echo "exists")
else
    KV_OUTPUT=$(wrangler kv:namespace create "B00T_SESSIONS_STAGING" 2>/dev/null || echo "exists")
    KV_PREVIEW_OUTPUT=$(wrangler kv:namespace create "B00T_SESSIONS_STAGING" --preview 2>/dev/null || echo "exists")
fi

# Extract namespace IDs if created
if [[ "$KV_OUTPUT" == *"id ="* ]]; then
    KV_ID=$(echo "$KV_OUTPUT" | grep -o 'id = "[^"]*"' | cut -d'"' -f2)
    echo "✅ KV namespace created: $KV_ID"
fi

if [[ "$KV_PREVIEW_OUTPUT" == *"id ="* ]]; then
    KV_PREVIEW_ID=$(echo "$KV_PREVIEW_OUTPUT" | grep -o 'id = "[^"]*"' | cut -d'"' -f2)
    echo "✅ KV preview namespace created: $KV_PREVIEW_ID"
fi

# Validate secrets
echo "🔐 Checking secrets..."

# Check if secrets exist
if ! wrangler secret list | grep -q "GITHUB_CLIENT_SECRET"; then
    echo "⚠️  GITHUB_CLIENT_SECRET not set"
    echo "Run: wrangler secret put GITHUB_CLIENT_SECRET"
    read -p "Enter GitHub Client Secret: " -s github_secret
    echo
    echo "$github_secret" | wrangler secret put GITHUB_CLIENT_SECRET
    echo "✅ GitHub Client Secret set"
fi

if ! wrangler secret list | grep -q "JWT_SECRET_KEY"; then
    echo "⚠️  JWT_SECRET_KEY not set"
    echo "Generating random JWT secret..."
    JWT_SECRET=$(openssl rand -base64 32 2>/dev/null || head -c 32 /dev/urandom | base64)
    echo "$JWT_SECRET" | wrangler secret put JWT_SECRET_KEY
    echo "✅ JWT Secret Key generated and set"
fi

# Deploy
echo "🚀 Deploying to Cloudflare Workers..."
if [ "$ENVIRONMENT" = "production" ]; then
    npm run deploy:production
else
    npm run deploy:staging
fi

# Get deployment URL
WORKER_URL=$(wrangler deployments list --name b00t-mcp-cloudflare 2>/dev/null | head -2 | tail -1 | awk '{print $4}' || echo "unknown")

echo ""
echo "🎉 Deployment Complete!"
echo "======================"
echo "Environment: $ENVIRONMENT"
echo "Worker URL: $WORKER_URL"
echo ""

if [ "$ENVIRONMENT" = "production" ]; then
    echo "🌐 Production URLs:"
    echo "   Landing page: https://b00t.promptexecution.com"
    echo "   Your instance: https://elasticdotventures.b00t.promptexecution.com"
    echo "   Health check: https://elasticdotventures.b00t.promptexecution.com/health"
    echo ""
    echo "🔧 Next Steps:"
    echo "1. Configure Cloudflare DNS:"
    echo "   - Add CNAME: * → b00t-mcp-cloudflare.your-workers-subdomain.workers.dev"
    echo "   - Or set up Worker Routes in Cloudflare Dashboard"
    echo ""
    echo "2. Test GitHub OAuth:"
    echo "   - Visit: https://elasticdotventures.b00t.promptexecution.com/auth/github"
    echo ""
    echo "3. Configure Claude Custom Connector:"
    echo "   - MCP URL: https://elasticdotventures.b00t.promptexecution.com/mcp"
    echo "   - OAuth URLs: /oauth/authorize, /oauth/token"
else
    echo "🧪 Staging URLs:"
    echo "   Worker: $WORKER_URL"
    echo "   Health: $WORKER_URL/health"
    echo ""
    echo "Test with: curl $WORKER_URL/health"
fi

echo ""
echo "📊 Monitor deployment:"
echo "   Logs: wrangler tail"
echo "   Analytics: wrangler dash"
echo ""
echo "📚 Documentation: ./README.md"