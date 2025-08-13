// Cloudflare Workers environment types
export interface Env {
  ENVIRONMENT: string;
  GITHUB_CLIENT_ID: string;
  GITHUB_CLIENT_SECRET: string;
  JWT_SECRET_KEY: string;
  
  // Durable Objects
  B00T_MCP_INSTANCE: DurableObjectNamespace;
  
  // R2 Storage
  B00T_USER_DATA: R2Bucket;
  
  // KV Storage  
  B00T_SESSIONS: KVNamespace;
}

// GitHub OAuth types
export interface GitHubUser {
  id: number;
  login: string;
  name: string | null;
  email: string | null;
  avatar_url: string;
}

export interface GitHubTokenResponse {
  access_token: string;
  token_type: string;
  scope: string;
}

// b00t-mcp types
export interface UserSession {
  github_user: GitHubUser;
  authenticated_at: number;
  expires_at: number;
}

export interface OAuthState {
  return_url: string;
  created_at: number;
}

// MCP Protocol types
export interface McpRequest {
  jsonrpc: "2.0";
  id: string | number | null;
  method: string;
  params?: unknown;
}

export interface McpResponse {
  jsonrpc: "2.0";
  id: string | number | null;
  result?: unknown;
  error?: {
    code: number;
    message: string;
    data?: unknown;
  };
}

export interface McpTool {
  name: string;
  description: string;
  inputSchema: {
    type: "object";
    properties: Record<string, unknown>;
    required?: string[];
  };
}

// b00t tool execution types
export interface ToolExecutionResult {
  success: boolean;
  output?: string;
  error?: string;
  exit_code?: number;
}