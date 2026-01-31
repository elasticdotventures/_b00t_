## * * * *// 
#* Purpose: ☕ b00tstraps rust
#* should be called directly from ./01-start.sh 
## * * * *\\

#* 进口v2 🥾 ALWAYS load c0re Libraries!
source "./_b00t_.bashrc"

log_📢_记录 "🚀 install rust"

curl https://sh.rustup.rs -sSf | sh
source $HOME/.cargo/env
# curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

