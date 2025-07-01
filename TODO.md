review README.md and AGENTS.md in the root of the repo.
we are modifying the _b00t_ .dotfiles repo, specifically to add new functionality.

[X] Today is a major refactor - progress towards version 1.2
[X] We want to introduce a new composable pattern for installation & version checking of applications.

[X] for example in the past 24 hours two cli's have been found to be 'out of date' the gh cli & the just command runner.

[X] ultimately we will migrate all commands to the new composable format

[X] we will be introducing a rust xtask command called _b00t_, the concept being a generic version manager
(similar to nvm, pyenv, etc) but for any command line tool.

[X] for now we will only be working with just & gh,
but the goal is to have a generic solution that can be extended to any command line tool.

[X] we will create a toml file for each command stored in ~/.dotfiles/_b00t_  ex: just.toml, gh.toml, etc.

[X] we will extract the installation command so that it uses a subcommands,
each _b00t_ subcommand has an alias(es) with :shortcode: and emoji to tell a visual story.
the ability to specify the --path to find .toml files should be configurable (ex: ~/.dotfiles/_b00t_ is default)

each of those will:
* [X] read the just.toml in the _b00t_ subdir (default ~/.dotfiles/_b00t_)
* [X] run the version command (specified in the toml, or default to --version)

_b00t_ detect just
_b00t_ detect gh
* [X] :detective: or detective emoji
* [X] outputs the version of the command if it exists (the version command is specified in the toml file)

#
_b00t_ desires just
* [X] wants is an alias for ':heart:' or heart emoji
* [X] will output the version desired by the toml file

_b00t_ install just
* [X] will run the install command in the specified file

_b00t_ update just
* [X] will check for a separate update stanza
* [X] if it exists, run the update command
* [X] if it does not exist, run the install command

#
_b00t_ . just
* [X] will run desires & just, comparing the two versions
* [X] if the versions are the same, it's a noop, it outputs "🥾👍🏻 $command" && exits with a 0
* [X] if the installed is newer it outputs "🥾🐣 $command $version" exits with a 0
* [X] if the installed is older it outputs "🥾😭 $command IS $version WANTS $desire_version" and exits with 1
* [X] if the command is not installed it outputs "🥾😱 $command MISSING" and exits with 2
* [X] if b00t is requested to check a command that doesn't have a toml file output $command UNDEFINED and exit 100

# the magic operator
[X] there should be a bash function added to:
    ~/.dotfiles/_b00t_/_b00t_.bashrc
[X] it should list all .toml files in the ~/.dotfiles/_b00t_ directory

[X] iterate over each file, run the . b00t_ command with the file name as an argument

[X] if there is more than one file which is out of date suggest running
_b00t_ up

[X] which should install / update as necessary all the commands managed by _b00t_

# New Tasks
[X] Add b00t.toml to allow b00t to upgrade itself.
[X] Add a --version flag to the b00t CLI.


# just.toml – b00t module definition for `just`
```

[b00t]
name = "just"
desires = "1.25.2"
install = '''
curl -sSL https://just.systems/install.sh | bash -s -- --to ~/.local/bin
'''
update = '''
# can be omitted if the install command is same
curl -sSL https://just.systems/install.sh | bash -s -- --to ~/.local/bin
'''
version = "just --version"
version_regex = "\\d+\\.\\d+\\.\\d+"
hint = "just is a command runner that abstracts cli tools for a repo without reading the docs. run `just -l` to list commands."


```

---

## Architectural Plan for _b00t_ Refactor (v1.2)

This plan outlines the steps to introduce a new composable pattern for installation and version checking of applications using a Rust `xtask` command called `_b00t_`.

### Streamlined Architecture Diagram

```mermaid
graph TD
    A[User / Shell] --> B{_b00t_ Bash Function}
    B -- Reads .toml files --> C[~/.dotfiles/_b00t_/*.toml]
    C -- Configures --> D[_b00t_ Rust xtask CLI]
    D -- Interacts with --> E[External CLI Tools (e.g., just, gh)]
    D -- Provides Status / Actions --> B
    B -- Suggests Batch Update --> F[_b00t_ up]
    F --> D
```

