/bin/bash -c "$(curl -fsSL [https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh](https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh))"
eval "$(/home/linuxbrew/.linuxbrew/bin/brew shellenv)"

brew tap ankitpokhrel/jira-cli
brew install jira-cli

docker run -e EDITOR -e JIRA_API_TOKEN -it --rm ghcr.io/ankitpokhrel/jira-cli:latest
