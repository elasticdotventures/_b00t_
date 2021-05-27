
# 🤖 Azure

## Map all Azure SDK Releases (all languages, functionality)
https://azure.github.io/azure-sdk/releases/latest/index.html#python

# Azure Authentication
https://devblogs.microsoft.com/azure-sdk/authentication-and-the-azure-sdk/

# 🤓 Advanced Use cases with Azure Active Directory B2C  (4:94m)
https://www.youtube.com/watch?v=-ZmPBuMZY-Y

# 🤓 Setup a sign in page for single-page app using Azure Active Directory
https://docs.microsoft.com/en-us/azure/active-directory-b2c/quickstart-single-page-app

## Azure templates (bicep & ARM)
https://docs.microsoft.com/en-us/azure/templates/microsoft.containerinstance/containergroups?tabs=json#LogAnalytics

## Automate container image builds and maintenance with ACR tasks
https://docs.microsoft.com/en-us/azure/container-registry/container-registry-tasks-overview

# 
az deployment group create -f ./main.bicep -g my-rg --parameters location=westus storageAccountName=uniquelogstorage001




# Azure Bicep 
# the -c parameter tells it to "Check" (safe), preview changes
az deployment group create -f ./main.bicep -g sportsworldas2-rg

^^^ so the bicep file builds the resource definition allowing
    a deployment of:
    * docker container 
    * database 
    FUTURE:
    * azure storage


## the following is a log of the actions taken
[README-github.md] Github, CI/CD notes
...bicep? 
[README-docker.md] Docker, build notes
[README-django.md] Django app, firebirddb & config nodes


## QuickStart Templates
https://github.com/Azure/azure-quickstart-templates

# MSAL Key Concepts
DefaultAzureCredential is best, uses a sequental chain of:
* Environment, * Managed Identity, VS Code, Azure CLI, Interactive

* Managed Identity:
if the application is deployed to an Azure host with Managed Identity enabled


# more fun:
https://github.com/microsoft/botframework-sdk


### IGNORE THIS:
az account set -s NAME_OR_ID

# 怎么样 Get
AZURE_VALID_REGIONS=`$AZ_CLI account list-locations --query '[].[name]' --output table`


# NOTE: setting up deploy to azure is easier, but skips the
#       important registry step

# Following tutorials from:
#3. https://github.com/marketplace/actions/azure-container-registry-build
#2. WRONG https://github.com/marketplace/actions/deploy-to-azure-container-instances
#1. OLD >> https://docs.microsoft.com/en-us/azure/container-instances/container-instances-github-action

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


| AZURE_CREDENTIALS 	The entire JSON output from the service principal creation step
see above. the --sdk-auth output

| REGISTRY_LOGIN_SERVER 	The login server name of your registry (all lowercase). Example: myregistry.azurecr.io
elasticdotventures.azurecr.io

| REGISTRY_USERNAME 	The clientId from the JSON output from the service principal creation
<clientId ^^^>

| REGISTRY_PASSWORD 	The clientSecret from the JSON output from the service principal creation
<clientSecret ^^^>


| RESOURCE_GROUP 	The name of the resource group you used to scope the service principal
$_Pr0J3ctID


-----
Service principal - A security identity used by applications or services to access specific Azure resources. You can think of it as a user identity (username and password or certificate) for an application.

Managed identity - An identity in Azure Active Directory that is automatically managed by Azure. You typically use managed identities when developing cloud applications to manage the credentials for authenticating to Azure services.



#!/bin/bash
source "/c0de/_b00t_/_b00t_.bashrc"

## HISTORY: a lot of the need for this file went away once az-cli was moved to docker, some is kept here. 

#function install_b1c3p() {
#    curl -Lo bicepinstall https://github.com/Azure/bicep/releases/latest/download/bicep-linux-x64
#    chmod +x ./bicepinstall
#    $SUDO_CMD mv ./bicepinstall /usr/local/bin/bicep
#    bicep --help
#}

