
# setting up github & azure 

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
