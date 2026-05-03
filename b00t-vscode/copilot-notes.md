 │    **Processed Files:**                                              │
 │    -                                                                 │
 │    `Core/Sources/GitHubCopilotViewModel/GitHubCopilotView            │
 │    Model.swift`                                                      │
 │    -                                                                 │
 │    `Tool/Sources/GitHubCopilotService/LanguageServer/GitH            │
 │    ubCopilotRequest.swift`                                           │
 │    -                                                                 │
 │    `Tool/Sources/GitHubCopilotService/LanguageServer/GitH            │
 │    ubCopilotService.swift`                                           │
 │    - `Tool/Sources/Status/Status.swift`                              │
 ╰──────────────────────────────────────────────────────────────────────╯
✦  - Initiation: User clicks "Sign In". GitHubCopilotViewModel.signIn()
     is called.
   - Device Flow: signIn() calls preSignIn(), which in turn calls
     GitHubCopilotService.signInInitiate(). This sends a SignInInitiate
     request to the Copilot language server.
   - User-Side Auth: The server returns a userCode and verificationURL.
     The user opens the URL and enters the code to authorize the app on
     GitHub.
   - Confirmation: The app calls
     GitHubCopilotService.signInConfirm(userCode:), sending a
     SignInConfirm request. A successful response contains the username
     and auth token.
   - State Management: Status.shared.updateAuthStatus() is called to
     globally reflect the logged-in state. The token is stored for
     subsequent requests.
   - Chat Access: Authenticated requests to the chat endpoint
     (CreateConversation, CreateTurn) are made through
     GitHubCopilotService. The service sends these requests to the
     language server, which proxies them to the GitHub Copilot API,
     adding the necessary authentication token to the headers.

