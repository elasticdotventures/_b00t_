
# Project: `copilot-auth-cli` - TODO

## Objective
Create a command-line interface (CLI) tool using TypeScript that programmatically authenticates with the GitHub Copilot LSP, retrieves an auth token, and saves it to `~/.config/gh-copilot/credentials.json`.

---

### Step 1: Project Scaffolding & Setup

- [ ] **Initialize Node.js Project:**
  ```bash
  mkdir copilot-auth-cli
  cd copilot-auth-cli
  pnpm init -y
  ```

- [ ] **Install Dependencies:**
  - [ ] Install TypeScript & Execution Tools:
    ```bash
    pnpm install --save-dev typescript ts-node @types/node
    ```
  - [ ] Install CLI Argument Parser:
    ```bash
    pnpm install commander
    ```

- [ ] **Initialize TypeScript Configuration:**
  - [ ] Run `tsc --init` with recommended settings:
    ```bash
    npx tsc --init --rootDir src --outDir dist --lib es2020 --target es2020 --module commonjs
    ```

- [ ] **Create Project Structure:**
  - [ ] Create the `src` directory.
  - [ ] Create `src/main.ts`.
  - [ ] Create `src/lsp-client.ts`.
  The final structure should look like this:
  ```
  copilot-auth-cli/
  ├── node_modules/
  ├── src/
  │   ├── main.ts
  │   └── lsp-client.ts
  ├── package.json
  └── tsconfig.json
  ```

---

### Step 2: LSP Communication (`src/lsp-client.ts`)

- [ ] **Define JSON-RPC Structures:**
  - [ ] Create a `JsonRpcRequest` interface.
  - [ ] Create a `JsonRpcResponse` interface.
  ```typescript
  // src/lsp-client.ts
  interface JsonRpcRequest {
      jsonrpc: '2.0';
      id: number;
      method: string;
      params: object;
  }

  interface JsonRpcResponse {
      jsonrpc: '2.0';
      id: number;
      result?: any;
      error?: any;
  }
  ```

- [ ] **Create `LspClient` Class:**
  - [ ] **Constructor:** Should accept the path to the `copilot-language-server` executable.
  - [ ] **`start()` method:**
    - [ ] Use Node.js's `child_process.spawn` to launch the language server process.
    - [ ] Attach listeners to the child process's `stdout` to handle incoming data.
  - [ ] **`sendRequest()` method:**
    - [ ] Accept `method` and `params` as arguments.
    - [ ] Construct a valid JSON-RPC request object.
    - [ ] Serialize the request and prepend the `Content-Length` header.
    - [ ] Write the message to the child process's `stdin`.
    - [ ] Return a `Promise` that resolves with the corresponding response from the server.
  - [ ] **`stop()` method:** Implement logic to terminate the language server child process.

---

### Step 3: Main Application Logic (`src/main.ts`)

- [ ] **Implement Imports:**
  - [ ] Import `Command` from `commander`.
  - [ ] Import `LspClient` from `./lsp-client`.
  - [ ] Import necessary Node.js modules (`path`, `os`, `fs/promises`).

- [ ] **Setup CLI with `commander`:**
  - [ ] Define the main command for the tool.
  - [ ] Add a required option `--lsp-path <path>` to specify the location of the `copilot-language-server` executable.

- [ ] **Implement Authentication Flow (`authenticate` function):**
  - [ ] **Instantiate and Start Client:** Create an instance of `LspClient` and call its `start()` method.
  - [ ] **Initialize Connection:** Send an `initialize` request. The Xcode plugin sends a detailed `clientInfo` object. Replicate this. (See *Payload Example 1*)
  - [ ] **Initiate Sign-In:**
    - [ ] Send a `signInInitiate` request with an empty params object. (See *Payload Example 2*)
    - [ ] Extract `userCode` and `verificationUri` from the response.
    - [ ] Print the `verificationUri` and `userCode` to the console for the user.
  - [ ] **Confirm Sign-In (Polling):**
    - [ ] Start a `while` loop to poll for confirmation.
    - [ ] Inside the loop, send a `signInConfirm` request with the `userCode`. (See *Payload Example 3*)
    - [ ] Check the `status` in the response.
    - [ ] If `status` is "OK", the user has successfully authenticated. Break the loop.
    - [ ] If `status` is "NotAuthorized", wait ~5 seconds before the next attempt.
    - [ ] Handle any potential errors and exit gracefully.
  - [ ] **Extract Credentials:** After successful authentication, the response will contain the GitHub `user`. The auth `token` is managed internally by the LSP. For this tool's purpose, we will need to investigate if the LSP provides a way to retrieve the token. If not, we will save the user and a placeholder for the token.

- [ ] **Implement Credential Saving (`saveCredentials` function):**
  - [ ] The function should accept `user` and `token`.
  - [ ] Define the credentials path: `~/.config/gh-copilot/credentials.json`.
  - [ ] Use `fs.mkdir` with `{ recursive: true }` to ensure the directory exists.
  - [ ] Write the credentials to the file in JSON format.

- [ ] **Execute Main Logic:**
  - [ ] Parse CLI arguments using `commander`.
  - [ ] Call the `authenticate` function to start the process.

---

### Step 4: Build & Run Scripts (`package.json`)

- [ ] Add `build` and `start` scripts to `package.json`.
  ```json
  "scripts": {
    "build": "tsc",
    "start": "ts-node src/main.ts"
  }
  ```

---

### Step 5: Finalization

- [ ] Review all code for clarity and error handling.
- [ ] Add JSDoc comments to explain the purpose of major classes and functions.
- [ ] Manually test the complete flow with a valid `copilot-language-server` executable.

---

### Appendix: LSP Payload Examples

*From `GitHubCopilotService.swift` and `GitHubCopilotRequest.swift`.*

**Payload Example 1: `initialize` Request**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "initialize",
  "params": {
    "processId": 12345,
    "clientInfo": {
      "name": "copilot-xcode",
      "version": "0.25.0"
    },
    "capabilities": {}
  }
}
```

**Payload Example 2: `signInInitiate` Request & Response**

*   **Request:**
    ```json
    {
      "jsonrpc": "2.0",
      "id": 2,
      "method": "signInInitiate",
      "params": {}
    }
    ```
*   **Response (`result`):**
    ```json
    {
      "status": "PromptUserDeviceFlow",
      "userCode": "ABCD-EFGH",
      "verificationUri": "https://github.com/login/device"
    }
    ```

**Payload Example 3: `signInConfirm` Request & Response**

*   **Request:**
    ```json
    {
      "jsonrpc": "2.0",
      "id": 3,
      "method": "signInConfirm",
      "params": {
        "userCode": "ABCD-EFGH"
      }
    }
    ```
*   **Response (`result`) on Success:**
    ```json
    {
      "status": "OK",
      "user": "github_username"
    }
    ```
*   **Response (`result`) while pending:**
    ```json
    {
        "status": "NotAuthorized"
    }
    ```
