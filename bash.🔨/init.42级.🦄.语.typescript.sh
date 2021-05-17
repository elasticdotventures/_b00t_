## * * * *// 
#* Purpose: 🦄 b00tstraps node & typescript
#* should be called directly from ./01-start.sh 
## * * * *\\

#* 进口v2 🥾 ALWAYS load c0re Libraries!
source "./_b00t_.bashrc"

# once we setup npm
# https://github.com/google/zx

npm i -g zx
# curl -sL https://deb.nodesource.com/setup_14.x | sudo -E bash -
# https://linuxize.com/post/how-to-install-node-js-on-ubuntu-20-04/#installing-nodejs-and-npm-from-nodesource


# YARN Node package manager
# 🤓 https://engineering.fb.com/2016/10/11/web/yarn-a-new-package-manager-for-javascript/
# yarn add create
if [ ! -x "/usr/local/bin/yarn" ]; then 
    $SUDO_CMD npm install -g yarn
    # yarn add $project
    # npm init @vitejs/app <project-name>
    # yarn create @vitejs/app <project-name>
fi 

# Docker SDK
yarn add @docker/sdk

# install dependencies as you code
# https://github.com/siddharthkp/auto-install

# 
# yarn create @vitejs/app _b00t_

# Three examples of PolyMorphism:
# https://blog.sessionstack.com/how-javascript-works-3-types-of-polymorphism-f10ff4992be1?source=collection_category---4------0-----------------------&gi=d6058f27a8e3

# ShellJS a portable unix shell commands for node.js
# https://github.com/shelljs/shelljs

# future.

# log_📢_记录 "🚀 install node"
#sudo snap install node --classic

#apt-get install npm 
#npm i -D foy

## Yeoman is awesome. Going to use this: 
#npm install -g yo generator-code
