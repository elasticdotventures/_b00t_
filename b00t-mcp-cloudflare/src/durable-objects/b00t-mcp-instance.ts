import { Env, McpRequest, McpResponse, McpTool, UserSession } from '../types';
import { verifyUserSession, generateAccessToken, verifyAccessToken } from '../lib/oauth';
import { 
  generateErrorResponse, 
  createJsonResponse, 
  createHtmlResponse,
  extractBearerToken,
  handleCors 
} from '../lib/utils';
import { getB00tTools, executeB00tTool } from '../lib/tools';

/**
 * Durable Object for per-user b00t-mcp instances
 * Each GitHub user gets their own isolated instance
 */
export class B00tMcpInstance {
  private state: DurableObjectState;
  private env: Env;
  private username: string;

  constructor(state: DurableObjectState, env: Env) {
    this.state = state;
    this.env = env;
    this.username = ''; // Will be set from request
  }

  async fetch(request: Request): Promise<Response> {
    try {
      // Handle CORS preflight
      const corsResponse = handleCors(request);
      if (corsResponse) return corsResponse;

      const url = new URL(request.url);
      
      // Extract username from hostname
      const hostname = url.hostname;
      const subdomain = hostname.split('.')[0];
      this.username = subdomain;

      // Route requests
      if (url.pathname === '/health') {
        return this.handleHealth();
      }

      if (url.pathname.startsWith('/oauth/')) {
        return this.handleOAuthEndpoints(request);
      }

      if (url.pathname === '/mcp' || url.pathname.startsWith('/mcp/')) {
        return this.handleMcpRequest(request);
      }

      // Default: serve instance info page
      return this.handleInstanceInfo();

    } catch (error) {
      console.error('B00tMcpInstance error:', error);
      return generateErrorResponse(500, 'Internal server error');
    }
  }

  /**
   * Handle health check
   */
  private async handleHealth(): Promise<Response> {
    return createJsonResponse({
      status: 'healthy',
      username: this.username,
      timestamp: new Date().toISOString(),
      instance_id: this.state.id.toString(),
    });
  }

  /**
   * Handle OAuth endpoints (authorize, token, consent)
   */
  private async handleOAuthEndpoints(request: Request): Promise<Response> {
    const url = new URL(request.url);
    
    switch (url.pathname) {
      case '/oauth/authorize':
        return this.handleAuthorize(request);
      case '/oauth/token':
        return this.handleToken(request);
      case '/oauth/consent':
        return this.handleConsent(request);
      case '/.well-known/oauth-authorization-server':
        return this.handleDiscovery();
      default:
        return generateErrorResponse(404, 'OAuth endpoint not found');
    }
  }

  /**
   * OAuth authorization endpoint
   */
  private async handleAuthorize(request: Request): Promise<Response> {
    const url = new URL(request.url);
    const clientId = url.searchParams.get('client_id');
    const redirectUri = url.searchParams.get('redirect_uri');
    const state = url.searchParams.get('state');
    const responseType = url.searchParams.get('response_type');
    const sessionId = url.searchParams.get('session');

    // Validate required parameters
    if (!clientId || !redirectUri || responseType !== 'code') {
      return generateErrorResponse(400, 'Invalid OAuth request parameters');
    }

    // Check if user is authenticated
    if (sessionId) {
      const githubUser = await verifyUserSession(this.env, this.username, sessionId);
      if (githubUser) {
        // User authenticated, redirect to consent
        const consentUrl = new URL('/oauth/consent', request.url);
        consentUrl.searchParams.set('client_id', clientId);
        consentUrl.searchParams.set('redirect_uri', redirectUri);
        consentUrl.searchParams.set('state', state || '');
        consentUrl.searchParams.set('session', sessionId);
        
        return Response.redirect(consentUrl.toString(), 302);
      }
    }

    // User not authenticated, redirect to GitHub login
    const loginUrl = new URL('/auth/github', request.url);
    const returnTo = `/oauth/authorize?${url.searchParams.toString()}`;
    loginUrl.searchParams.set('return_to', returnTo);
    
    return Response.redirect(loginUrl.toString(), 302);
  }

  /**
   * OAuth consent page
   */
  private async handleConsent(request: Request): Promise<Response> {
    const url = new URL(request.url);
    
    if (request.method === 'GET') {
      const sessionId = url.searchParams.get('session');
      const clientId = url.searchParams.get('client_id');
      
      if (!sessionId) {
        return generateErrorResponse(400, 'Missing session parameter');
      }

      const githubUser = await verifyUserSession(this.env, this.username, sessionId);
      if (!githubUser) {
        return generateErrorResponse(401, 'Invalid session');
      }

      return createHtmlResponse(this.getConsentPageHtml(githubUser.login, clientId || 'unknown'));
    }

    if (request.method === 'POST') {
      return this.handleConsentSubmission(request);
    }

    return generateErrorResponse(405, 'Method not allowed');
  }

