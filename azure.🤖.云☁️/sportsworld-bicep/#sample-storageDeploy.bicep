param storageAccountName string
param location string

resource stg 'Microsoft.Storage/storageAccounts@2019-06-01' = {
  name: storageAccountName // must be globally unique
  location: location
  kind: 'Storage'
  sku: {
    name: 'Standard_LRS'
  }
}

