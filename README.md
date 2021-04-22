# _pr0j3ct_
ElasticDotVentures is 
a highly opinionated set of tools for deploying Azure Functions, with EV C0r3 libraries, for Google 👾, Azure 🤖.  Extensive use of 1337 speak, emoji "storytell" logging, HSK 1 Chinese Vocabulary.  Azure Durable Functions, Service Bus, KeyVault Configs, ARM/Bicep 💪, check Jargon.md for more info on naming conventions.  Docker 🐳, Python 🐍, TypeScript 🦄 - emoji indicates things like designee, consignee, etc. 

# What this Is: 为是吗
This is elastic.ventures complete pipeline orchestration system with integrated VS Code development environemnt, CI/CD Pipeline, base system which is suitable for deploying any cloud-scale state-less machine-learning project in frameworks such as PyTorch, Tensorflow, etc. as required.  These are the base idempotent templates for resources & public/private code-libraries, written in Azure Bicep. 
The ultimate output is a fully operational cloud-resource group, sensibile file shares, key-vaults, monitoring, logging scaffold skeletons in TS & Python as well (the "c0re") which presents itself as an interactive filesytem/blob storage. 

# What is so Opionated? 
By subscribing to this pattern, an effort is made to obviate certain things.  It assumes the author will (ultimately) decide to end up using a combination of stateful logic, so it simplifies the interface to those by creating a unified command language that can be further build on.  There is a method to the madness, I assure you.  The patterns utilize serverless consumption plans whenever possible.  The plan is to eventually include complete VS code project files & plugin.    This assumes the developer(s) are using a three stage release model, "InnerLoop", "OuterLoop", "Production" each of those moving the data to the cloud and toward the public, no attempts are made . 

# Why Emoji & HSK1 Chinese
https://brianhorakh.medium.com/emoji-logging-warning-much-silliness-ahead-4cae73d7089


``txt
/c0de/_b00t_                     : this bootstrap code.
/c0de/_b00t_/01-start.sh         : setups up environment
/c0de/_b00t_/02-project.sh       : create a new project, with tooling. 
/c0de/_b00t_/ZZ-remove.sh        : clean up a project 
``


## Get Started: 
```bash
create a resource group:
[![Deploy to Azure](https://aka.ms/deploytoazurebutton)](https://portal.azure.com/#create/Microsoft.Template/uri/https%3A%2F%2Fraw.githubusercontent.com%2FAzure%2Fazure-quickstart-templates%2Fmaster%2F101-storage-account-create%2Fazuredeploy.json)

export resourceGroup="newProject"
export region="

mkdir -p /c0de && cd /c0de
git clone git@github.com:elasticdotventures/_pr0j3ct_.git
cd ./_project_/ && ./01-start.sh

```

# 