  /**
   * Handle consent form submission
   */
  private async handleConsentSubmission(request: Request): Promise<Response> {
    try {
      const formData = await request.formData();
      const action = formData.get('action');
      const sessionId = formData.get('session_id') as string;
      const redirectUri = formData.get('redirect_uri') as string;
      const state = formData.get('state') as string;

      if (!sessionId || !redirectUri) {
        return generateErrorResponse(400, 'Missing required parameters');
      }

      if (action === 'deny') {
        const errorUrl = new URL(redirectUri);
        errorUrl.searchParams.set('error', 'access_denied');
        if (state) errorUrl.searchParams.set('state', state);
        return Response.redirect(errorUrl.toString(), 302);
      }

      if (action === 'allow') {
        // Generate authorization code
        const authCode = crypto.randomUUID();
        
        // Store authorization code (expires in 10 minutes)
        await this.state.storage.put(`auth_code:${authCode}`, sessionId, {
          expirationTtl: 600
        });

        const successUrl = new URL(redirectUri);
        successUrl.searchParams.set('code', authCode);
        if (state) successUrl.searchParams.set('state', state);
        
        return Response.redirect(successUrl.toString(), 302);
      }

      return generateErrorResponse(400, 'Invalid action');
    } catch (error) {
      console.error('Consent submission error:', error);
      return generateErrorResponse(500, 'Failed to process consent');
    }
  }

  /**
   * OAuth token endpoint
   */
  private async handleToken(request: Request): Promise<Response> {
    if (request.method !== 'POST') {
      return generateErrorResponse(405, 'Method not allowed');
    }

    try {
      const formData = await request.formData();
      const grantType = formData.get('grant_type');
      const code = formData.get('code') as string;
      const clientId = formData.get('client_id');
      const clientSecret = formData.get('client_secret');

      if (grantType !== 'authorization_code') {
        return generateErrorResponse(400, 'Unsupported grant type');
      }

      if (!code || !clientId) {
        return generateErrorResponse(400, 'Missing required parameters');
      }

      // Retrieve and verify authorization code
      const sessionId = await this.state.storage.get(`auth_code:${code}`);
      if (!sessionId) {
        return generateErrorResponse(400, 'Invalid authorization code');
      }

      // Clean up authorization code
      await this.state.storage.delete(`auth_code:${code}`);

      // Get user session
      const githubUser = await verifyUserSession(this.env, this.username, sessionId as string);
      if (!githubUser) {
        return generateErrorResponse(400, 'Invalid session');
      }

      // Generate access token
      const accessToken = await generateAccessToken(this.env, githubUser, this.username);

      return createJsonResponse({
        access_token: accessToken,
        token_type: 'Bearer',
        expires_in: 3600,
        scope: 'b00t:read b00t:write',
      });

    } catch (error) {
      console.error('Token endpoint error:', error);
      return generateErrorResponse(500, 'Failed to generate token');
    }
  }

  /**
   * OAuth discovery endpoint
   */
  private async handleDiscovery(): Promise<Response> {
    const baseUrl = `https://${this.username}.b00t.promptexecution.com`;
    
    return createJsonResponse({
      issuer: baseUrl,
      authorization_endpoint: `${baseUrl}/oauth/authorize`,
      token_endpoint: `${baseUrl}/oauth/token`,
      response_types_supported: ['code'],
      grant_types_supported: ['authorization_code'],
      scopes_supported: ['b00t:read', 'b00t:write'],
      code_challenge_methods_supported: ['plain'],
    });
  }

  /**
   * Handle MCP protocol requests
   */
  private async handleMcpRequest(request: Request): Promise<Response> {
    // Verify authentication
    const authHeader = request.headers.get('Authorization');
    const token = extractBearerToken(authHeader);
    
    if (!token) {
      return generateErrorResponse(401, 'Missing or invalid authorization token');
    }

    const tokenData = await verifyAccessToken(this.env, token);
    if (!tokenData || tokenData.username !== this.username) {
      return generateErrorResponse(401, 'Invalid or expired token');
    }

    const url = new URL(request.url);
    
    if (url.pathname === '/mcp' && request.method === 'GET') {
      return this.handleMcpCapabilities();
    }

    if (url.pathname === '/mcp/tools' && request.method === 'GET') {
      return this.handleMcpListTools();
    }

    if (url.pathname === '/mcp/tools/call' && request.method === 'POST') {
      return this.handleMcpCallTool(request);
    }

    // Handle JSON-RPC requests
    if (request.method === 'POST') {
      return this.handleMcpJsonRpc(request);
    }

    return generateErrorResponse(404, 'MCP endpoint not found');
  }

