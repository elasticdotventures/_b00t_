import { Env, GitHubUser, GitHubTokenResponse, OAuthState, UserSession } from '../types';
import { 
  generateRandomString, 
  generateErrorResponse,
  createHtmlResponse,
  createJsonResponse,
  isValidGitHubUsername 
} from './utils';
import { SignJWT, jwtVerify } from 'jose';

/**
 * GitHub OAuth integration for b00t-mcp
 */

export async function handleOAuth(request: Request, env: Env, username: string): Promise<Response> {
  const url = new URL(request.url);
  
  // Validate GitHub username format
  if (!isValidGitHubUsername(username)) {
    return generateErrorResponse(400, 'Invalid GitHub username format');
  }
  
  switch (url.pathname) {
    case '/auth/github':
      return handleGitHubLogin(request, env, username);
    case '/auth/github/callback':
      return handleGitHubCallback(request, env, username);
    case '/auth/logout':
      return handleLogout();
    default:
      return generateErrorResponse(404, 'OAuth endpoint not found');
  }
}

/**
 * Initiate GitHub OAuth flow
 */
async function handleGitHubLogin(request: Request, env: Env, username: string): Promise<Response> {
  try {
    const url = new URL(request.url);
    const returnTo = url.searchParams.get('return_to') || '/oauth/consent';
    
    // Generate OAuth state
    const oauthState = generateRandomString(32);
    const stateData: OAuthState = {
      return_url: returnTo,
      created_at: Date.now(),
    };
    
    // Store OAuth state (expires in 10 minutes)
    await env.B00T_SESSIONS.put(
      `oauth_state:${username}:${oauthState}`,
      JSON.stringify(stateData),
      { expirationTtl: 600 }
    );
    
    // Build GitHub authorization URL
    const githubAuthUrl = new URL('https://github.com/login/oauth/authorize');
    githubAuthUrl.searchParams.set('client_id', env.GITHUB_CLIENT_ID);
    githubAuthUrl.searchParams.set('redirect_uri', getRedirectUri(username));
    githubAuthUrl.searchParams.set('scope', 'user:email');
    githubAuthUrl.searchParams.set('state', oauthState);
    
    // Redirect to GitHub
    return Response.redirect(githubAuthUrl.toString(), 302);
    
  } catch (error) {
    console.error('GitHub login error:', error);
    return generateErrorResponse(500, 'Failed to initiate GitHub OAuth');
  }
}

/**
 * Handle GitHub OAuth callback
 */
async function handleGitHubCallback(request: Request, env: Env, username: string): Promise<Response> {
  try {
    const url = new URL(request.url);
    const code = url.searchParams.get('code');
    const state = url.searchParams.get('state');
    const error = url.searchParams.get('error');
    
    // Handle OAuth error
    if (error) {
      return createHtmlResponse(getErrorPage(
        'GitHub OAuth Error',
        `Authentication failed: ${error}`
      ), 400);
    }
    
    // Validate required parameters
    if (!code || !state) {
      return createHtmlResponse(getErrorPage(
        'Invalid OAuth Response',
        'Missing authorization code or state parameter'
      ), 400);
    }
    
    // Retrieve and validate OAuth state
    const stateKey = `oauth_state:${username}:${state}`;
    const stateDataJson = await env.B00T_SESSIONS.get(stateKey);
    if (!stateDataJson) {
      return createHtmlResponse(getErrorPage(
        'Invalid OAuth State',
        'OAuth state expired or invalid. Please try again.'
      ), 400);
    }
    
    const stateData: OAuthState = JSON.parse(stateDataJson);
    
    // Clean up OAuth state
    await env.B00T_SESSIONS.delete(stateKey);
    
    // Exchange code for access token
    const tokenResponse = await fetch('https://github.com/login/oauth/access_token', {
      method: 'POST',
      headers: {
        'Accept': 'application/json',
        'Content-Type': 'application/x-www-form-urlencoded',
        'User-Agent': 'b00t-mcp/1.0',
      },
      body: new URLSearchParams({
        client_id: env.GITHUB_CLIENT_ID,
        client_secret: env.GITHUB_CLIENT_SECRET,
        code,
      }),
    });
    
    if (!tokenResponse.ok) {
      throw new Error(`GitHub token exchange failed: ${tokenResponse.status}`);
    }
    
    const tokenData: GitHubTokenResponse = await tokenResponse.json();
    
    // Get user information from GitHub
    const userResponse = await fetch('https://api.github.com/user', {
      headers: {
        'Authorization': `Bearer ${tokenData.access_token}`,
        'User-Agent': 'b00t-mcp/1.0',
        'Accept': 'application/vnd.github.v3+json',
      },
    });
    
    if (!userResponse.ok) {
      throw new Error(`GitHub user fetch failed: ${userResponse.status}`);
    }
    
    const githubUser: GitHubUser = await userResponse.json();
    
    // Verify the authenticated user matches the requested subdomain
    if (githubUser.login.toLowerCase() !== username.toLowerCase()) {
      return createHtmlResponse(getErrorPage(
        'Authentication Mismatch',
        `You authenticated as "${githubUser.login}" but tried to access "${username}.b00t.promptexecution.com". Please use your own subdomain: https://${githubUser.login}.b00t.promptexecution.com`
      ), 403);
    }
    
    // Create user session
    const sessionId = generateRandomString(48);
    const userSession: UserSession = {
      github_user: githubUser,
      authenticated_at: Date.now(),
      expires_at: Date.now() + (24 * 60 * 60 * 1000), // 24 hours
    };
    
    // Store session (expires in 24 hours)
    await env.B00T_SESSIONS.put(
      `session:${username}:${sessionId}`,
      JSON.stringify(userSession),
      { expirationTtl: 86400 }
    );
    
    // Redirect with session cookie
    const redirectUrl = new URL(stateData.return_url, `https://${username}.b00t.promptexecution.com`);
    redirectUrl.searchParams.set('session', sessionId);
    
    return Response.redirect(redirectUrl.toString(), 302);
    
  } catch (error) {
    console.error('GitHub callback error:', error);
    return createHtmlResponse(getErrorPage(
      'Authentication Error',
      'Failed to complete GitHub authentication. Please try again.'
    ), 500);
  }
}

