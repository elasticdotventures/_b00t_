// this is the main file which sets up the sportsworlds instances




// set the target scope for this file
// targetScope = 'subscription'

// https://docs.microsoft.com/en-us/azure/templates/

// param location string = 'eastus'
var location = resourceGroup().location

// deploy a resource group to the subscription scope
// resource myRg 'Microsoft.Resources/resourceGroups@2020-10-01' = {
//  name: 'sportsworldtest-rg'
//  location: location
//  properties: {
//  }
//}

// deploy a module to that newly-created resource group
// module myMod './key-vault-create.bicep' = {
  // name: 'sportsworldAs2Secrets'
  // scope: myRg
// }


param containerNameSAR array = [
  'dogs'
  'cats'
  'fish'
]


//resource blob 'Microsoft.Storage/storageAccounts/blobServices/containers@2019-06-01' = [for name in containerNameSAR: {
//  name: '${stgA.name}/default/${name}'
//  // dependsOn will be added when the template is compiled
//}]

///// include a file:
// module stgA './storageDeploy.bicep' = {
// name: 'storageDeploy'
//  params: {
//    storageAccountName: 'swuniquestorage001'
//  }
//}

param deployments array = [
  'foo'
  'bar'
]
module stgB './storageDeploy.bicep' = [for item in deployments: {
  name: 'storageDeploy${item}'
  params: {
    location: resourceGroup().location
    storageAccountName: 'swuniquestorage001${item}'   
  }
}]

