#!/bin/bash
source "/c0de/_b00t_/_b00t_.bashrc"

function install_b1c3p() {
    curl -Lo bicepinstall https://github.com/Azure/bicep/releases/latest/download/bicep-linux-x64
    chmod +x ./bicepinstall
    $SUDO_CMD mv ./bicepinstall /usr/local/bin/bicep
    bicep --help
}

if n0ta_xfile_📁_好不好 "/usr/local/bin/bicep" ; then
    log_📢_记录 "🤓 installing bicep. "
    install_b1c3p
fi

if [ $(az account list -o json | jq '. | length') -eq 1 ] ; then
    log_📢_记录 "found one account"
else
     log_📢_记录 "🍒 sorry, multi-account not supported (YET)."
fi

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
export AZURE_ACCOUNT_ID=$( az account list -o json | jq '.[0].id' )
export AZURE_ACCOUNT_NAME=$( az account list -o json | jq '.[0].name' )
export AZURE_TENANT_ID=$( az account list -o json | jq '.[0].tenantId' )
export AZURE_USERNAME=$( az account list -o json | jq '.[0].user.name' )
export AZURE_USERTYPE=$( az account list -o json | jq '.[0].user.type' )

# for "Service Principal" 
# AZURE_CLIENT_ID = "id of an azure active directory application"
# AZURE_CLIENT_SECRET = "one of the applications secrets"


# i.e.
# fuzzy_chooser 
#function fuzzy_chooser() {
#    local args=("$@")
#    local function=${args[0]}
#    local topic=${args[1]}
#    local key=${args[2]}

AZURE_LOCATION_ID=$( crudini_get "AZURE" "LOCATION_ID" )
if [ -z "$AZURE_LOCATION_ID" ] ; then
  log_📢_记录 "💙🤖🤓: Please choose a location"
  export AZURE_LOCATION_ID=$( az account list-locations -o json | jq -c --raw-output '.[]|[.name,.displayName] | @tsv' | sort | fzf-tmux --delimiter='\t' --with-nth=1 --preview='echo {2}' --height 40% | awk '{print $1}' )
  crudini_set "AZURE" "LOCATION_ID" $AZURE_LOCATION_ID
fi
log_📢_记录 "💙🤖 Location: $AZURE_LOCATION_ID"


## select a project id
# export PROJECT_ID = 
function select_project_id() {
  local selected=$(for i in {0..5}
  do
    if [ "$i" -eq "0" ] ; then 
      echo "_input_"
    else 
      echo $( Pr0J3ct1D )
    fi
  done | fzf-tmux )
  if [ "$selected" = "_input_" ] ; then
    read -p "Pr0J3ct1D:" selected
  fi 
  echo $selected
  return $?
}

## test to see how hard it is use fzf
function get_true_false() {
  echo "true
false" | fzf-tmux 
  return $?
}

## someday.. 
function emoji_menu() {

  # cat ../r3src_资源/inspiration.json | jq ".[]|[.symbol,.word] | @tsv" -r | fzf-tmux
  return 0 
}

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
