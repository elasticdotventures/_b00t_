/**
 * Utility functions for b00t-mcp Cloudflare Workers
 */

/**
 * Extract subdomain from hostname
 * Returns the GitHub username portion of the domain
 */
export function extractSubdomain(hostname: string): string | null {
  // Handle localhost development
  if (hostname.includes('localhost') || hostname.includes('127.0.0.1')) {
    return 'dev-local';
  }
  
  // Extract subdomain from *.b00t.promptexecution.com
  const parts = hostname.split('.');
  if (parts.length >= 4 && parts[1] === 'b00t' && parts[2] === 'promptexecution') {
    return parts[0];
  }
  
  return null;
}

/**
 * Generate standardized error response
 */
export function generateErrorResponse(
  status: number,
  message: string,
  details?: Record<string, unknown>
): Response {
  return Response.json(
    {
      error: message,
      status,
      timestamp: new Date().toISOString(),
      ...details,
    },
    { status }
  );
}

/**
 * Generate CORS headers for API responses
 */
export function getCorsHeaders(origin?: string): Record<string, string> {
  return {
    'Access-Control-Allow-Origin': origin || '*',
    'Access-Control-Allow-Methods': 'GET, POST, OPTIONS',
    'Access-Control-Allow-Headers': 'Content-Type, Authorization',
    'Access-Control-Max-Age': '86400',
  };
}

/**
 * Handle CORS preflight requests
 */
export function handleCors(request: Request): Response | null {
  if (request.method === 'OPTIONS') {
    return new Response(null, {
      status: 200,
      headers: getCorsHeaders(request.headers.get('Origin') || undefined),
    });
  }
  return null;
}

/**
 * Generate a secure random string for OAuth state
 */
export function generateRandomString(length: number = 32): string {
  const chars = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';
  let result = '';
  for (let i = 0; i < length; i++) {
    result += chars.charAt(Math.floor(Math.random() * chars.length));
  }
  return result;
}

/**
 * Validate GitHub username format
 */
export function isValidGitHubUsername(username: string): boolean {
  // GitHub username rules:
  // - May only contain alphanumeric characters or hyphens
  // - Cannot have multiple consecutive hyphens
  // - Cannot begin or end with a hyphen
  // - Maximum 39 characters
  const githubUsernameRegex = /^[a-zA-Z0-9]([a-zA-Z0-9-])*[a-zA-Z0-9]$|^[a-zA-Z0-9]$/;
  return githubUsernameRegex.test(username) && username.length <= 39;
}

/**
 * Create HTML response with proper content type
 */
export function createHtmlResponse(html: string, status: number = 200): Response {
  return new Response(html, {
    status,
    headers: {
      'Content-Type': 'text/html; charset=utf-8',
      ...getCorsHeaders(),
    },
  });
}

/**
 * Create JSON response with proper CORS headers
 */
export function createJsonResponse(data: unknown, status: number = 200): Response {
  return Response.json(data, {
    status,
    headers: getCorsHeaders(),
  });
}

/**
 * Parse Authorization header for Bearer token
 */
export function extractBearerToken(authHeader: string | null): string | null {
  if (!authHeader || !authHeader.startsWith('Bearer ')) {
    return null;
  }
  return authHeader.slice(7); // Remove "Bearer " prefix
}

/**
 * Simple rate limiting check using timestamp-based windows
 */
export function checkRateLimit(
  key: string,
  limit: number,
  windowMs: number,
  timestamps: number[]
): { allowed: boolean; remaining: number; resetTime: number } {
  const now = Date.now();
  const windowStart = now - windowMs;
  
  // Filter out expired timestamps
  const validTimestamps = timestamps.filter(ts => ts > windowStart);
  
  const remaining = Math.max(0, limit - validTimestamps.length);
  const resetTime = validTimestamps.length > 0 
    ? validTimestamps[0] + windowMs 
    : now + windowMs;
  
  return {
    allowed: validTimestamps.length < limit,
    remaining,
    resetTime,
  };
}

/**
 * Create base64-encoded basic auth header
 */
export function createBasicAuth(username: string, password: string): string {
  return 'Basic ' + btoa(`${username}:${password}`);
}

/**
 * Format bytes into human readable string
 */
export function formatBytes(bytes: number, decimals: number = 2): string {
  if (bytes === 0) return '0 Bytes';
  
  const k = 1024;
  const dm = decimals < 0 ? 0 : decimals;
  const sizes = ['Bytes', 'KB', 'MB', 'GB'];
  
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  
  return parseFloat((bytes / Math.pow(k, i)).toFixed(dm)) + ' ' + sizes[i];
}