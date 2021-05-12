

# safely initialize _b00t_ bash
source "/c0de/_b00t_/_b00t_.bashrc"

#az login
#az login --use-device-code

## 项目 * * * * \\  
# (Xiàngmù) Project Id
export c0re_pr0j3ct_id=`project`
##* * * * //

## !TODO: Do you need a project name?
## !TODO: Do we have an AZ tenant Id?
## 要不要　
## !TODO: Do you need a resource Group?
## !TODO: 
🐙

##* * * * \\
az_resGroupId=$(az group show --name $az_groupName --query id --output tsv)
# $echo groupId
# /subscriptions/{###}/resourceGroups/{groupName}
az ad sp create-for-rbac \
  # --scopes  # !TODO
  --scope $az_resGroupId --role Contributor \
  --name $az_projectId-🤴校长_principal \
  --sdk-auth
##* * * * //