  /**
   * Handle MCP capabilities
   */
  private async handleMcpCapabilities(): Promise<Response> {
    return createJsonResponse({
      version: '1.0.0',
      implementation: {
        name: 'b00t-mcp-cloudflare',
        version: '1.0.0',
      },
      capabilities: {
        tools: {
          listChanged: false,
        },
        resources: {
          subscribe: false,
          listChanged: false,
        },
        prompts: {
          listChanged: false,
        },
        logging: {},
      },
    });
  }

  /**
   * Handle MCP list tools
   */
  private async handleMcpListTools(): Promise<Response> {
    const tools = await getB00tTools(this.username);
    return createJsonResponse({ tools });
  }

  /**
   * Handle MCP tool execution
   */
  private async handleMcpCallTool(request: Request): Promise<Response> {
    try {
      const body = await request.json();
      const { name, arguments: args } = body;
      
      if (!name) {
        return generateErrorResponse(400, 'Missing tool name');
      }

      const result = await executeB00tTool(name, args || {}, this.username, this.env);
      
      return createJsonResponse({
        content: [
          {
            type: 'text',
            text: result.output || result.error || 'Tool executed',
          },
        ],
        isError: !result.success,
      });

    } catch (error) {
      console.error('Tool execution error:', error);
      return generateErrorResponse(500, 'Tool execution failed');
    }
  }

  /**
   * Handle JSON-RPC MCP requests
   */
  private async handleMcpJsonRpc(request: Request): Promise<Response> {
    try {
      const mcpRequest: McpRequest = await request.json();
      
      const response: McpResponse = {
        jsonrpc: '2.0',
        id: mcpRequest.id,
      };

      switch (mcpRequest.method) {
        case 'initialize':
          response.result = {
            protocolVersion: '2024-11-05',
            serverInfo: {
              name: 'b00t-mcp-cloudflare',
              version: '1.0.0',
            },
            capabilities: {
              tools: { listChanged: false },
            },
          };
          break;

        case 'tools/list':
          const tools = await getB00tTools(this.username);
          response.result = { tools };
          break;

        case 'tools/call':
          const { name, arguments: args } = mcpRequest.params as any;
          const result = await executeB00tTool(name, args || {}, this.username, this.env);
          
          response.result = {
            content: [
              {
                type: 'text',
                text: result.output || result.error || 'Tool executed',
              },
            ],
            isError: !result.success,
          };
          break;

        default:
          response.error = {
            code: -32601,
            message: 'Method not found',
            data: { method: mcpRequest.method },
          };
      }

      return createJsonResponse(response);

    } catch (error) {
      console.error('JSON-RPC error:', error);
      return createJsonResponse({
        jsonrpc: '2.0',
        id: null,
        error: {
          code: -32700,
          message: 'Parse error',
        },
      }, 400);
    }
  }

  /**
   * Handle instance info page
   */
  private async handleInstanceInfo(): Promise<Response> {
    return createHtmlResponse(this.getInstanceInfoHtml());
  }

