
[![Gitter](https://badges.gitter.im/elasticdotventures/community.svg)](https://gitter.im/elasticdotventures/community?utm_source=badge&utm_medium=badge&utm_campaign=pr-badge)


[![Project Status: WIP – Initial development is in progress, but there has not yet been a stable, usable release suitable for the public.](https://www.repostatus.org/badges/latest/wip.svg)](https://www.repostatus.org/#wip)

# \__b00t__
------
\__b00t\__ 'cognitive middleware'
@elasticdotventures:matrix.org

## presently alpha/没做完
** VERY UNFINISHED **

B00t is a polylingual polyframework voice-vision-cli "cognitive format transformer & instruction learning" approach to automata.

An idiomatically control language for an assistant that can "learn" new skills through direction or experimentation. 

Using external API provider, such as Jovo allows _b00t_ to interface with either the PSTN (phone network) or relay through smart assistants (targeting: cortana & google home).  

_b00t_ is built using an 'open source' tool-chain inside Win11/WSL2 or other Linux environment. 

Examples of 'skill' target(s) are ambitious:
* design, simulate, control and monitor robotic manufacturing & cloud infrastructure
* GPT summarize text, and/or math, write code, methods to build "develop" new skills as directed
* attend meetings (via video/audio intercept & character avatar synthesis & lip-sync)
* interactively screen PSTN & messanger calls/sessions & make appointments, website chat-bot
* write reports & visualizations (online three.js, svg, collada, png) /maybe excel, google sheets/
* write & maintain code & orm graph schema and interfaces to svg, openscad, stl, collada, sql, 
* transpilation of python, rust, & typescript => (wasm) => skills 
* skills are either 'bash steps' or 'Makefiles', which can be automated using ci/cd 
* find & import 3d objects, electronics data-sheets to for references in digital shadows.

FUTURE:
* theorize about new object structures, physics, electro-chemistry, protein folding, etc.
* injest metrics from eBPF hooks for OS-level threat monitoring & telemetry
* play/wingman @video games 'good enough' to be fun, including social aspects
* design and print 3d objects & assemblies (and simulations, etc.)
* prepare & apply video & audio for presentations, etc. 
* purchasing, order reconciliation & accounting, customer service, HR (using zero code i.e. node-red, appsmith or Az durable functions)

These capabilities are rolled up into interactive macros called 'tasks', which when grouped together are called a 'toil'.
The skills are build in an 'oregon trail' where the operator must decide which tasks are 'worthy' of receiving operating cycles, and what (if any) resources must be used for discovery & digestion of new skills.  In some cases the operator may (such as using a mouse, joystick, voice, etc.) "fit" the AI by helping it assemble, compile, etc.  to teach skills, explain materials, objects, etc.  

The main cost with a system like this is the operational cost might doesn't work the same as an employee.

Being able to consume video, audio, websites, documents, data-feeds, injest/participate in social systems such as twitter, discord, matrix, slack & ms-teams in a poly-medium 'chat interface'. 

Azure is preferred because SSO + Active Directory simplifies a variety of security + access control toil in terms of interfacing, although at some point I prefer to replace this with zanzibar and offer at least an AWS & k8s terra-form approach but that isn't explicitly necessary for this project since it's all (mostly) serverless.  

Functionally driven "smart pipes" and should be generic enough that a new software/service simply by reading it's API (i.e. an API transpiler) that is compatible with one or more of it's messaging or database protocols, especially if that's OpenAPI/Swagger, SQL, GraphQL, or JSON/protobufs.  It is designed to 'assist' a human or small team, but to amplify and obviate the require for *most* big companies by efficiently replacing people with intelligent robotic process control.  

*VERY WIP*

Presently this is only a compendium of my tools & undocumented scripts for a (future) interactive initalization (using fzf) setup & installer. 

I'm presently learning/rewriting this to RUST 😍🦀, and solidifying on Python 😍🐍, TypeScript 😍🦄 interfaces .. WASM is a dream.
Turns out cargo is amazing and obviates a lot of work, RUST's complex pattern matching language is awesome (reminds me of Perl).
On the embedded side I've moved from C++ to RUST as well, so ESP32 S2/C3 are the target 'hardware' platforms but it could run on an RPI or whatever for physical motor & sensor i/o.  

By generating *everything* programmatically each physical object will have a digital object shadow, that would include inheritance & robust 'math' & 'logic' to describe how the object is used, programmatically describe it's motion, etc.  this makes setting up things like  monitoring via OpenTelemetry straightforward and more standardized across a big / complex project. 

For the 3d objects I've now abandoned my traditional mouse-driven parametric CAD (OnShape) entirely and moved to FreeCAD so I could use Python.  I've now discovered OpenSCAD and it's vitamin module libraries (which aren't terribly well organized).  I'm presently formulating/learning how to write a RUST transpiler (which basically means reading a lot of other peoples RUST code) into OpenSCAD.  But learning OpenSCAD has been awesome & rewarding. ;-)

Once that task is finished then I will be either begin adding the "AI" features including
	* CLIP or CycleGAN commands to let the AI design & fill textures, backgrounds
	* GPT-3 or GPT-NEOx to create SVG & OpenSCAD codex templates
	
I also intend for this *eventually* to be a massive toil reduction & time saver by using git to track & approve asset changes, objects & complex assemblies in sync auto-magically (automagic: without doing File | Import/Export) between apps - presenting the same state across FreeCAD (easy), Blender (via Collada) & possibly Unity (also Collada) using a git-action build chain (which can also run locally) the resulting approach then be used for visualization, simulation, training & regression testing prior to output of complex FDM 3d-printed & CNC milled assemblies, eventually building toward zero-human robotic manufacturing & assembly.  "Post-work"

Once all that is working I will start on the VHDL 'Verilog' & SpiceDB PCB & circuit simulator as well at which point KiCad is the most likely candidate for interface, but realistically we'll just be using KiCad for it's component libraries and treating the PCB's as complex assemblies. 

This repo is the 'base', it's intended to be used as a project template via a sub-module to print in a LOT of dependencies and try to provide some harmony to the chorus of OSS dev happening around the planet in 2022.

The plan is to have a Yahoo-esque "Awesome" curated list of *installable* 
time-saving & labor-reducing tools "fundamentals", to interface any cloud AI/Skill. 

next version will feature better docker compose, rust cli

Sharing with contributors, advisors, feedback, & testers (soon). Comments, issues, PR's welcome. Please 🍴🍰🤩

Keep up to date and get Pachyderm support via:
- [![Twitter](https://img.shields.io/twitter/follow/_b00t_?style=social)](https://twitter.com/brianhorakh) Follow us on Twitter


------
```text/plain
      __         __      __   __                
     /\ \      /'__`\  /'__`\/\ \__             
     \ \ \____/\ \ \ \/\ \/\ \ \ ,_\            
      \ \ '__`\ \ \/\ \ \ \/\ \ \ \/            
       \ \ \L\ \ \ \/\ \ \ \_\ \ \ \_           
        \ \_,__/\ \____/\ \____/\ \__\          
  _______\/___/  \/___/  \/___/  \/__/  _______ 
 /\______\                             /\______\
 \/______/       开机🥾🐛虫亡          \/______/ 
   B U G   S Q U 1 S H 1 N G   S 0 F T W A R 3
            
          FOR THE BENEFIT OF HUMANITY
                 THE ULTIMATE 
             COMPENDIUM OF USEFUL
          MULTI-DISCIPLINARY END-TO-END
         SYSTEMS ENGINEERING TOOLKIT FOR
          🧙 REDUCTION IN DIFFICULTY

  THIS REPO COMPONENTS: 
    bash, python, ts scaffold(s)
    * shells:
      * https://starship.rs/installing/#termux
      * https://doc.redox-os.org/ion-manual/repl.html
      * https://murex.rocks/docs/GUIDE.quick-start.html
      * https://nixos.org/manual/nix/unstable/command-ref/nix-shell.html
    * extensive use of more-utils, yq, jq, fzf, fdfind, etc.
  SERVER ARCHITECTURES:
    x86-nvidia-cuda-optimized, x86, amd64, arm64-rpi 
    alpine (planned, blocked by 3rd party webinstall.dev)
  EMBEDDED ARCHITECTURES:
    atmega64, esp32
  DEV:
    vs-code on linux or wsl2
    git, github (issues, build ci/cd)
    idempotent (app-templates, app-config, app-data, app-lib, app-src separated)
    platform.io (for embedded)
    * extensive: json/yaml/toml setup "working configurations"
  FRONT END/APP: 
    Typescript/Vue (Vitesse SPA, I18N template)
    OpenAuth/OpenID to Azure B2C, MSAL2 (federated security)
    interfaces: rest/GraphQL, RxJS
    zip/open-openTelemetry setup
  BACK END/APP:
    Typescript: >v4 
    Python: 3.9 
    (and others, poly-lingual)
  CONTAINER ORCHESTRATION: 
    docker, k8, drone-ci (and others, any OCI is fine)
    * author bias: minimalist k8, minikube
    pachctl (data lineage)
  CLOUD: 
    azure, google, aws, ali
    * author bias: azure > gcp | ali | aws
    * Azure will have the most extensive support *initially
  DATABASE:
    ephermal: sqlite
    local: sqlite, firebird, mariadb, postgres
    * author bias: value-engineering to freeze/unfreeze projects
    poly-cloud managed (firebase, nosql, etc.) 
    * plurality of storage volume options
    * cloud for online/on-demand cloud hosted or offline "cold"
  MESSAGING:
    protobufs, redis, rabbitmq, zmq, mqtt, (and others)
  SECURITY:
    low to perfect (per complexity/risk requirements)
  POLY-SCENARIOS:
    artificial intelligence, ag robotics, hardware sensors
    iot dev, 3d rendering, red-team cybersecurity, ecommerce 
    sales, finance accounting, chat/ops business operations
    automation, crypto banking, academic science r&d,
    r-engineering, tiger-team problem solving (aka math) 

  FUTURE:
    * STACK-PIERCER
	cargo install zellij
        eBPF filters & io_uring bindings
        openTelemetry logging
        Bash "bats" - TAP testing https://testanything.org/
    * STORYTIME: 
        AI/OPS, interactive voice-cli/assistant control surface
   
            _b00t_ is a CYB3RPUNK_STARTUP_K1T

    ..UNSUITABLE FOR MILITARY OR ANTISOCIAL PROJECTS..
    *per the license, for valid-inclusion in hackathons

        Elastic License v2 (ELK2) Open License 

```

seeking testers, investors, contributors, any feedback/peer-review. star the repo if you think I should keep working on this. I'm planning to submit this to OReilly as a nutshell book idea "_b00t_: from the ground up".   

## 🤓 QUICKSTART:
```bash
git clone https://github.com/elasticdotventures/_b00t_.git

# fzf setup/tutorial menu
bash$ ./_b00t_/_b00t_.sh

# or just load the tooling/environment
bash$ source ./_b00t_/_b00t_.bashrc

# or checkout the repo, 85% of the "finished" work is presently in the [bash.🔨/]() folder. 
```


# \__b00t__: A View from the Ground Up

\__b00t__ is systems-engineering poly-glue publishing transpiler, it will use menus to drive a cli for choosing and updating "poly-stack scenarios" which are infrastructure code & templates & working-samples for all relevant stack layers "层".  _b00t_ intends to provide a curated starting point for a variety of poly-disciplinary aka "complex" projects. A modern curated catalog of computer interfaces, academic and elastic libraries. This project was born partially out of necessity and the rest out of pure curiousity for what could be built and how quickly a menu driven "Oregon trail" of architectural templates for a project. 

_b00t_ is colorful, and not traditional, it is "cheeky", but non-discriminatory, it is poly-lingual but it may not be suitable for traditional corporate or government environments, as such it is better suited for the home inventor / hobbyiest or open-source hardware developer/fabricator, children under 16 should ask your parents before using _b00t_. 

Emoji and ideogram based utf8 / i18n called "storytime v1" naming conventions for an "Oregon Trail" themed menu, or anything goes CLI project init-to-exit coverage.  This non traditional approach offer faster diagnostics and efficient operational characteristics allowing 'svelte-teams' for ops-scaffolding. 

For containers, installation of development environment naming with emoji is improved with menus, intellisense editors, and cli tab completion.  _b00t_ heavily uses VS-code extensions for read/write/running private or hosted zero-code engines, an artificially written/operated infrastructures "AI/OPS", hopefully improving indivual productivity to 100x for creating poly-disciplinary projects. 

Presently _b00t_ hopes to become a modern slackware (built on Linux, a deployable Yahoo directory of Open Source, with some light tutorial "learn only what you desire to", with some 'minimal' explanation of truth/consequences "Oregon Trail" of decisions framework for generating idempotent templates to build complex, maintainable, efficient "systems" engineering principles.

_b00t_ will attempt to present itself in many formats and give operators a massive "common" arsenal of pre-interoperably-configured tools & example projects in python, typescript, bash, etc. ("poly-lingual") as well as popular patterns for integrating tools "from the ground up".  _b00t_ includes some strong opinions such as VS-code development environment. 

_b00t_ can be thought of as a cloud-shell enhancement, and it's primary role is as a catalyst to easily deploy a "reliable pattern" for interconnection between applications.  A menu-driven OR cli driven poly-stack-poly-interface-poly-software "awesome" distribution, examples of complex json/yaml manipulation, complete automation of daily activities, ultimately I hope to add a CLI voice interface (based on Jovo) .. something like how Tony Stark interacts with Jarvis for software projects and it could sell, support and provision the software.  To accomplish this I need to structure this repo & tools in a uniquely _b00t_ way, which I'm attempting to make easier for myself, and hopefully you.

_b00t_ is variable low-to-high security postures, for example there are a variety of security postures available from secret keys stored in a local .env file, a self hosted vault, or a cloud based key repository.  protocols & packages have been curated based on their utility in poly-lingua, poly-cloud technology library aka "_b00t_".   The key is providing a known set of steps to differentiate the posture between dev/build/release.

_b00t_ framework itself is k8-minimalist, but very pro-git pipelines/automation. k8 "kubernetes" should only be used when it is absolutely necessary, and then as sparingly as possible. Pipelines, idempotentent containers, those are 100% awesome.

k8 support is my present area of improvement in bash.🔨/ scripting.



## TODO
Trying not to re-invent the wheel, found a bunch of new tools and refactoring mid/project: 
* bsfl 
* https://github.blog/2015-06-30-scripts-to-rule-them-all/
* https://github.com/bashup/.devkit
* https://github.com/bashup/mdsh
* https://github.com/bashup/events
* https://github.com/bashup/lore
* https://github.com/bashup/loco
* azure graph openid authentication scaffold
* in typescript spa vue vite vitesse based template
* please excuse the scattered bits, work in progress

```
## you might try the setup file, or explore the color repo!
bash$ cd _b00t_; ./01-startup.sh 

```

## 🥾 is presently suitable for: 
probably absolutely nobody besides Brian Horakh (the author).
but i welcome anybody with similar passions in pre-dev stage projects, incubators, self-reliant engineering teams (no vendor support, except the Author & Git Issues) who want to sponsor my progress or would like to use this framework in their own companies. 

Sponsor through github, or the _b00t_ Ethereum Address is:
https://vittominacori.github.io/ethereum-badge/detail.html?address=0x24bd79c0efff201bd5b24b622d5ea09f93dbfaa3


🙏 Please note, $$ goes a long way towards demonstrating to my wife that this warrants the amount of attention I give it versus her projects. Thanks! ask for ☕ if you want! Social likes, github ⭐, twitter follows.  Share with your friends!


## _b00t_ vision: 
A multi-stack directory, "from the ground up", with directory of poly lingual-interfaces like docker Moby. _b00t_ is for:
* AI/Robotics/CyberSec/Code Hackathon Groups or red-teams who need to "tandem" or "poly-hackathon", global startup "code-a-thon" who need fast iteration and shared debugging. 
* Hackers who want to deploy multi-language code across serverless functions & docker containers (in Azure). 
* System engineers or integrators building a container system
* Application developers looking for an easy way to run their Python, Typescript and related applications in containers with VS Code, Azure Tooling & best practices.

_b00t_ intends to provide an early stage complete open ops-stack for a company (examples included) to quickly easily and affordably, minimize and compartmentalized "best-practices" templates for pr0j3ct design.  Caching and also runtime isolation (for offpeak processing $Saving m0n3y!) other layers included to reduce costs and monitor integrity, but can also be accessed as a slimmed down bare os and should 'probably' work in anything which can run aptitude package manager (usually debian, ubuntu).  This targets ubuntu, all architectures. 

## What _b00t_ intends to provide: 
```
    * Functional Examples & Opinionated Toolchoices & VS Code Dev Environment & Plugins 
    * FZF menuing and high automata, with detailed config notes. 
    * Repeatable "clean" environment with sane defaults, that can be easily disabled i.e. remove python or remote typescript, select database, deployment zones, etc. 
    ** For application developers. 
```

## Application Design: 
```
    * Best industry practices security built-in OAuth/OpenID
    * Hardened Determinsic Patterns using Azure Durable Functions
    * Multi-facet ideogram cyb3rpunk! Emoji & Pinyin filled logs & metrics, such 3117! 
    * Multi-stage dev, build, test, deploy CI/CD is presently done through github & azure app-services. 
    * Containerized Projects, using K8-esque pipelines
    * Especially well suited for:
        * Machine Learning Create=>Cloud_Deploy Pipelines
        * Bi-lingual projects using Python & Typescript (or others)
        * Your Project
    * Demo/Modules Roadmap (using _b00t_):
        * v0: present. work in progress. 
        * v1: AiiA Call Center [http://github.com/elasticdotventures/aiia-callcenter] - Small Business NLP/TTS/STT "Call Center" intelligent assistant based on Jovo framework, with additional interfaces for discord & slack. bigCommerce integration planned. 
        * v2: DongXi Inventory [http://github.com/elasticdotventures/dongxi] Discrete Inventory & Accounting, Ethereum & Solidity Interfaces. bigCommerce integration.  
        * v3: gr0w.P0t.B0t [http://github.com/elasticdotventures/gr0w.P0t.B0t]  AI-driven Hydroponic Biogenesis Appliance (hydroponic farm controller) built on ESP32 with AI. 

   Currently under development:
      * internal libraries and essential tooling, docker deploy, then integrate AZ with bicep. 
      * fzf menus & installer
```

# \__b00t__: the vision 

## What About \_b00t_?
ElasticDotVentures \_b00t_ is 
a highly opinionated set of tools for deploying Azure cloud services with EV libraries (called "c0re"), mostly for Azure 🤖.  \_b00t_ brings together a plurality of powerful Cloud tools to  to encourage a non-traditional but extremely p0werful mix-martial-art-of-code, which itself is a curated catalog of utilities to manage "infrastructure as code" presently DEV/OPS, but with an eye toward future AI/OPS.   This text here is mostly visionary explaining the "what" about _b00t_ which is presently early stage being re_b00t_ed😆 from my prior projects. 

## _b00t_ Syntax Rationale: 
The _b00t_ approach uses incorporates 1337 speak for c0mm0n words to encourage brevity at all layers.

The 1337speak used in _b00t_ is primarily used to make unique pnemonics that are substantially easier to grep during a subsystem trace across layers in the stack, thus providing f1ng3r printing.  Glue itself isn't a tool, it's designed to demonstrate how to deploy [for example] deploy a harware or machine learning transformer models affordbly at scale. 

The naming models create colorful and meaningful filters to radically improve code quality & debuggability and incorporating "zero-code" deterministic actions Azure Logic Functions. 

The higher visual payload of short 1 & 4 character Emoji & HSK1 in names is informative, and has valuable screen real-estate. WIN+";" is emoji keyboard /(*in windows)/ - but Emoji with cut and paste + intellisense make this easier than you'd think.  Especially when you type "d0cker" and a 🐳 pops up in spell check using a custom dictionary.  


1337 mechanics generally indicate logical role or purpose. Using tab-complete or mouse-hover in VS-Code Intellisense IDE and CLI makes it easy and artistic on the screen reinforcing art-in-code.  _b00t_ focusing on the pictures and basic glyph optimized based Simplified Mandarin only reading the code when it's necessary since the human brain is better at visual pattern sorting than it is reading.  This approach allows for grouping by symbol and simplifies some aspects of command line searching properties, test generation and several other aspects.  

Built to deploy Azure Logic Connectors & Azure Durable Functions with Python & Typescript Connectors, Azure Service Bus 🚌, Azure KeyVault💎 Configs⚙️, Azure ARM/Bicep 💪. Check Jargon.md for more the full glossary & naming conventions.  

Docker 🐳, Python 🐍, TypeScript 🦄 - emoji indicates things like designee, consignee, etc.  This is an important aspect of the "storytell" logging.  StoryTell creates really colorful transaction logs & error dumps, these will eventually be extended to perform basic ML application forensics & monitoring.  These meta-patterns (using computers to monitor computers) helps our soon-to-be-obsolete primate brains abstract patterns that wouldn't be nearly as obvious in regular English text and notice problems and inform the determinisitic control surfaces to take action (isolate, block-hold, ignore with possible consequences 😬).  This isn't the whole application freezing, it's a message in an application, or a corrupt video frame grabber in a video stream, any payload which doesn't match the model.   It's not lost forever, it's simply flagged for review. 


# Emoji in Code
One of the most controversial aspects of _b00t_ is it's heavy use of emoji and pinyin. The reality is NLP (natural language processing) of English is really difficult "expensive & less-reliable" than the one character per word/symbol approach.  
In the _b00t_ world, any emoji which is "red" in color is "bad" a warning/color.  If a build process has any red symbols that appear, that's a problem.  Even the Cherry "🍒" is reserved as a missing element (a token which the user could "solve" and maybe someday earn points). 

_b00t_ is bi-lingual, one of the c0des it understands is bash script.  
Here's a sample Bash function taken from the c0re to see if a machine is running Windows System for Linux version 2, abbreviated to WSLv2. The Emoji colorfully demonstrates this bi-lingual principle: 
```bash

## Microsoft Windows Linux Subsystem II 
## 🤓 https://docs.microsoft.com/en-us/windows/wsl/install-win10
#
function is_WSLv2_🐧💙🪟v2() {
    return `cat /proc/version | grep -c "microsoft-standard-WSL2"`
}

```

## And here's how to decode it:
* 🐧 Penguin .....  Linux (Tux, Mascot)
* 💙 Blue Heart ..  Microsoft:💙, Google:💚, AWS:🪙
* 🪟 Windows .....  self explanatory! (*may not appear on Android!)

So: Without knowing what WSLv2 "is" using only three symbols you can infer a lot about it! 

## ⚠️ Disclaimer
This is Alpha software. _b00t_ is Provided AS-IS! \_b00t_is rique NFSW, for example a default project could be auto-named "butt_plug" or something like that, it's intentionally cheeky and non-discriminatory.  _b00t_ contains a powerful build process that can muster substantial resources.  

## 🥋 Mixed-Martial-Art-Of-Coding: "StoryTime Logging"
\_b00t_ is designed around the idea of "StoryTime" logging that includes Emoji & HSK 1 Chinese Vocabulary - Please Don't be intimidated.  Default settings leave English translations on.  The Author (@BrianHorakh) is a native English speaker and multi-language polygot (Spanish, Mandarin, ~Italian, ~German) all have their own linguistic style and strengths.  Think of this as mixed-martial-art-of-coding. 

I'm not suggesting anybody actually learn Chinese, your browser already knows it. Install Google Translate, right click any symbol.

## 🤯 为是吗TF use Pinyin? 
How much information can be transmitted in a symbol vs. a character? 

HSK1 is Level 1 Chinese Mandarin language using simplified pinyin symbols "ideograms" for action/commands and stack layer naming conventions (designators for frontend-前面(Qiánmiàn), backend-背面 [Bèimiàn]).
```
In "Correct" Chinese files would be named like this:
file.前面  relates to the front-end
file.背面  backend
```
### But wait! why waste a character? 
```
file.前火  front-end firewall
file.背器  backend container
```
### Now, let's add some emoji
```
file.前火.🔥🧱  front-end firewall
file.背器.🔚🐳  backend container
```
(these are ONLY illustrative examples to you show you symbology) 
Using a browser plugin you can translate anything in the documentation easily.  But by integrating _b00t_ scripting at various stages it produces a well defined pattern based marshalling "summary" layer of logic for inputs and outputs of different subsystems. 

Simplified Pinyin is a screen glyph optimized font - that means HSK1 characters are *by design* very easy to read & learn!  Presently emoji pictograms are second-class languages are regionally-ambiguous (i.e. "🍆💦" has a plurality of meanings) see [jargon.md](). 

[README_名约_syntax.md]

# 🤩 Wait, What does _b00t_ do? 
\_b00t_ is a novel and *opinionated* pipeline orchestration system with integrated VS Code development environemnt, CI/CD Pipeline, base system which is suitable for deploying any cloud-scale state-less machine-learning project in frameworks such as Nvidia Cuda, PyTorch, Tensorflow, etc. as desired.  \_b00t_ provides the base idempotent templates for resources & public/private code-libraries written in Azure ARM/Bicep. 

The ultimate output is a fully operational cloud-resource group, sensibile file shares, key-vaults, monitoring, logging scaffold skeletons in TS & Python as well (the "c0re") which presents itself as an interactive filesytem/blob storage.  Python & Typescript bindings. I'll eventually add some higher level Vue templates and hardware IOT/Arduino & ESP32 templates as well. 

One unique aspect of \_b00t_ is that it will allow the ultimate images to be removed (using docker dive) during the publish to live/production.

From the \_b00t_ perspective it's going to help you build SOMETHING, it's only a foundation.  Further application templates can be built on _b00t_ and then easily upgraded taking advantage of new features. 

In Cloud based determinsic systems (such as Azure Logic Functions), containers can be "frozen" fully loaded, or kept hot (with standbys) based on a load balancer. Such as a cloud-function (or container) can be readonly NVM-e backed memory "blob" that is awaiting a trigger (probably by an inbound HTTP Websocket, E_POLL, or io_submit() ). 
The published container can be stripped down thus improving both size & security posture by removing tools & configuration files from public facing images. 

Cloud based AppConfig Stores & KeyVaults are used. KeyVaults contain types that are first-order types which are aware the passwords, etc. in them must be kept secure.  For example in Azure Logic Functions secure tokens (such as passwords, or access keys) from a Vault are tagged and automatically beautifully-filtered from logs as well! This makes compliance & user privacy easier! Extensive use of pipelines and messaging queues allow for tests and other large jobs to be run in parallel at cloud scale.  

# Why is _b00t_ Opionated mean? 
For example, _b00t_ believes that VS Code with it's intellisense, typescript & python, docker, azure, aws, gcp, code-auditing, and plurality of other useful extensions makes VS Code the one true editor.  The author believes _b00t_ pattern works best when using remote containers, which is one of the 层 Layers it builds & configures. 

The \_b00t_ organizational pattern is formatted around an intentionally lean "svelte" Enterprise, everything is automated and structured for easy updates using GitOps and JSON. Debugging is on by default but can be reduced later to save $$. Serverless/consumption plans are also default and cost centers are isolated by project/resource group for good reporting & security.  _b00t_ assumes an agile cadence of releasing early and continuously integration. Fast fail.

_b00t_ also is a K8 "minimalist" recognizing that K8 has a large footprint and creates significant unnecessary complexity for cloud-native (non-LEGACY) applications. 

# Resource Division
_b00t_ uses Azure Landing Zone "best practice" patterns to orchestrate away a considerable amount of complexity in terms of assembling a plurality of well known popular libraries and tools together inside docker containers that then fit into a local inner/outer development loop.  

# B00t tries NOT to reinvent the wheel*

_b00t_ assumes VS Code as an integrated environment, thus prescribing a suggest list of IDE extensions, along with a 
sufficiently pnemonic unique naming approach using ideograms & pinyin command glyphs for a variety of tasks such as idea routing & error handling "storytell mode". An emphasis is put on Windows development but uses a remote container development environment so it's highly agnostic of individual OS choice.  _b00t_ relies heavily on intellisense and the VS-code extension ecosystem, especially it's integration with Azure for a variety of "1 click" tasks.

# /c0de/_b00t_
Presently _b00t_ generally expects to be in /c0de/_b00t_. 

Ultimately __b00t__ will include internal tooling sufficient to _b00t_ a company or product in a few hours.  It presumes Bash, Typescript and Python environment and also works to some extent on raspberry pi (i.e. "no nvidia cuda").  I intend iot esp32 and arduino functionality in future releases. 

Built on top of the _b00t_ template.  Plans to compile a variety of unique reporting summarized using _b00t_ notation to quickly assess project quality, code sentiment analysis and identify weak spots. Presently this is done with tabnine, etc. 

# What is Idempotence & Determinism? 
The primary "preferred" pattern for operational code is to operate with as much idempotence (isolation) as possible.  Also to have the option of both deterministic and non-deterministic business/application logic. 

https://en.wikipedia.org/wiki/Idempotence
Idempotence (UK: /ˌɪdɛmˈpoʊtəns/,[1] US: /ˌaɪdəm-/)[2] is the property of certain operations in mathematics and computer science whereby they can be applied multiple times without changing the result beyond the initial application. 

A deterministic algorithm is an algorithm which, given a particular input, will always produce the same output, with the underlying machine always passing through the same sequence of states.

# Python
Python severless backend functions with a plurality of messaging & socket options for IPC.  Logging (Azure Insight), Authentication, scaffold is Azure Insight.  Future infrastructure for algorithimic pattern monitoring (effectively an AI regression that determines if the code is working)

# Typescript - Vite, Vue, Vitesse =>  Vitestestee
[Vitestestee](http://github.com/elasticdotventures/vitestestee) is a sample project based on Vite, Vue, Vitesse

# OpenID, MS Graph, MS ADSLv2


# 
Using Azure Functions, and Azure Logic Apps for orchestrating actions which allows a _b00t_ stack to behave as a globally distributed finite-state machine.   This is the higest level of durability which can be assigned to a software platform and is suitable to running fail-safe systems such as nuclear reactors. The author explicitly disclaims any responsibility for circumstances occurring decide to use _b00t_ to run your own backyard reactor.

https://en.wikipedia.org/wiki/Deterministic_system


# What is so Opionated? 
0MG. _b00t_ tries very hard to be Templates and Tools ("TnT") but inevitably through the selection of those it's opinions on "best" approach.  For example, snapd packages are at the core of ubuntu, and for various reasons ubuntu is the base image.   Even if you start at alpine Linux it's going to look very ubuntu-ish if you use _b00t_. 

The organizational pattern is formatted around a cross-competency, "Don't make me think" (any more than I need to) so it assigns emojis to meanings.  

This allows for the system to implement "story tell" during logs, showing entire transactions as a series of pictograms (colorful markov chains). Here is a sample of the _projects_ layout opinion: 

```
Here are the _projects_ opinion: 

/hoome/.b00t/    # your configs
|- ./your_Project/..   # each project has it's own directory. 

/c0de/
|- /_b00t_    # where your projects live
|- /project/  # your configs. 

# 🤓 NOTE:
#   improve security posture: make upper level filesystems
#   readonly and removing configs from lower levels using 
#   docker "dive"

# Files:

/c0de/*                # rationale: 4 characters.  
 |- ./01-start.sh      # 🍰 ** Start Here!! Run this ./01-start.sh **
 
/c0de/_b00t_           # contains the repo. PUT this repo here.  
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

By subscribing to this pattern, an effort is made to obviate certain things.  
Layers are built upon layers. 
For example a deployed system can be wiped of Dockerfiles using:

```
rm -Rf ./Dockerfile ./docker.🐳
```

This is handy at later builds.  For example a GIT filesystem can be stripped of utilities that is no longer needed.  Once that is compressed at a Docker Buildx layer then that information has destroyed during the idempotent container creation. 

```
## Tools of _b00t_
* Git
* Bash
    * JQ - https://stedolan.github.io/jq/download/
    * YQ - 
    * FZF - 
* Python
* Node-Ts
* Docker
```

_b00t_ assumes the author will (ultimately) decide to end up using a combination of stateful logic so it simplifies the interface to those by creating a unified command language that can be further build on.  There is a method to the madness, I assure you.  The patterns utilize serverless consumption plans whenever possible.  The plan is to eventually include complete VS code project files & plugin.    This assumes the developer(s) are using a three stage release model, "InnerLoop" (Local), "OuterLoop" (Cloud and/or Local), "Production" (Live) each of those moving the data to the cloud and toward the public, no attempts are made. 

# Want to see examples Emoji & HSK1 Chinese
https://brianhorakh.medium.com/emoji-logging-warning-much-silliness-ahead-4cae73d7089


``txt
/c0de/_b00t_                     : this bootstrap code.
/c0de/_b00t_/01-start.sh         : setups up environment
/c0de/_b00t_/02-project.sh       : create a new project, with tooling. 
/c0de/_b00t_/ZZ-remove.sh        : clean up a project 
``


## Get Started: 
NOTE: Someday (soon-ish) I'll have this as a DEPLOY to AZURE button working!

```bash
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

\_b00t_ will create your project in /c0de/$my_project_id
In the future to upgrade \_b00t_ you can simply use GIT. 
```

```
# Or clone AiiA! 
AiiA is the tool being built in _b00t_, it's an NLP call center application. 
(http://elasticdotventures/aiia-callcenter)


#  When Finished:
you'll have a fully integrated development environment with secure language bindings to two languages, full permission provision, resources with budget-friendly serverless consumption models by default. 

# To cleanup:
**NOT FINISHED**
```
/c0de/_b00t_/ZZ-cleanup.sh $my_project_id
```

# Emojis & Chinese on the CLI
The author is a hardcore CLI guy too.  For some things using your mouse to copy-paste is better since it eliminates fat fingers. Let's keep it real - nobody except absolutely masochists would try to hand-type AZ Resource strings, so \_b00t_ strings are no different.  You will need to do some cutting and pasting, you probably won't like \_b00t_ if you're developing on a Digital VT100 .. 🙄 yeah, nah. 

## How to move around \_b00t_ on the CLI
You use the CLI? You're 1337! 0k4y. Hack your brain to use tab complete & wildcard shortcuts. 

For directories with emojis or mixed case, use wildcards to hit targets. 
So `cd /c0*/` will chdir to `/c0de/`

Generally the targets use Emoji & HSK at the end, but as an exercise, here's a badly named directory: 
`/c0de/_b00t_/.../蓝色_Bicep_ARM_AzrResMgr.💪`

could be accessed from it's pwd using ANY of the following `cd` command. 
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

# _b00t_: what i'm building.
  WIZARD-MENU/CLI SYSTEM-ENGINEERING POLY-INTERFACE "GLUE" of many BEST PRACTICES for anybody that desires a "DONT MAKE ME THINK MORE THAN I NEED TO" KISS approach to TECHNICAL SYSTEMS ENGINEERING accomplished through EASY-IDEMPOTENT DETERMINISTIC (meaning predictable) CONTAINER DESIGN/INTEGRATION/LIFECYCLE.  

  A FULL LIFECYCLE OPEN-SOURCE SYSTEMS TOOL:
    INCEPTION, CREATION, AND DETERMINISTIC OPERATION FROM DEV TO RELEASE => [ COMPLIANCE, LOGGING, CYBER-SECURITY ] TO EXIT 



Future:
https://unikraft.org/

