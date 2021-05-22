#!/bin/bash

## * * * * * * * * * * * \\
#*
#* Purpose: ⚠️ THIS IS AN EXAMPLE/TEMPLATE! (code in here doesn't run)
#*
## * * * * * * * * * * * //

#* 进口v2 🥾 ALWAYS load c0re Libraries!
source $_B00T_C0DE_Path"/_b00t_.bashrc"
. .venv/bin/activate

# seriously, azure-cli beta, it's br0ked. so going to use latest. 
docker pull mcr.microsoft.com/azure-cli:latest
# docker run -it mcr.microsoft.com/azure-cli:latest

# This resets on each use, so it needs to store or load the credentials to a shared spot. 
docker run -it mcr.microsoft.com/azure-cli:latest bash -c "az login"
docker run --rm -it -v ~/.azure:/root/.azure -v $(pwd):/root mcr.microsoft.com/azure-cli:2.9.1


if [ true ] ; then 

     # error: invalid command 'bdist_wheel'
    $SUDO_CMD pip install wheel
    # $SUDO_CMD python setup.py bdist_wheel 

    ## these beta builds are broken! 
    #pip3 install --pre --extra-index-url https://azcliprod.blob.core.windows.net/beta/simple/ azure-cli
    #chmod +x /usr/bin/az

    # http://azure.github.io/azure-sdk-for-python/
    pip3 install azure-sdk-for-python

    # Blob Storage
    pip3 install azure-storage-blob

    ## * * * * \\
    # Azure Service bus
    # https://docs.microsoft.com/en-us/python/api/overview/azure/servicebus-readme?view=azure-python
    pip3 install azure-servicebus

    # Identity
    pip3 install azure-identity
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
    # pip3 install --pre --extra-index-url https://azcliprod.blob.core.windows.net/beta/simple/ azure-cli

    # AZ fzf ?? 
    # https://docs.microsoft.com/en-us/cli/azure/fzf?view=azure-cli-latest
    # az config set extension.use_dynamic_install=yes_without_prompt
    # az config set auto-upgrade.enable=yes
    # az upgrade
    # az fzf install 

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

