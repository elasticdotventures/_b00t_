
# \_b00t_ is a Git Template

## What Makes Templates Special?
Forking a project brings all the history with it. 
Using a remote template creates a fresh snapshot as if the entire repository was created in one single commit.  If you keep your t00ling in _b00t_ or in place with wrappers in the b00t cli.  Then install modules as normal. 

## Fork vs. Base Template
The *preferred* method of integrating a new app is to
introduce b00t gently. Installing the libraries and treating your software as a project (in which case there is no reason to introduce _b00t_ beyond a clone to /c0de)

HOWEVER if you want to introduce b00t to your own tooling, then you'd want to merge the histories. 

create a new repo.
🍰 https://stackoverflow.com/questions/56577184/github-pull-changes-from-a-template-repository/56577320
🍰  https://saintgimp.org/2013/01/22/merging-two-git-repositories-into-one-repository-without-losing-file-history/

# Handy Git Commands
recover a file: 
```
git checkout $commit_id -- $file
```


# add _b00t_
git remote add template git@github.com:elasticdotventures/_b00t_.git
# download _b00t_ 
git fetch --all
# git merge template/_b00t_

```
# FFUT

git remote add template https://github.com/elasticdotventures/_b00t_
git fetch template

## GITVFS
# 💙🪟 micros0ft "git" VFS
https://github.com/microsoft/VFSForGit
GIT based local caching filesystem, it only downloads files
when they are accessed such that subsequent reads will be much 
faster. commits can be made in directories using branches?

### downloads
https://github.com/Microsoft/VFSForGit/releases