### Detailed Plan Steps

1.  **[X] Prepare Project Structure:**
    *   [X] Create the `_b00t_` directory for TOML files: `~/.dotfiles/_b00t_`.
    *   [X] Create the `just.toml` and `gh.toml` files within `~/.dotfiles/_b00t_` based on the provided template.
    *   [X] Create the Rust project structure for `_b00t_` (in `~/.dotfiles/boot-cli`).

2.  **[X] Architect `_b00t_` Rust `xtask` Command:**
    *   [X] Define the `xtask` structure and entry point.
    *   [X] Implement the TOML parsing logic.
    *   [X] Implement each subcommand (`detect`, `desires`, `install`, `update`, `.`, `up`).
    *   [X] Handle version comparison and exit codes.
    *   [X] Incorporate emoji and shortcode aliases.

3.  **[X] Architect Bash Integration:**
    *   [X] Define the structure of the bash function that iterates over TOML files.
    *   [X] Outline the logic for suggesting `_b00t_ up`.

4.  **[_] Design Test Cases (Test-Driven Development):**
    *   For each `_b00t_` subcommand, define unit and integration tests.
    *   Focus on testing:
        *   TOML parsing correctness.
        *   Version detection (correct output, handling missing commands).
        *   Desired version retrieval.
        *   Install/update command execution (mocking external commands).
        *   Comparison logic and exit codes.
        *   Error handling for invalid TOML or missing commands.

5.  **[_] Architect Validation Steps:**
    *   List manual and automated validation steps for the entire solution.

6.  **[X] Update `TODO.md`:**
    *   Add a section to `TODO.md` with the detailed plan and validation steps.

### Validation Steps

1.  **[_] Unit Tests:**
    *   Verify TOML parsing for all fields, including optional `update`.
    *   Test `detect` logic: correct version extraction, handling of non-existent commands, and commands with non-standard `--version` output.
    *   Test `desires` logic: correct version retrieval from TOML.
    *   Test `install` and `update` logic: ensure correct external commands are constructed and executed (mock external calls).
    *   Test `.` command logic:
        *   Installed == Desired (exit 0, correct emoji/message).
        *   Installed > Desired (exit 0, correct emoji/message).
        *   Installed < Desired (exit 1, correct emoji/message).
        *   Command Missing (exit 2, correct emoji/message).
    *   Test edge cases for version regex (e.g., multiple matches, no matches).

2.  **[_] Integration Tests (using `bats` or similar for shell scripting):**
    *   Create a temporary `~/.dotfiles/_b00t_` directory with mock `just.toml` and `gh.toml`.
    *   Test `_b00t_ detect just` and `_b00t_ detect gh` with various installed versions (mock `just --version` and `gh --version`).
    *   Test `_b00t_ desires just` and `_b00t_ desires gh`.
    *   Test `_b00t_ install just` and `_b00t_ install gh` (verify execution of install script).
    *   Test `_b00t_ update just` and `_b00t_ update gh` (verify update vs. install logic).
    *   Test `_b00t_ . just` and `_b00t_ . gh` for all four exit code scenarios.
    *   Test the main bash function:
        *   Verify it iterates over all `.toml` files.
        *   Verify it correctly identifies out-of-date commands.
        *   Verify it suggests `_b00t_ up` when appropriate.
    *   Test `_b00t_ up` command: verify it correctly installs/updates all commands.

3.  **[X] Manual Testing:**
    *   [X] Manually create `just.toml` and `gh.toml` with different desired versions.
    *   [X] Run `_b00t_ detect just`, `_b00t_ desires just`, `_b00t_ . just` and observe output and exit codes.
    *   [X] Manually install/uninstall `just` and `gh` to test missing/present scenarios.
    *   [X] Run the bash function and observe its output and suggestions.
    *   [X] Run `_b00t_ up` and verify successful installation/update.
    *   [X] Test with a non-existent TOML file.
    *   [X] Test with a malformed TOML file.

```