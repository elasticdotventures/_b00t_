

#* 进口 (Jìnkǒu) 🚀 *ALWAYS* load c0re Libraries!
if [ ! -x "./bash/c0re.🚀.sh" ] ; then
    echo "missing ./bash/c0re.🚀.sh" && exit 
else
    source "./bash/c0re.🚀.sh" 
fi

az account set -s NAME_OR_ID

az local-context 

# 怎么样 Get
AZURE_VALID_REGIONS=`$AZ_CLI account list-locations --query '[].[name]' --output table`