#if n0ta_xfile_📁_好不好 "/usr/local/bin/bicep" ; then
#    log_📢_记录 "🤓 installing bicep. "
#    install_b1c3p
#fi


# SAMPLE: 
#[
#  {
#    "cloudName": "AzureCloud",
#    "id": "AZURE_ACCOUNT_ID",
#    "isDefault": true,
#    "name": "AZURE_ACCOUNT_NAME",
#    "state": "Enabled",
#    "tenantId": "AZURE_TENANT_ID",
#    "user": {
#      "name": "AZURE_USERNAME",
#      "type": "AZURE_USERTYPE"
#    }
#  }
#]

# for "Service Principal" 
# AZURE_CLIENT_ID = "id of an azure active directory application"
# AZURE_CLIENT_SECRET = "one of the applications secrets"


 log_📢_记录 "💖🥾 True=New Project or false=Clone from Repo? "
printf "truefalse: %s\n" $( get_true_false )


log_📢_记录 "💖🥾 YOU MUST SELECT A PROJECT ID"
Pr0J3ct1D=$( select_project_id )

# or PROJECT_ID=$( Pro)
log_📢_记录 "💖🥾 Pr0J3ct1D: $Pr0J3ct1D"




# todo:
# TODO: get ACR details
# TODO: get github 
# create a docker volume at the mount point
# link in 
AZURE_PATH="/c0de/$Pr0J3ct1D/.azure"
mkdir -p "$AZURE_PATH"

az group create --name $Pr0J3ct1D --location australiasoutheast

az ad sp create-for-rbac \
  --sdk-auth true \
  --name $Pr0J3ct1D \
  --role "/subscriptions/0b1f6471-1bf0-4dda-aec3-111122223333/resourceGroups/$Pr0JEct1D"


#{
#  "appId": "5efdb87f-e412-4b31-ad22-7f75a3320b83",
#  "displayName": "azure-cli-2021-05-02-17-20-48",
#  "name": "http://azure-cli-2021-05-02-17-20-48",
#  "password": "ebe8cc3a-3a8d-42b0-b322-92de12eea7f0",
#  "tenant": "26320343-bd50-40b1-82dd-865bb28dd266"
#}

# az ad sp create-for-rbac --keyvault  --sdk-auth true --skip-assignment true
# --role Contributor --scopes /subscriptions/{SubID}/resourceGroups/{ResourceGroup1}

# Access Control -
#   write to elasticdotventures/

# setup a storage account
# setup a service bus


# create a service principal
# bicep deploy an azure config & keystore


echo "az group delete --yes --name $Pr0J3ct1D" >> 
undo.sh




#echo -e 'first line\tfirst preview\nsecond line\tsecond preview' \
#    | fzf --delimiter='\t' \
#    --with-nth=1 --preview='echo {2}' --height 40% 



exit


# ARM templates & Github	
# https://devkimchi.com/2020/09/30/github-actions-and-arm-template-toolkit-to-test-bicep-codes/

# ARM template toolkit
# https://github.com/marketplace/actions/arm-template-toolkit-arm-ttk



name: Azure Bicep CI
# Controls when the action will run. 
on:
  # Triggers the workflow on push or pull request events but only for the main branch
  push:
    branches: [ main ]
  pull_request:
    branches: [ main ]
 
  # Allows you to run this workflow manually from the Actions tab
  workflow_dispatch:
 
