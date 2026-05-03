#!/bin/bash

# test for docker alias to podman, if it is then act isn't supported
# https://github.com/nektos/act/issues/303
if command -v podman &> /dev/null || alias docker | grep -q "podman"; then
  echo "😭 podman is installed or docker is aliased to podman, act is not supported with podman."
  exit 1
fi

# Check for required dependencies
for cmd in fzf gh sed docker; do
  if ! command -v "$cmd" &> /dev/null; then
    echo "$cmd could not be found. Please install $cmd first."
    exit 1
  fi
done

# Check if the 'act' extension for the 'gh' CLI is installed
if ! gh extension list | grep -q "gh-act"; then
  echo "gh cli extension 'act' is not installed. Please install it using 'gh extension install nektos/gh-act'."
  exit 1
fi

if [ -n "$1" ]; then
  workflow="$1"
  # Check if workflow file exists
  if [ ! -f "$workflow" ]; then
    echo "Workflow file not found: $workflow"
    exit 1
  fi
else
  # List available workflows using fzf
  workflow=$(find .github/workflows -type f -regex ".*\.\(yml\|yaml\)$" | sort | fzf --prompt="Select a GitHub workflow: ")
fi



# Exit if no workflow is selected
if [ -z "$workflow" ]; then
  echo "No workflow selected. Exiting."
  exit 1
fi

# handling for the job argument: $2
if [ -n "$2" ]; then
  job="$2"
else
  # List available jobs using yq, strip the '-' and prompt user to select one
  job=$(yq e '.jobs | keys' "$workflow" | sed 's/- //' | fzf --prompt="Select a job to run: ")
fi


# Extract the first job name from the workflow file or use a default
if [ -z "$job" ]; then
  echo "No job found in the selected workflow. Using default job name 'build'."
  job="build"  # Default job name if none is found
fi

# https://nektosact.com/missing_functionality/docker_context.html
export DOCKER_HOST=$(docker context inspect --format '{{.Endpoints.docker.Host}}')
# export DOCKER_CERT_PATH=$(docker context inspect --format '{{.Storage.TLSPath}}')/docker

# Run the selected workflow using act
echo "Running workflow: $workflow with job: $job"

export SSH_PRIVATE_KEY=$(cat ~/.ssh/id_ed25519) &&
  dotenv && \
  gh act push \
    -j $job \
    -W $workflow \
    -P test-xavier=catthehacker/ubuntu:act-latest \
    -s SSH_PRIVATE_KEY \
    -s GITHUB_TOKEN=$(gh auth token) \
    --env-file .env \
    --container-architecture=linux/amd64 \
    --action-offline-mode \
    -e act-event.json

# -e act-event.json
#  You cannot use the env context in job level if conditions, but you can add a custom event property to the github context. You can use this method also on step level if conditions
#  if: ${{ !github.event.act }} # skip during local actions testing

# --action-offline-mode
# If you want to speed up running act and using cached actions and container images you can enable this mode.
# stops pulling existing images
# stops failing if an action has been cached and you cannot connect to GitHub
# pulls non existent actions and images
# act will work offline if it has at least ran once while you are online
# get rid of unnecessary timeouts when you have an unstable connection to GitHub or Container registries workaround rate limit problems


echo " 🤓 hint: just watch-gh-action $workflow '$job'"