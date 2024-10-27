param namePrefix string
param sku string = 'B1'

resource appPlan 'Microsoft.Web/serverfarms@2020-10-01' = {
  name: '${namePrefix}appPlan'
  location: resourceGroup().location
  kind: 'linux'
  sku:{
    name: sku
  }
  properties:{
    reserved: true
  }
}

output planId string = appPlan.id

// adsf
// az deploy group create -f ./filename.bicep -g my-rg
