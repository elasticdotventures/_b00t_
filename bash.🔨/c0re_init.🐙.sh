# install Git & Utilities
if [ `type -t "_b00t_init_🥾_开始"` == "function" ]; then 
    # detect _b00t_ environment 
    _b00t_init_🥾_开始
fi

sudo apt install git-all

# official PPA for GitHub CLI and update your system packages accordingly.
# 🍰 https://github.com/cli/cli
# 🍰 https://www.techiediaries.com/install-github-cli-ubuntu-20/

sudo apt-key adv --keyserver keyserver.ubuntu.com --recv-key C99B11DEB97541F0
sudo apt-add-repository https://cli.github.com/packages
sudo apt update
sudo apt install -y gh


