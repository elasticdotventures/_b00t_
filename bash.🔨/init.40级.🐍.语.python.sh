#* 进口v2 🥾 ALWAYS load c0re Libraries!
source "./_b00t_.bashrc"


## * * * *// 
#* 🐍Purpose: b00tstraps python, so we can start using libraries
#* should be called directly from ./01-start.sh 
## * * * *\\

# Pip requires: 
$SUDO_CMD apt install -y build-essential libssl-dev libffi-dev python3-dev

# Python init. 

$SUDO_CMD apt install -y python3-pip
$SUDO_CMD apt install -y python3-venv

# Establish virtual environemnt
python3 -m venv .venv
source .venv/bin/activate


# TODO: verify tests can be run!




