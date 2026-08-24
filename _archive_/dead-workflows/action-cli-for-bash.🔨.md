# https://github.com/marketplace/actions/install-action-cli

name: 'action-cli'
on: ["push"]
jobs:
  self-test:
    name: Self test
    runs-on: ubuntu-latest
    steps:
      - uses: zimbatm/action-cli@v0.3.0
      - run: | \
        action-cli warning this a warning
        action-cli get-input HOME
        action-cli
    steps:
      - uses: zimbatm/action-cli@v0.8.0
      - run: action-cli warning --file Cargo.toml --line 2 --col 2 "Ooops"

    # This little tool wraps common tasks that one would do with GitHub Actions and is currently only supported by https://github.com/actions/toolkit/
    # SUBCOMMANDS:
      add-mask         Mask a value in log
      add-path         Add a system path
      debug            Set a debug message
      end-group        End an output group
      error            Set an error message
      export           Like set-env but exports an existing environment variable
      get-input        Gets the value of an input. The value is also trimmed
      get-state        Gets the value of an state set by this action's main execution
      help             Prints this message or the help of the given subcommand(s)
      is-debug         Gets whether Actions Step Debug is on or not
      issue-command    The generic version of the other commands
      post-comment     Creating comment based on issues and pull requests
      save-state       Saves state for current action, the state can only be retrieved by this action's post job
                      execution
      set-env          Set and environment variable for future actions in the job
      set-output       Set an output parameter
      start-group      Begin an output group
      stop-commands    Stop and start log commands
      warning          Set a warning message


name: 'My Container Action'
description: 'Get started with Container actions'
author: 'GitHub'
inputs:
  myInput:
    description: 'Input to use'
    default: 'world'
runs:
  using: 'docker'
  image: 'Dockerfile'
  args:
    - ${{ inputs.myInput }}

