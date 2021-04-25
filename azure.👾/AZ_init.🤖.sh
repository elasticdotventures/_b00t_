

# safely initialize _b00t_ bash
if [ `type -t "_b00t_init_🚀_开始"` == "function" ]; then 
    # detect _b00t_ environment 
    _b00t_init_🚀_开始
fi

az account set -s NAME_OR_ID

az local-context 

# 怎么样 Get
AZURE_VALID_REGIONS=`$AZ_CLI account list-locations --query '[].[name]' --output table`

