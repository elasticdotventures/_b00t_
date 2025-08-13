# 🌤️ b00t-mcp Cloudflare Deployment

Cloudflare Workers deployment for b00t-mcp with GitHub OAuth and per-user instances.

## 🏗️ Architecture

- **Domain Pattern**: `{github-username}.b00t.promptexecution.com`
- **Main Worker**: Routes requests to user-specific Durable Objects
- **Durable Objects**: Per-user b00t-mcp server instances
- **Authentication**: GitHub OAuth 2.0 with subdomain validation
- **Storage**: R2 for configs, KV for sessions

## 🚀 Deployment

### Prerequisites

1. **Cloudflare Account**: With Workers Paid plan ($5/month) for Durable Objects
2. **Domain Control**: `promptexecution.com` domain in Cloudflare
3. **GitHub OAuth App**: Created for your domain

### Setup Steps

#### 1. Install Dependencies

```bash
cd b00t-mcp-cloudflare
npm install
```

#### 2. Configure GitHub OAuth App

Create a GitHub OAuth app at https://github.com/settings/applications/new:

- **Application name**: `b00t-mcp Production`
- **Homepage URL**: `https://b00t.promptexecution.com`  
- **Authorization callback URL**: `https://*.b00t.promptexecution.com/auth/github/callback`

Note your Client ID and Client Secret.

#### 3. Configure Secrets

```bash
# Set GitHub OAuth credentials
wrangler secret put GITHUB_CLIENT_SECRET

# Set JWT signing key (generate random 256-bit key)
openssl rand -base64 32 | wrangler secret put JWT_SECRET_KEY
```

#### 4. Update wrangler.toml

Update the configuration in `wrangler.toml`:

```toml
# Set your GitHub Client ID
[env.production.vars]
GITHUB_CLIENT_ID = "your-github-client-id"

# Update R2 bucket and KV namespace IDs
[[r2_buckets]]
bucket_name = "b00t-user-data-prod"  # Create this bucket

[[kv_namespaces]]
id = "your-kv-namespace-id"  # Create this KV namespace
```

#### 5. Create Resources

```bash
# Create R2 bucket for user data
wrangler r2 bucket create b00t-user-data-prod

# Create KV namespace for sessions
wrangler kv:namespace create "B00T_SESSIONS"
wrangler kv:namespace create "B00T_SESSIONS" --preview

# Update wrangler.toml with the returned namespace IDs
```

#### 6. Configure DNS

In Cloudflare DNS, add a CNAME record:

```
Type: CNAME
Name: *
Content: b00t-mcp-cloudflare.your-subdomain.workers.dev
Proxy: Yes (Orange cloud)
```

Or use a Worker Route directly in the dashboard.

#### 7. Deploy

```bash
# Deploy to production
npm run deploy:production

# Test deployment
curl https://elasticdotventures.b00t.promptexecution.com/health
```

## 🧪 Local Development

```bash
# Start local development server
npm run dev

# Test endpoints
curl http://localhost:8787/health
curl http://localhost:8787  # Welcome page
```

## 🔧 Configuration

### Environment Variables

| Variable | Description | Required |
|----------|-------------|----------|
| `GITHUB_CLIENT_ID` | GitHub OAuth app client ID | Yes |
| `GITHUB_CLIENT_SECRET` | GitHub OAuth app secret | Yes |
| `JWT_SECRET_KEY` | JWT signing key (256-bit) | Yes |
| `ENVIRONMENT` | Deployment environment | No |

### Resources

| Resource | Purpose | 
|----------|---------|
| `B00T_MCP_INSTANCE` | Durable Object namespace for user instances |
| `B00T_USER_DATA` | R2 bucket for user configurations |
| `B00T_SESSIONS` | KV namespace for session storage |

## 🎯 Usage

### For Users

1. Visit `https://{your-github-username}.b00t.promptexecution.com`
2. Click "Login with GitHub" 
3. Authorize the application
4. Configure Claude with the MCP endpoint

### Claude Integration

Configure Claude Custom Connector:

```json
{
  "name": "b00t-mcp",
  "mcp_server_url": "https://{username}.b00t.promptexecution.com/mcp",
  "oauth": {
    "authorization_url": "https://{username}.b00t.promptexecution.com/oauth/authorize",
    "token_url": "https://{username}.b00t.promptexecution.com/oauth/token",
    "client_id": "b00t-mcp-client",
    "scopes": ["b00t:read", "b00t:write"]
  }
}
```

## 🛠️ Available Tools

| Tool | Description |
|------|-------------|
| `b00t_whoami` | Get user and environment information |
| `b00t_learn` | Learn about topics with curated resources |
| `b00t_status` | Get system status and health |
| `github_user_info` | Get GitHub user information |
| `github_repositories` | List GitHub repositories |

## 📊 Monitoring

### Health Checks

- `GET /{username}/health` - Instance health
- `GET /health` - Overall service health

### Logs

```bash
# View real-time logs
wrangler tail

# View logs for specific user instance
wrangler tail --format json | grep "username"
```

## 🔒 Security

### Authentication Flow

1. User visits `{username}.b00t.promptexecution.com`
2. Redirected to GitHub OAuth if not authenticated
3. GitHub validates user identity
4. Username must match subdomain (prevents impersonation)
5. JWT token issued with user claims
6. MCP requests require valid Bearer token

### Isolation

- Each user gets isolated Durable Object instance
- No cross-user data access possible
- GitHub OAuth prevents subdomain hijacking

### Rate Limiting

Default limits applied:
- 50ms CPU time per request
- Built-in Cloudflare DDoS protection
- Per-user resource isolation

## 💰 Cost Estimation

### Cloudflare Workers

- **Paid Plan**: $5/month base
- **Requests**: $0.15/million after 10M/month
- **Durable Objects**: $0.15/million requests + storage
- **R2 Storage**: $0.015/GB/month
- **KV**: $0.50/million reads, $5.00/million writes

### Expected Usage (50 active users)

- Monthly requests: ~1M
- Storage: ~1GB
- **Total**: ~$10-15/month

## 🚨 Troubleshooting

### Common Issues

**"Invalid GitHub username format"**
- Ensure subdomain matches GitHub username exactly
- Username must be valid GitHub format (alphanumeric, hyphens)

**"Authentication mismatch"**  
- User authenticated as different GitHub user than subdomain
- Must use your own subdomain: `{your-username}.b00t.promptexecution.com`

**"OAuth state expired"**
- OAuth state expires after 10 minutes
- Clear browser cache and try again

**"Durable Object not found"**
- Check Durable Object binding in wrangler.toml
- Verify deployment completed successfully

### Debug Commands

```bash
# Check deployment status
wrangler deployments list

# View KV storage
wrangler kv:key list --binding B00T_SESSIONS

# Test OAuth flow
curl -i "https://{username}.b00t.promptexecution.com/oauth/authorize?client_id=test&redirect_uri=https://example.com&response_type=code"
```

## 📈 Scaling

The architecture scales automatically:

- **Users**: Unlimited (each gets own Durable Object)
- **Requests**: Cloudflare global edge handles traffic
- **Storage**: R2 scales to petabytes
- **Geographic**: Durable Objects migrate to user regions

For high-volume usage, consider:
- Implementing request caching
- Using Cloudflare Analytics
- Setting up monitoring alerts