

# this might need to move to python. 


## * * * * \\
# Get filename and --parameters
filename=$1
while getopts ":g:i:n:m:e:" arg; do
  case $arg in
    g) resourceGroup=$OPTARG;;
    i) ID=$OPTARG;;
    n) Name=$OPTARG;;
    m) Manufacturing_date=$OPTARG;;
    e) Expire_date=$OPTARG;;
  esac
done
echo -e "\n$ID $Name\n"
# ./bash-cmd.新.sh -g -i p001 -n 'Hot Cake' -m '01-01-2018' -e '06-01-2018'
## * * * * //

## make a cifs share
docker volume create \
	--driver local \
	--opt type=cifs \
	--opt device=//uxxxxx.your-server.de/backup \
	--opt o=addr=uxxxxx.your-server.de,username=uxxxxxxx,password=*****,file_mode=0777,dir_mode=0777 \
	--name cif-volume

  
git remote add template https://github.com/elasticdotventures/_b00t_
git fetch template