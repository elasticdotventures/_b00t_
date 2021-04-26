# install Git & Utilities
source "../_b00t_.bashrc"


# sudo apt install -y git-all
sudo apt install -y git

# Set git to use the credential memory cache
git config --global credential.helper cache
git config --global credential.helper 'cache --timeout=3600'
# use vs code for Git commits. 
# https://stackoverflow.com/questions/30024353/how-to-use-visual-studio-code-as-default-editor-for-git

# official PPA for GitHub CLI and update your system packages accordingly.
# 🍰 https://github.com/cli/cli
# 🍰 https://www.techiediaries.com/install-github-cli-ubuntu-20/

## Github CLI
sudo apt-key adv --keyserver keyserver.ubuntu.com --recv-key C99B11DEB97541F0
sudo apt-add-repository https://cli.github.com/packages
sudo apt update -y  # TODO: make -y optional
sudo apt install -y gh

# Git Automation with OAuth Tokens
# https://docs.github.com/en/github/extending-github/git-automation-with-oauth-tokens



# gist
# gist-paste
# sudo apt install -y gist
# gist --login
# TODO: use the oauth
# https://tools.ietf.org/html/rfc6749#section-4.1


