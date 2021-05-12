

# safely initialize _b00t_ bash
source "/c0de/_b00t_/_b00t_.bashrc"

az account set -s NAME_OR_ID

az local-context 

# 怎么样 Get
AZURE_VALID_REGIONS=`$AZ_CLI account list-locations --query '[].[name]' --output table`

# Moved to bicep.sh 
#curl -Lo bicepinstall https://github.com/Azure/bicep/releases/latest/download/bicep-linux-x64
#chmod +x ./bicepinstall
#sudo mv ./bicepinstall /usr/local/bin/bicep
#bicep --help

# NOTE: setting up deploy to azure is easier, but skips the
#       important registry step

# Following tutorials from:
3. https://github.com/marketplace/actions/azure-container-registry-build
2. WRONG https://github.com/marketplace/actions/deploy-to-azure-container-instances
1. OLD >> https://docs.microsoft.com/en-us/azure/container-instances/container-instances-github-action

Stardate: 2020-04-06


```
  
# AZURE_CREDENTIALS --sdk-auth output
{
}
cat > /.env/azure-sdk-auth.json



```
```json output-withhighlights

```
```bash
registryId=$(az acr show \
  --name elasticdotventures.azurecr.io \
  --query id --output tsv)

# $ echo registryId
```
```bash
#  --assignee is clientId
#  NOTE: not sure that this assignee is still provisioned. 
az role assignment create \
  --assignee "X" \
  --scope $registryId \
  --role AcrPush

```
```json
## NOTE: The values below are **HISTORICAL**, please see README-evops
{
  "canDelegate": null,
  "condition": null,
  "conditionVersion": null,
  "description": null,
  "id": "/subscriptions/0bbc8a89-c129-49b5-b5c6-6265b773bf87/resourceGroups/elasticventures-ops/providers/Microsoft.ContainerRegistry/registries/elasticdotventures/providers/Microsoft.Authorization/roleAssignments/596836de-082d-49c8-b74c-01df1d160770",
  "name": "596836de-082d-49c8-b74c-01df1d160770",
  "principalId": "922df98d-4cd1-4969-8664-7a48ebfbb0f5",
  "principalType": "ServicePrincipal",
  "resourceGroup": "elasticventures-ops",
  "roleDefinitionId": "/subscriptions/0bbc8a89-c129-49b5-b5c6-6265b773bf87/providers/Microsoft.Authorization/roleDefinitions/8311e382-0749-4cb8-b61a-304f252e45ec",
  "scope": "/subscriptions/0bbc8a89-c129-49b5-b5c6-6265b773bf87/resourceGroups/elasticventures-ops/providers/Microsoft.ContainerRegistry/registries/elasticdotventures",
  "type": "Microsoft.Authorization/roleAssignments"
}
```


# In GITHUB
github *REPO* elastdicdotventures/sportsworld-as2
settings>secrets

| AZURE_CREDENTIALS 	The entire JSON output from the service principal creation step
see above. the --sdk-auth output

| REGISTRY_LOGIN_SERVER 	The login server name of your registry (all lowercase). Example: myregistry.azurecr.io
elasticdotventures.azurecr.io

| REGISTRY_USERNAME 	The clientId from the JSON output from the service principal creation
<clientId ^^^>

| REGISTRY_PASSWORD 	The clientSecret from the JSON output from the service principal creation
<clientSecret ^^^>


| RESOURCE_GROUP 	The name of the resource group you used to scope the service principal
sportsworldchicago


-----
Service principal - A security identity used by applications or services to access specific Azure resources. You can think of it as a user identity (username and password or certificate) for an application.

Managed identity - An identity in Azure Active Directory that is automatically managed by Azure. You typically use managed identities when developing cloud applications to manage the credentials for authenticating to Azure services.

 sportsworld-serviceUserIdentity
 is *not used* (at this time)
	




#################### STOP HERE 





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


