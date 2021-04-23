# __b00t__
ElasticDotVentures is 
a highly opinionated set of tools for deploying Azure Functions, with EV libraries (called "c0re", a subset of "c0de"), for Google 👾, Azure 🤖.  The pattern uses extensive use of 1337 speak for c0mm0n words which appear in projects.  The 1337speak is mostly used to make pnemonics easier to spot when applications are failing at the lower levels, and to indicate logical role.  Use tab-complete and it's no issue. 

# StoryTime Logging
_b00t_ is designed around the idea of "StoryTime" logging that includes Emoji & HSK 1 Chinese Vocabulary - Please Don't be intimidated.  Default settings leave English translations on.  The Author (@BrianHorakh) is a native English speaker and multi-language polygot (Spanish, Mandarin, ~Italian, ~German, ~Portguese) all have their own linguistic style and strengths. 

This code AS-IS is definitely NFSW for example a default project could be auto-named "butt_plug" or something like that but hopefully it's always in a cheeky and non-discriminatory way!  It's probably easy to make a SFW fork if you require that. 

At the highest level, Azure Durable Functions with Python & Typescript Connectors, Azure Service Bus, Azure KeyVault Configs, Azure ARM/Bicep 💪. Check Jargon.md for more the full glossary & naming conventions.  Docker 🐳, Python 🐍, TypeScript 🦄 - emoji indicates things like designee, consignee, etc.  This is an important aspect of the "storytell" logging, it creates really colorful error dumps and quickly helps primate brains abstract patterns that wouldn't be obvious in regular English text. 

# 为是吗TF is HSK1? 
Emoji pictograms are second-class languages that aren't allowed A LOT of places (such as Azure resources), fortunately Chinese is allowed. If we think of Chinese pictograms as the "original" black & white Emoji, HSK1 represents ~500 meanings.  When we talk about Good naming conventions -- i.e. logic-role, it's useful to use short uniform fixed width sequences 1-4 characters.  Using the western English 36 characters (A-Z,0-9) results in ~1.6m possible combinations & meanings.  Those same 4 characters using 1024 Emojis (~1.9tn), plus 500 HSK1 mandarin characters (~62.5bn), in total 1560 characters (36+1024+500) results in 5.9t possible "stories" encoded in 4 characters.

Wow, 4 characters representing 5.9 trillion stories, is a shit-ton of information.  With English mode, the system will add the English word (the Chinese pictograms are common, "5 year" words, at a deficit of 1 fixed character width per word.), it's never an issue since your brain will naturally & gently learn to encode the Chinese characters. Tada, you're learning to read Chinese while you code!  Chinese Mandarin represents 1.8 billion _native_ speakers, English also has roughly 1.8bn but only 400m are native speakers, everybody else speaks another language first.  

# What does _b00t_ do? 
_b00t_ is a complete pipeline orchestration system with integrated VS Code development environemnt, CI/CD Pipeline, base system which is suitable for deploying any cloud-scale state-less machine-learning project in frameworks such as Nvidia Cuda, PyTorch, Tensorflow, etc. as desired.  _b00t_ provides the base idempotent templates for resources & public/private code-libraries written in Azure ARM/Bicep. 

The ultimate output is a fully operational cloud-resource group, sensibile file shares, key-vaults, monitoring, logging scaffold skeletons in TS & Python as well (the "c0re") which presents itself as an interactive filesytem/blob storage.  The containers themselves could also be used to quarantine or freeze containers for forensic uses as well.  Python & Typescript bindings. I'll eventually add some higher level Vue templates and hardware IOT/Arduino & ESP32 templates as well. 

One aspect _b00t_ is that it can be hardened and subsequently removed (using docker dive) during the publish to live/production.

From the _b00t_ perspective it's trying to produce a cloud-function (or container) which is a readonly NVM-e backed memory blob that is frozen until it is triggered (probably by an inbound HTTP Websocket or Filesystem E_POLL notification).  The published container can be stripped down to ONLY perform the task it is assigned, thus improving it's security posture by removing tools & configuration files from public facing images. 

When this is combined with the fact that since AppConfig Stores & KeyVaults are used -- these are strongly typed first-order support  for example in Azure Logic Functions any secure tokens (such as passwords, or access keys) from a Vault are tagged and automatically filtered from logs as well, making compliance & user privacy easier! Extensive use of pipelines and messaging queues allow for tests and other large jobs to be run in parallel at cloud scale.  

# What is Cloud Scale?
Cloud Scale, with respect to _b00t_ refers to highly parallelized jobs which can be executed simulatenously.  For example if you have 1,500 tests and each one takes avg 2 seconds to run, that's roughly 50 minutes to "finish" tests before you can even start a build to production.  A cloud scale approach would be to complete those tests in parallel on 1,500 servers thus only takes 2 seconds (or, actually as long as the longest test, which is often a timeout, so ~60 seconds). 

# Why is it so Opionated? 
The _b00t_ organizational pattern is formatted around an intentionally lean "svelte" Enterprise. Debugging is on by default, serverless/consumption plans are default.  It assumes a development posture and assumes VS Code as an integrated environment, thus prescribing a suggest list of IDE extensions. 

