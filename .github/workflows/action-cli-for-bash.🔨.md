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
      - uses: zimbatm/action-cli@v0.3.0
      - run: action-cli warning --file Cargo.toml --line 2 --col 2 "Ooops"


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