  /**
   * Generate consent page HTML
   */
  private getConsentPageHtml(githubUsername: string, clientId: string): string {
    return `
      <!DOCTYPE html>
      <html>
      <head>
          <title>b00t-mcp Authorization</title>
          <style>
              body { 
                  font-family: Arial, sans-serif; 
                  margin: 40px; 
                  text-align: center; 
                  background: #f5f5f5; 
              }
              .container { 
                  max-width: 500px; 
                  margin: 0 auto; 
                  background: white;
                  padding: 40px;
                  border-radius: 8px;
                  box-shadow: 0 2px 10px rgba(0,0,0,0.1);
              }
              .user-info {
                  background: #e3f2fd;
                  padding: 15px;
                  border-radius: 5px;
                  margin: 20px 0;
              }
              button { 
                  padding: 12px 24px; 
                  margin: 10px; 
                  font-size: 16px; 
                  cursor: pointer; 
                  border: none;
                  border-radius: 5px;
              }
              .allow { background: #4CAF50; color: white; }
              .deny { background: #f44336; color: white; }
              .allow:hover { background: #45a049; }
              .deny:hover { background: #da190b; }
          </style>
      </head>
      <body>
          <div class="container">
              <h2>🥾 b00t-mcp Authorization</h2>
              <p>Claude is requesting access to your b00t tools.</p>
              
              <div class="user-info">
                  <strong>Authenticated as:</strong> ${githubUsername}<br>
                  <strong>Client:</strong> ${clientId}<br>
                  <strong>Instance:</strong> ${this.username}.b00t.promptexecution.com
              </div>
              
              <p>This will allow Claude to execute b00t-cli commands on your behalf through the MCP protocol.</p>
              
              <form method="post" action="/oauth/consent" style="display: inline;">
                  <input type="hidden" name="action" value="allow" />
                  <input type="hidden" name="session_id" value="${new URL(location.href).searchParams.get('session')}" />
                  <input type="hidden" name="redirect_uri" value="${new URL(location.href).searchParams.get('redirect_uri')}" />
                  <input type="hidden" name="state" value="${new URL(location.href).searchParams.get('state')}" />
                  <button type="submit" class="allow">✅ Allow Access</button>
              </form>
              
              <form method="post" action="/oauth/consent" style="display: inline;">
                  <input type="hidden" name="action" value="deny" />
                  <input type="hidden" name="session_id" value="${new URL(location.href).searchParams.get('session')}" />
                  <input type="hidden" name="redirect_uri" value="${new URL(location.href).searchParams.get('redirect_uri')}" />
                  <input type="hidden" name="state" value="${new URL(location.href).searchParams.get('state')}" />
                  <button type="submit" class="deny">❌ Deny Access</button>
              </form>
          </div>
      </body>
      </html>
    `;
  }

  /**
   * Generate instance info page HTML
   */
  private getInstanceInfoHtml(): string {
    return `
      <!DOCTYPE html>
      <html>
      <head>
          <title>🥾 ${this.username} - b00t-mcp Instance</title>
          <style>
              body { 
                  font-family: Arial, sans-serif; 
                  max-width: 800px; 
                  margin: 0 auto; 
                  padding: 40px 20px; 
                  background: #f5f5f5;
              }
              .container {
                  background: white;
                  padding: 40px;
                  border-radius: 8px;
                  box-shadow: 0 2px 10px rgba(0,0,0,0.1);
              }
              .header { text-align: center; margin-bottom: 40px; }
              .endpoint { 
                  background: #f8f9fa; 
                  border-left: 4px solid #4CAF50;
                  padding: 15px; 
                  margin: 15px 0; 
                  font-family: monospace;
                  word-break: break-all;
              }
              .section { margin: 30px 0; }
              a { color: #2196F3; text-decoration: none; }
              a:hover { text-decoration: underline; }
          </style>
      </head>
      <body>
          <div class="container">
              <div class="header">
                  <h1>🥾 ${this.username}</h1>
                  <p>Personal b00t-mcp Instance</p>
              </div>
              
              <div class="section">
                  <h3>🔗 MCP Endpoints</h3>
                  <div class="endpoint">https://${this.username}.b00t.promptexecution.com/mcp</div>
                  <p>Use this URL in Claude's Custom Connector configuration.</p>
              </div>
              
              <div class="section">
                  <h3>🔐 OAuth Configuration</h3>
                  <strong>Authorization URL:</strong><br>
                  <div class="endpoint">https://${this.username}.b00t.promptexecution.com/oauth/authorize</div>
                  
                  <strong>Token URL:</strong><br>
                  <div class="endpoint">https://${this.username}.b00t.promptexecution.com/oauth/token</div>
                  
                  <strong>Discovery:</strong><br>
                  <div class="endpoint">https://${this.username}.b00t.promptexecution.com/.well-known/oauth-authorization-server</div>
              </div>
              
              <div class="section">
                  <h3>🛠️ Available Actions</h3>
                  <ul>
                      <li><a href="/auth/github">Login with GitHub</a></li>
                      <li><a href="/mcp">View MCP Capabilities</a> (requires auth)</li>
                      <li><a href="/mcp/tools">List Available Tools</a> (requires auth)</li>
                      <li><a href="/health">Health Check</a></li>
                  </ul>
              </div>
              
              <div class="section">
                  <h3>📋 Integration Instructions</h3>
                  <p>To use this instance with Claude:</p>
                  <ol>
                      <li>In Claude, go to Custom Connectors settings</li>
                      <li>Add a new connector with the MCP endpoint above</li>
                      <li>Configure OAuth using the authorization and token URLs</li>
                      <li>Authenticate with your GitHub account</li>
                  </ol>
              </div>
          </div>
      </body>
      </html>
    `;
  }
}