
# Fzf notes

EXCELLENT FZF DOCS:
https://www.mankier.com/1/fzf
https://github.com/junegunn/fzf/wiki/examples
https://seb.jambor.dev/posts/improving-shell-workflows-with-fzf/


fzf --multi
(use tab to select)

# can be combined with whiptail
whiptail --title "Check list example" --checklist \
"Choose user's permissions" 20 78 4 \
"NET_OUTBOUND" "Allow connections to other hosts" ON \
"NET_INBOUND" "Allow connections from other hosts" OFF \
"LOCAL_MOUNT" "Allow mounting of local devices" OFF \
"REMOTE_MOUNT" "Allow mounting of remote devices" OFF

sudo apt install -y bat


# Good fzf examples for docker: 
https://github.com/junegunn/fzf/wiki/examples

# https://github.com/b4b4r07/emoji-cli


# some good FZF examples in _b00t_.bashrc and sample bash file.