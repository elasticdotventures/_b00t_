#!/bin/bash

## * * * * * * * * * * * \\
#*
#* Purpose: ⚠️ THIS IS AN EXAMPLE/TEMPLATE! (code in here doesn't run)
#*
## * * * * * * * * * * * //

#* 进口v2 🥾 ALWAYS load c0re Libraries!
source $_B00T_C0DE_Path"/_b00t_.bashrc"
. .venv/bin/activate

if [ true ] ; then 

    $SUDO_CMD pip install wheel
    # $SUDO_CMD python setup.py bdist_wheel 

    pip3 install --pre --extra-index-url https://azcliprod.blob.core.windows.net/beta/simple/ azure-cli
    chmod +x /usr/bin/az

    # http://azure.github.io/azure-sdk-for-python/
    pip3 install azure-sdk-for-python

    # Blob Storage
    pip install azure-storage-blob

    ## * * * * \\
    # Azure Service bus
    # https://docs.microsoft.com/en-us/python/api/overview/azure/servicebus-readme?view=azure-python
    pip3 install azure-servicebus

    # Identity
    pip install azure-identity
    # from azure.identity import DefaultAzureCredential

    # https://github.com/census-instrumentation/opencensus-python/tree/master/contrib/opencensus-ext-azure
    # https://pypi.org/project/opencensus-ext-azure/
    pip3 install opencensus-ext-azure

    # https://docs.microsoft.com/en-us/python/api/overview/azure/appconfiguration-readme?view=azure-python
    # https://pypi.org/project/azure-appconfiguration/
    pip3 install azure-appconfiguration


    # Python vscode: 
    # https://docs.microsoft.com/en-us/azure/azure-functions/durable/quickstart-python-vscode
    pip3 install azure-functions
    pip3 install azure-functions-durable

    # AZ CLI Beta
    pip3 install --pre --extra-index-url https://azcliprod.blob.core.windows.net/beta/simple/ azure-cli

    # AZ fzf ?? 
    # https://docs.microsoft.com/en-us/cli/azure/fzf?view=azure-cli-latest
    az config set extension.use_dynamic_install=yes_without_prompt
    az config set auto-upgrade.enable=yes
    az upgrade
    az fzf install 

    # AZ Docs:
    ## 😲 this didn't work, ended up having to use Azure Portal
    #Create a tenant | Azure Active Direcotr
    #az provider register --namespace 'Microsoft.AzureActiveDirectory'
    ##az provider register --namespace 'Microsoft.AzureActiveDirectory' --accept-terms

    # What are Durable Functions (examples)
    # Function chaining, Fan-out/fan-in, Async HTTP APIs
    # Monitoring, Human interaction, Aggregator (stateful entities)
    # https://docs.microsoft.com/en-us/azure/azure-functions/durable/durable-functions-overview?tabs=csharp

    # ^^^ (continued)
    # Orchestrator, Activity Entity Functions
    # https://docs.microsoft.com/en-us/azure/azure-functions/durable/durable-functions-types-features-overview

    # Azure Functions developer guide
    # FUnction code vs Function App, Register binding extension
    # https://docs.microsoft.com/en-us/azure/azure-functions/functions-reference

    # Durable Orchestrations (Python & JS)
    # https://docs.microsoft.com/en-us/azure/azure-functions/durable/durable-functions-orchestrations?tabs=csharp

    # OPENAPI:
    # https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.1.0.md
    # https://github.com/OAI/OpenAPI-Specification#python
    # https://github.com/OAI/OpenAPI-Specification/blob/main/versions/2.0.md#format
    # Functions OpenAPI definition
    # https://docs.microsoft.com/en-us/azure/azure-functions/functions-openapi-definition
    # Connexion/Swagger
    # https://github.com/zalando/connexion
    # https://connexion.readthedocs.io/en/latest/quickstart.html#prerequisites
    # Flask:
    # https://haseebmajid.dev/blog/rest-api-openapi-flask-connexion
    # TS & OpenAPI:
    # https://github.com/metadevpro/openapi3-ts
    # Python & Open API:
    # https://pypi.org/project/openapi3/

    # Trigger Bindings for Durable Functions
    # https://docs.microsoft.com/en-us/azure/azure-functions/durable/durable-functions-bindings#activity-trigger
fi

dirWas=$(pwd)
tmpDir=$(mktemp -d)
cd $tmpDir
curl -Lo bicepinstall https://github.com/Azure/bicep/releases/latest/download/bicep-linux-x64
chmod +x ./bicepinstall
sudo mv ./bicepinstall /usr/local/bin/bicep
bicep --help
cd $dirWas

# AI
# in azure speak there are 4 types of NVIDIA Image for AI
az extension add --name azure-devops
az devops configure --defaults organization=https://elastic.ventures/r41nm4k3r project=r41nm4k3r
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

## NOTE: for "can't build rome in a day reasons"
## iv'e presently configured the r41nm4k3r dev environment
#mkdir azagent;cd azagent;curl -fkSL -o vstsagent.tar.gz https://vstsagentpackage.azureedge.net/agent/2.186.1/vsts-agent-linux-x64-2.186.1.tar.gz;tar -zxvf vstsagent.tar.gz; if [ -x "$(command -v systemctl)" ]; then ./config.sh --environment --environmentname "r41nm4k3r-pr0t0typ0s-sm311s11k3s01d3r" --acceptteeeula --agent $HOSTNAME --url https://dev.azure.com/elasticdotventures/ --work _work --projectname 'r41nm4k3r-b00t-图形蛇' --auth PAT --token zalrpp4xivpxopfc5dxamphlyt3eb44xean5vnpulumrmqtbtqqq --runasservice; sudo ./svc.sh install; sudo ./svc.sh start; else ./config.sh --environment --environmentname "r41nm4k3r-pr0t0typ0s-sm311s11k3s01d3r" --acceptteeeula --agent $HOSTNAME --url https://dev.azure.com/elasticdotventures/ --work _work --projectname 'r41nm4k3r-b00t-图形蛇' --auth PAT --token zalrpp4xivpxopfc5dxamphlyt3eb44xean5vnpulumrmqtbtqqq; ./run.sh; fi

#ssh -i r41nm4k3r--pr0t0typ0x--b00t_key.pem azureuser@r41nm4k3r--pr0t0typ0x--b00t.australiasoutheast.cloudapp.azure.com
#ssh -i r41nm4k3r--pr0t0typ0x--b00t_key.pem w1ndy@r41nm4k3r-nvidia.southeastasia.cloudapp.azure.com
