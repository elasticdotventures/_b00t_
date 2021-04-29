

# Azure templates (bicep & ARM)
https://docs.microsoft.com/en-us/azure/templates/microsoft.containerinstance/containergroups?tabs=json#LogAnalytics

# Automate container image builds and maintenance with ACR tasks
https://docs.microsoft.com/en-us/azure/container-registry/container-registry-tasks-overview

# 
az deployment group create -f ./main.bicep -g my-rg --parameters location=westus storageAccountName=uniquelogstorage001

# Azure Bicep for SportsWorldAS2
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



