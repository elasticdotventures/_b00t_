# _b00t_
ElasticDotVentures is 
a highly opinionated set of tools for deploying Azure Functions, with EV libraries (called "c0re", a subset of "c0de"), for Google 👾, Azure 🤖.  The pattern uses extensive use of 1337 speak for c0mm0n words which appear in projects.  The 1337speak is mostly used to make pnemonics easier to spot when applications are failing at the lower levels, and to indicate logical role.  Use tab-complete and it's no issue. 

# StoryTime Logging
The emoji's introduce "StoryTime" logging including HSK 1 Chinese Vocabulary - Please Don't be intimidated.  Default settings leave English translations on.  The Author (@BrianHorakh) is a native English speaker who is a polyglot so there's a lot of words that are correct in their native language, you might find some Spanish, German, Italian & Albanian.  Some parts of this code is definitely NFSW, for example a default project could be named "butt_plug" or something like that, but hopefully always in a cheeky and non-discriminatory  way! 

At the highest level, Azure Durable Functions with Python & Typescript Connectors, Azure Service Bus, Azure KeyVault Configs, Azure ARM/Bicep 💪. Check Jargon.md for more the full glossary & naming conventions.  Docker 🐳, Python 🐍, TypeScript 🦄 - emoji indicates things like designee, consignee, etc.  This is an important aspect of the "storytell" logging, it creates really colorful error dumps and quickly helps primate brains abstract patterns that wouldn't be obvious in regular English text. 

# 为是吗TF is HSK1? 
Some places Emoji isn't allowed, fortunately in most clouds Chinese is allowed. If we think of Chinese as the "original" Emoji, then we can start to group projects by logic-role in a uniform fixed width, quickly conveying a ton of information.  With English mode, the system will add the English word (the Chinese pictograms are common, "5 year" words, at a deficit of 1 fixed character width per word.), it's never an issue since your brain will naturally & gently encode the Chinese characters. Tada, you're learning to read Chinese while you code!  Chinese Mandarin represents 1.8 billion _native_ speakers, English also has roughly 1.8bn but only 400m are native speakers, everybody else speaks another language first.   

This is elastic.ventures complete pipeline orchestration system with integrated VS Code development environemnt, CI/CD Pipeline, base system which is suitable for deploying any cloud-scale state-less machine-learning project in frameworks such as PyTorch, Tensorflow, etc. as required.  These are the base idempotent templates for resources & public/private code-libraries, written in Azure Bicep. 
The ultimate output is a fully operational cloud-resource group, sensibile file shares, key-vaults, monitoring, logging scaffold skeletons in TS & Python as well (the "c0re") which presents itself as an interactive filesytem/blob storage.  The containers themselves can also be used to quarantine or freeze containers for forensic reasons.  

# What is so Opionated? 
The organizational pattern is formatted around a cross-competency, "Don't make me think" (any more than I need to) so it
assigns emojis to meanings.  This allows for the system to "story tell" during logs, showing an entire transaction as a series of pictograms.    Here is a sample of the _projects_ opinion: 

```
/mnt/c0re/._b00t_.     # this is the current memory core for _b00t_.  It will contain keys, it could be ephemeral (such as one time use keys)

/c0de/*                # namespace on localfilesystem is mostly hardcoded. rationale: low DIRENT seek times by being @ /rootFs. 
 |- ./01-start.sh      # 🍰 Start Here!! Run this ./01-start.sh  
/c0de/_b00t_           # contains this template, used to 
 |- ./bash/            # anything in a .sh, templates
 |- ./bash/c0de_init.🚀.sh   # also, the main init script, called from ./01-start.sh 
 |- ./Dockerfile       # base Docker image (standard)
 |- ./docker.🐳/      # additional Docker build files, emoji coded 层 (Céng) Layers
 |- ./python.🐍/      # python stuff that will probably end up in _t00ls_
 |- ./azure.🤖/       # azure cloud 
 |- ./google.👾/      # google cloud (mostly for gsheet, apis) 
 |- ./aws.🦉/         # aws cloud (nothing planned here, presently) 
 |- ./node-ts.🦄/     # typescript libraries
```

By subscribing to this pattern, an effort is made to obviate certain things.  Layers are built upon layers. 
For example a deployed system can be wiped of Dockerfiles using:
```
rm -Rf ./Dockerfile ./docker.🐳
```
This is handy at later builds.  For example a GIT filesystem can be stripped of utilities that is no longer needed.  Once that is compressed at a Docker Buildx layer then that information has destroyed during the idempotent container creation. 

It assumes the author will (ultimately) decide to end up using a combination of stateful logic, so it simplifies the interface to those by creating a unified command language that can be further build on.  There is a method to the madness, I assure you.  The patterns utilize serverless consumption plans whenever possible.  The plan is to eventually include complete VS code project files & plugin.    This assumes the developer(s) are using a three stage release model, "InnerLoop", "OuterLoop", "Production" each of those moving the data to the cloud and toward the public, no attempts are made . 

# Why Emoji & HSK1 Chinese
I'm not gonna here, just read my Medium:
https://brianhorakh.medium.com/emoji-logging-warning-much-silliness-ahead-4cae73d7089


``txt
/c0de/_b00t_                     : this bootstrap code.
/c0de/_b00t_/01-start.sh         : setups up environment
/c0de/_b00t_/02-project.sh       : create a new project, with tooling. 
/c0de/_b00t_/ZZ-remove.sh        : clean up a project 
``





## Get Started: 
```bash
Someday I'll have this DEPLOY to AZURE working., for

create a resource group:
[![Deploy to Azure](https://aka.ms/deploytoazurebutton)](https://portal.azure.com/#create/Microsoft.Template/uri/https%3A%2F%2Fraw.githubusercontent.com%2FAzure%2Fazure-quickstart-templates%2Fmaster%2F101-storage-account-create%2Fazuredeploy.json)

For now:

export resourceGroup="newProject"
export region="useast"

mkdir -p /c0de && cd /c0de
git clone git@github.com:elasticdotventures/_b00t_.git
cd ./_b00t_/ && ./01-start.sh

that will start running the soon-to-be interactive installer. 

```
#  When Finished:
you'll have a fully integrated development environment with secure language bindings to two languages, full permission provision, resources with budget-friendly serverless consumption models by default. 
