

## Windows Terminal has the best Emoji support, but doesn't
support x-forwarding? 

# https://unix.stackexchange.com/questions/207365/x-flag-x11-forwarding-does-not-appear-to-work-in-windows
export DISPLAY=127.0.0.1:0; 
ssh -X brianh@192.168.0.137 'cd /home/guest && exec bash -l'

# virtual screens:
WIN+tab 