_b00t_ tries to use a "Don't make me think" (any more than I need to) colorful, pattern rich, highly-compressed output. 
This allows for the system to "story tell" inside logs often showing an entire transaction as a series of pictograms.
It's amazing how easy it is to spot the problem in a sequence like this: 😁😁😁🤬😁
The deterministic nature means it's often possible to go back to the beginning of a transaction and re-run it. 

Here is a sample of the _projects_ opinion: 

```
/mnt/c0re/._b00t_./    # this is the current memory core for _b00t_.  It will contain keys, it could be ephemeral (such as one time use keys)
|- ./your_Project/..   # each project has it's own directory. A project will only mount it's directory though. 
A project can also delete data it no longer needs, but it is (for now) a good semi-durable hash. 

/c0de/*                # namespace on localfilesystem is mostly hardcoded. rationale: low DIRENT seek times by being @ /rootFs. 
 |- ./01-start.sh      # 🍰 Start Here!! Run this ./01-start.sh  
/c0de/_b00t_           # contains this template, used to 
 |- ./bash/            # anything in a .sh, templates
 |- ./bash/c0de_init.🚀.sh   # also, the main init script, called from ./01-start.sh 
 |- ./Dockerfile       # base Docker image (standard)
 |- ./docker.🐳/      # additional Docker build files, emoji coded 层 (Céng) Layers
 |- ./python.🐍/      # python stuff that will probably end up in _t00ls_
 |- ./node-ts.🦄/     # typescript libraries
/c0de/cloud.🌩️
 |- ./azure.🤖/       # azure cloud 
 |- ./google.👾/      # google cloud (still fresh) 
 |- ./aws.🦉/         # aws cloud (nothing planned here, presently) 
 |- ./aws.🦉/         # aws cloud (nothing planned here, presently) 
```

By subscribing to this pattern, an effort is made to obviate certain things.  Layers are built upon layers. 
For example a deployed system can be wiped of Dockerfiles using:
```
rm -Rf ./Dockerfile ./docker.🐳
```
This is handy at later builds.  For example a GIT filesystem can be stripped of utilities that is no longer needed.  Once that is compressed at a Docker Buildx layer then that information has destroyed during the idempotent container creation. 

It assumes the author will (ultimately) decide to end up using a combination of stateful logic, so it simplifies the interface to those by creating a unified command language that can be further build on.  There is a method to the madness, I assure you.  The patterns utilize serverless consumption plans whenever possible.  The plan is to eventually include complete VS code project files & plugin.    This assumes the developer(s) are using a three stage release model, "InnerLoop", "OuterLoop", "Production" each of those moving the data to the cloud and toward the public, no attempts are made . 


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

Then, once you've setup _b00t_ you can start to create your own projects. 
```
# to start a new project: 
/c0de/_b00t_/up new ~ 

# or: 
my_project_id="<YOUR PROJECT>"
/c0de/_b00t_/up new $my_project_id 

```

_b00t_ will create your project in /c0de/$my_project_id
In the future to upgrade _b00t_ you can simply use GIT. 
```

```


#  When Finished:
you'll have a fully integrated development environment with secure language bindings to two languages, full permission provision, resources with budget-friendly serverless consumption models by default. 

# To cleanup:
**NOT FINISHED**
```
/c0de/_b00t_/ZZ-cleanup.sh $my_project_id
```

# Emojis & Chinese on the CLI
The author is a hardcore CLI guy as well.  For some things using your mouse to copy-paste is better since it avoids fat fingers. Let's keep it real - nobody except masochists would try to AZ Resource strings, so _b00t_ strings are no different.  

For directories with emojis or mixed case, use tab complete and wildcards to hit targets. 
So ```cd /c0*/``` will chdir to ```/c0de/```
Generally the targets use Emoji & HSK at the end, but as an exercise: 
```/c0de/_b00t_/.../蓝色_Bicep_ARM_AzrResMgr.💪```

could be accessed from it's pwd using ANY of the following ```cd``` command. 
```
cd *Bicep*
cd *ARM*
cd *AzRes*
cd *💪
```
Technically this file is misnamed, should be ```Bicep_ARM_AzrResMgr.💙🤖💪```

On Windows, make sure you're using WSL2 on Unbuntu 20.04 with Windows Terminal Preview rather than the default shell and emoji works fine.  Putty and VS Terminal both work for outbound SSH and Sakura for inbound X-term/RDP.  If you're terminal doesn't support emoji, switch terminals.   If you're still using Vim you're missing intellisense in VS-Code and literally every single task is more difficult and error prone.  Tools like Azure ARM Bicep *assume* VS Code + intellisense for their transpiler also, so no VIM/Emacs support there. 

# StoryTelling in Emoji & HSK1 Chinese
I'm not gonna here, just read my Medium:
https://brianhorakh.medium.com/emoji-logging-warning-much-silliness-ahead-4cae73d7089

```

```

