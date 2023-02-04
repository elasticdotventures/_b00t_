

* MSAL   "Microsoft Authentication Libraries"
https://docs.microsoft.com/en-us/azure/active-directory/develop/msal-overview
* SPA app
* Web app
* Mobile & Desktop
* Daemon or back-end service

* No need to directly use the OAuth libraries or code against the protocol in your application.
* Acquires tokens on behalf of a user or on behalf of an application (when applicable to the platform).
* Maintains a token cache and refreshes tokens for you when they are close to expire. You don't need to handle token expiration on your own.
* Helps you specify which audience you want your application to sign in (your org, several orgs, work, and school and Microsoft personal accounts, social identities with Azure AD B2C, users in sovereign, and national clouds)*  Helps you set up your application from configuration files.
* Helps you troubleshoot your app by exposing actionable exceptions, logging, and telemetry.

# GITHUB 
https://docs.github.com/en/developers/apps/building-oauth-apps/authorizing-oauth-apps

Authorization Callback url convention:
/L0gg1N  (*not /login)



SAML - single sign on, it's difficult, obscure
can be access with Okta
developer.okta.com (free account)

## 🤓 https://www.youtube.com/watch?v=996OiexHze0

Don't use OAuth for authentication (AuthN), only authorization (AuthZ).
Using OAuth for authentication is bad, bceause:
* OAuth isn't concerned with authentication
* no standard way to get users information
* Every implementation is a little different
* No common set of scopes

OpenID Connect is an extension to OAuth, makes
OAuth good for Authentication by adding:
* ID token - represents information about the user
* UserInfo endpoint - ubiquitos in OpenID, 
   * for getting more info about User, i.e. birthday
* Standardized set of scopes
* Standardized implementation
* it's simply an OAuth scope
```
Redirect URI: yelp.com/callback
Response type: code
Scope: openid profile 
```
All OpenId requests are oauth requests, but not all
OAuth request are OpenID. 

```mermaid
TODO - diagram. 
CLIENT -> TOKEN -> API

```

JWT - json web token
* standard way of transmitting
* jsonwebtoken.io
HEADER - purpose unknown. 
PAYLOAD - json (shown below), called "CLAIM" ?
SIGNATURE - verifies
# the "ID" token 
{
    "iss":"https://accounts.google.com",
    "sub": "you@gmail.com"
    "name": "brian .."
    "aud": "s6bhddd"
    "exp": epochts,
    "iat": epochts,
    "auth_time": epochts
}


example.com -> 
    OpenID Connect (code flow) -> 
        google.com (login) ->
            example.com (return) ->
                Set-Cookie: sessionid=f00b4r

example native mobile app ->
    OpenID connect (code flow + PKCE) ->
        google.com (login) ->
            return to app with code grant, exchanged for ID token & access token ->
                Store tokens in protected device storage
                Use ID token to know how the user is
                Attach access token to outcoming API requests
                ** Use AppAuth library for native apps
                ** AppAuth compatible with Azure AD
                ** 




## More on OAuth 
# https://oauthdebugger.com/


oauth2.0 Terms and Meaning

* resource owner : who owns the data
* client  : application, i.e. yelp
* authorization server : accounts.google.com
* resource server : system which holds the data, contacts.google.com
* authorization grant : the thing that proves the user has given consent
    * OAuth 2.0 flow types:  
    *  authorization "code" grant, via "code flow", uses
        front-channel + back-channel
    *  implicit (front channel only)
        * used ONLY for single page 'static' apps
        * Redirect URI: yelp.com/feedback
        * Response type: token
        * Scope: profile contacts 
    * Resource owner password credentials (back channel only)
        * legacy compatibility with older applications,
          other flows, not recommended for new starts. 
    * Client credentials (back channel only)
        * machine to machine api
* redirect uri : callback url, where does the user end up.
* access token : lets the client do what it needs to do.
    * also called "Bearer Token" because they appear like:
```
    GET api.google.com/some/endpoint
    Authorization: Bearer $access_token
```

* scope:
    authorization server has list of scopes
    Unique to the authorization server
    i.e. ReadContacts or ReadMessages

# "Authorization Code Flow" 
* back-channel "most secure"
    * client secret: private, issued during oauth enrollment (scope registration)

* front-channel "least secure"
    * client id: public, issued during oauth enrollment (scope registration)