# A workflow run is made up of one or more jobs that can run sequentially or in parallel
jobs:
  # This workflow contains a single job called "build"
  build:
    # The type of runner that the job will run on
    runs-on: ubuntu-latest
 
    # Steps represent a sequence of tasks that will be executed as part of the job
    steps:
      # Checks-out your repository under $GITHUB_WORKSPACE, so your job can access it
      - uses: actions/checkout@v2
 
      - run: | 
          curl -Lo bicepinstall https://github.com/Azure/bicep/releases/latest/download/bicep-linux-x64
          chmod +x ./bicepinstall
          sudo mv ./bicepinstall /usr/local/bin/bicep
          bicep --help
       
      - run:    bicep build ./bicep/storage.bicep
       
      - name: Archive production artifacts
        uses: actions/upload-artifact@v2
        with:
          name: dist-without-markdown
          path: ./**/*.json



https://github.com/marketplace/actions/arm-template-toolkit-arm-ttk


- run: | 
    curl -Lo bicepinstall https://github.com/Azure/bicep/releases/latest/download/bicep-linux-x64
    chmod +x ./bicepinstall
    sudo mv ./bicepinstall /usr/local/bin/bicep
    bicep --help

# Pivot to Tutorial 3

name: build_poi
on:
  push:
    paths:
      - "src/poi/**"
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - name: ACR build
        id: acr
        uses: ams0/acr-task-github-action@v1
        with:
          service_principal: ${{ secrets.service_principal }}
          service_principal_password: ${{ secrets.service_principal_password }}
          tenant: ${{ secrets.tenant }}
          registry: ${{ secrets.registry }}
          repository: ${{ secrets.repository }}
          image: poi
          git_access_token: ${{ secrets.git_access_token }}
          folder: src/poi
          dockerfile: ../../dockerfiles/Dockerfile_3










#################### LINE OF DEPRICATION ##########################################
# THE SAMPLE BELOW WAS NEVER USED, it's from tutorial 1 & 2

# Create workflow file github ui
actions > new workflow


on: [push]
name: Linux_Container_Workflow

jobs:
    build-and-deploy:
        runs-on: ubuntu-latest
        steps:
        # checkout the repo
        - name: 'Checkout GitHub Action'
          uses: actions/checkout@main
          
        - name: 'Login via Azure CLI'
          uses: azure/login@v1
          with:
            creds: ${{ secrets.AZURE_CREDENTIALS }}
        
        - name: 'Build and push image'
          uses: azure/docker-login@v1
          with:
            login-server: ${{ secrets.REGISTRY_LOGIN_SERVER }}
            username: ${{ secrets.REGISTRY_USERNAME }}
            password: ${{ secrets.REGISTRY_PASSWORD }}
        - run: |
            docker build . -t ${{ secrets.REGISTRY_LOGIN_SERVER }}/sportsworld-as2:${{ github.sha }}
            docker push ${{ secrets.REGISTRY_LOGIN_SERVER }}/sportsworld-as2:${{ github.sha }}

        - name: 'Deploy to Azure Container Instances'
          uses: 'azure/aci-deploy@v1'
          with:
            resource-group: ${{ secrets.RESOURCE_GROUP }}
            dns-name-label: ${{ secrets.RESOURCE_GROUP }}${{ github.run_number }}
            image: ${{ secrets.REGISTRY_LOGIN_SERVER }}/sportsworld-as2:latest
            registry-login-server: ${{ secrets.REGISTRY_LOGIN_SERVER }}
            registry-username: ${{ secrets.REGISTRY_USERNAME }}
            registry-password: ${{ secrets.REGISTRY_PASSWORD }}
            name: sportsworld-as2-docker
            environment-variables: key1=value1 key2=value2
            secure-environment-variables: key1=${{ secrets.ENV_VAL1 }} key2=${{ secrets.ENV_VAL2 }}
            location: 'east us'

        - uses: Azure/aci-deploy@v1
            with:
            resource-group: contoso
            dns-name-label: url-for-container
            image: nginx
            name: contoso-container
            azure-file-volume-share-name: shareName
            azure-file-volume-account-name: accountName
            azure-file-volume-account-key: ${{ secrets.AZURE_FILE_VOLUME_KEY }}
            azure-file-volume-mount-path: /mnt/volume1
            location: 'east us'            

