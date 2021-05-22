

# safely initialize _b00t_ bash
source "$_B00T_C0DE_Path/_b00t_.bashrc"
source "./b00t-extra.bashrc"
if is_n0t_aliased "az" ; then
  log_📢_记录 "🥵 crashed, az-cli alias 'az' is required"
  exit  
fi


#az login
#az login --use-device-code

## 项目 * * * * \\  
# (Xiàngmù) Project Id
if [ -z "$_Pr0J3ct1D" ] ; then 
  export _Pr0J3ct1D=$(crudini_get "b00t" "_Pr0J3ct1D")
fi
if [ -z "$_Pr0J3ct1D" ] ; then 
  export _Pr0J3ct1D=$(Pr0J3ct1D)
  crudini_set "b00t" "_Pr0J3ct1D" "$_Pr0J3ct1D"
fi
log_📢_记录 "🥾 ProjectID: $_Pr0J3ct1D"

AZURE_LOCATION_ID=$( crudini_get "AZURE" "LOCATION_ID" )
if [ -z "$AZURE_LOCATION_ID" ] ; then
  log_📢_记录 "💙🤖🤓: Please choose a location"
  export AZURE_LOCATION_ID=$( az account list-locations -o json | jq -c --raw-output '.[]|[.name,.displayName] | @tsv' | sort | fzf-tmux --delimiter='\t' --with-nth=1 --preview='echo {2}' --height 40% | awk '{print $1}' )
  crudini_set "AZURE" "LOCATION_ID" $AZURE_LOCATION_ID
fi
log_📢_记录 "💙🤖 Location: $AZURE_LOCATION_ID"


if [ -z "$AZ_RESOURCE_GROUP" ] ; then 
  log_📢_记录 "👽: sorry, you need AZ_RESOURCE_GROUP"
  fzf 
  exit
fi

if [ $(az account list -o json | jq '. | length') -eq 1 ] ; then
    log_📢_记录 "found one account"
else
     log_📢_记录 "🍒 sorry, multi-account not supported (YET)."
fi


export AZURE_ACCOUNT_ID=$( az account list -o json | jq '.[0].id' )
export AZURE_ACCOUNT_NAME=$( az account list -o json | jq '.[0].name' )
export AZURE_TENANT_ID=$( az account list -o json | jq '.[0].tenantId' )
export AZURE_USERNAME=$( az account list -o json | jq '.[0].user.name' )
export AZURE_USERTYPE=$( az account list -o json | jq '.[0].user.type' )


## !TODO: Do you need a project name?
## !TODO: Do we have an AZ tenant Id?
## 要不要　
## !TODO: Do you need a resource Group?
## !TODO: 

##* * * * \\
az_resGroupId=$(az group show --name $az_groupName --query id --output tsv)
# $echo groupId
# /subscriptions/{###}/resourceGroups/{groupName}
az ad sp create-for-rbac \
  # --scopes  # !TODO
  --scope $az_resGroupId --role Contributor \
  --name $az_projectId-🤴校长_principal \
  --sdk-auth
##* * * * //







if [ false ] ; then
    az login

    export AZ_LOCATION=southeastasia
    export AZ_RESOURCE_GROUP="r41nm4k3r.pr0t0sG0里面"
    az group create --location $AZ_LOCATION --resource-group $AZ_RESOURCE_GROUP

    # https://whatibroke.com/2021/03/27/quick-sample-to-create-a-vm-azure-bicep/ 
    az deployment group create --template-file ./main.bicep  --parameters ./parameters/parameters.prod.json -g "$AZ_RESOURCE_GROUP"

# AI
# in azure speak there are 4 types of NVIDIA Image for AI
# az extension add --name azure-devops
# az devops configure --defaults organization=https://elastic.ventures/r41nm4k3r project=r41nm4k3r

## Az Devops guide:
## 
#$ az devops -h
#   
#Group
#    az devops : Manage Azure DevOps organization level operations.
#        Related Groups
#        az pipelines: Manage Azure Pipelines
#        az boards: Manage Azure Boards
#        az repos: Manage Azure Repos
#        az artifacts: Manage Azure Artifacts.
#   
#Subgroups:
#    admin            : Manage administration operations.
#    extension        : Manage extensions.
#    project          : Manage team projects.
#    security         : Manage security related operations.
#    service-endpoint : Manage service endpoints/service connections.
#    team             : Manage teams.
#    user             : Manage users.
#    wiki             : Manage wikis.
#Commands:
#    configure        : Configure the Azure DevOps CLI or view your configuration.
#    feedback         : Displays information on how to provide feedback to the Azure DevOps CLI team.
#    invoke           : This command will invoke request for any DevOps area and resource. Please use
#                       only json output as the response of this command is not fixed. Helpful docs -
#                       https://docs.microsoft.com/rest/api/azure/devops/.
#    login            : Set the credential (PAT) to use for a particular organization.
#    logout           : Clear the credential for all or a particular organization.

## NOTE: for "can't build rome in a day reasons" -- this is a mess, since it's layers inside of layers. 
## iv'e presently configured the r41nm4k3r dev environment
#mkdir azagent;cd azagent;curl -fkSL -o vstsagent.tar.gz 
# https://vstsagentpackage.azureedge.net/agent/2.186.1/vsts-agent-linux-x64-2.186.1.tar.gz;
# tar -zxvf vstsagent.tar.gz; if [ -x "$(command -v systemctl)" ]; 
# then ./config.sh --environment --environmentname "r41nm4k3r-pr0t0typ0s-sm311s11k3s01d3r" --acceptteeeula --agent $HOSTNAME --url https://dev.azure.com/elasticdotventures/ --work _work --projectname 'r41nm4k3r-b00t-图形蛇' --auth PAT --token zalrpp4xivpxopfc5dxamphlyt3eb44xean5vnpulumrmqtbtqqq --runasservice; sudo ./svc.sh install; sudo ./svc.sh start; else ./config.sh --environment --environmentname "r41nm4k3r-pr0t0typ0s-sm311s11k3s01d3r" --acceptteeeula --agent $HOSTNAME --url https://dev.azure.com/elasticdotventures/ --work _work --projectname 'r41nm4k3r-b00t-图形蛇' --auth PAT --token zalrpp4xivpxopfc5dxamphlyt3eb44xean5vnpulumrmqtbtqqq; ./run.sh; fi

#ssh -i r41nm4k3r--pr0t0typ0x--b00t_key.pem azureuser@r41nm4k3r--pr0t0typ0x--b00t.australiasoutheast.cloudapp.azure.com
#ssh -i r41nm4k3r--pr0t0typ0x--b00t_key.pem w1ndy@r41nm4k3r-nvidia.southeastasia.cloudapp.azure.com


    # mount _b00t_ in the d0cker? 
fi



