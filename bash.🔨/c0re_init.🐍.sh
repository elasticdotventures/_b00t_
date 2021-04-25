# should be run by _b00t_

if [ `type -t "_b00t_init_🥾_开始"` == "function" ]; then 
    # detect _b00t_ environment 
    _b00t_init_🥾_开始
fi


## * * * *// 
#* 🐍Purpose: b00tstraps python, so we can start using libraries
#* should be called directly from ./01-start.sh 
## * * * *\\

# Python init. 

sudo apt install -y python3-pip
sudo apt install -y python3-venv

python3 -m venv .venv


