

# safely initialize _b00t_ bash
source "/c0de/_b00t_/_b00t_.bashrc"

az account set -s NAME_OR_ID

az local-context 

# 怎么样 Get
AZURE_VALID_REGIONS=`$AZ_CLI account list-locations --query '[].[name]' --output table`

curl -Lo bicepinstall https://github.com/Azure/bicep/releases/latest/download/bicep-linux-x64
chmod +x ./bicepinstall
sudo mv ./bicepinstall /usr/local/bin/bicep
bicep --help


##* * * * * * * *//
#* 👾 Azure parameters: 
##* * * * * * * *\\
#while getopts ":g:rg:location:AZ_location:" arg; do
#  case $arg in
#    g) AZ_resourceGroup=$OPTARG;;
#    rg) AZ_resourceGroup=$OPTARG;;
#    location) AZ_location=$OPTARG;;
#    AZ_location) AZ_location=$OPTARG;;
#  esac
#done


##* * * * * * * *//
#* 👾 $AZ_resourceGroup
##* * * * * * * *\\
if [ -n "$1" ] ; then
    AZ_resourceGroup=$1
elif [ -z "$AZ_resourceGroup" ] ; then
    echo "please designate \$AZ_resourceGroup using -rg parameter"
    exit 0
fi 

echo "AZ_resourceGroup: $AZ_resourceGroup"
export AZ_resourceGroup




##* * * * * * * *//
#* 👾 $AZ_location
##* * * * * * * *\\
if [ -z "$AZ_location"] ; then
  # 🤖 default AZ region
  # Valid List:
  # $AZ_CLI account list-locations --query '[].[name]' --output table
  AZ_location="australiasoutheast"
fi
export AZ_location
echo "AZ_location: $AZ_location"


