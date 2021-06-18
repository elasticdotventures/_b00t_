 # _b00t_ docker clean/tidy
 

# https://forums.docker.com/t/how-to-remove-none-images-after-building/7050
d1ngL3=`docker images -f "dangling=true" -q | wc -l`
if [ "$d1ngL3_本" -gt 0 ] ; then
    log_📢_记录 "🐳 d1ngL3_本 is:$d1ngL3_本"

    # clean all
    docker image rm `docker images -f "dangling=true" -q`

    # clean-selective with fzf
    docker image rm `docker images -f "dangling=true" -q | fzf`

fi 