/**
 * Handle logout
 */
async function handleLogout(): Promise<Response> {
  return createHtmlResponse(`
    <!DOCTYPE html>
    <html>
    <head>
        <title>Logged Out - b00t-mcp</title>
        <style>
            body { font-family: Arial, sans-serif; text-align: center; margin: 40px; }
            .container { max-width: 400px; margin: 0 auto; }
        </style>
    </head>
    <body>
        <div class="container">
            <h2>🥾 Logged Out</h2>
            <p>You have been successfully logged out of b00t-mcp.</p>
            <a href="/auth/github">Login Again</a>
        </div>
    </body>
    </html>
  `);
}

/**
 * Verify user session and return user data
 */
export async function verifyUserSession(
  env: Env, 
  username: string, 
  sessionId: string
): Promise<GitHubUser | null> {
  try {
    const sessionKey = `session:${username}:${sessionId}`;
    const sessionDataJson = await env.B00T_SESSIONS.get(sessionKey);
    
    if (!sessionDataJson) {
      return null;
    }
    
    const sessionData: UserSession = JSON.parse(sessionDataJson);
    
    // Check if session is expired
    if (Date.now() > sessionData.expires_at) {
      // Clean up expired session
      await env.B00T_SESSIONS.delete(sessionKey);
      return null;
    }
    
    return sessionData.github_user;
    
  } catch (error) {
    console.error('Session verification error:', error);
    return null;
  }
}

/**
 * Generate JWT access token for authenticated user
 */
export async function generateAccessToken(
  env: Env,
  githubUser: GitHubUser,
  username: string
): Promise<string> {
  const secretKey = new TextEncoder().encode(env.JWT_SECRET_KEY);
  
  return await new SignJWT({
    sub: `github:${githubUser.login}`,
    aud: `b00t-mcp:${username}`,
    username: githubUser.login,
    name: githubUser.name,
    avatar: githubUser.avatar_url,
  })
    .setProtectedHeader({ alg: 'HS256' })
    .setIssuedAt()
    .setExpirationTime('1h')
    .setIssuer('b00t-mcp-cloudflare')
    .sign(secretKey);
}

/**
 * Verify and decode JWT access token
 */
export async function verifyAccessToken(
  env: Env,
  token: string
): Promise<{ username: string; githubUser: Partial<GitHubUser> } | null> {
  try {
    const secretKey = new TextEncoder().encode(env.JWT_SECRET_KEY);
    const { payload } = await jwtVerify(token, secretKey);
    
    return {
      username: payload.username as string,
      githubUser: {
        login: payload.username as string,
        name: payload.name as string,
        avatar_url: payload.avatar as string,
      },
    };
  } catch (error) {
    console.error('JWT verification error:', error);
    return null;
  }
}

/**
 * Get redirect URI for GitHub OAuth
 */
function getRedirectUri(username: string): string {
  return `https://${username}.b00t.promptexecution.com/auth/github/callback`;
}

/**
 * Generate error page HTML
 */
function getErrorPage(title: string, message: string): string {
  return `
    <!DOCTYPE html>
    <html>
    <head>
        <title>${title} - b00t-mcp</title>
        <style>
            body { 
                font-family: Arial, sans-serif; 
                text-align: center; 
                margin: 40px;
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
            .error { color: #d32f2f; }
            a { color: #1976d2; text-decoration: none; }
            a:hover { text-decoration: underline; }
        </style>
    </head>
    <body>
        <div class="container">
            <h2 class="error">⚠️ ${title}</h2>
            <p>${message}</p>
            <p><a href="/auth/github">Try Again</a></p>
        </div>
    </body>
    </html>
  `;
}