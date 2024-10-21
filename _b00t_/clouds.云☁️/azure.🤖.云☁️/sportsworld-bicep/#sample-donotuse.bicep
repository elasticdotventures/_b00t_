
// adsf
// az deploy group create -f ./filename.bicep -g my-rg

// Tutorial create and deploy first Azure Resource Manager bicep file
// https://docs.microsoft.com/en-us/azure/azure-resource-manager/templates/bicep-tutorial-create-first-bicep?tabs=azure-powershell

// https://docs.microsoft.com/en-us/azure/azure-resource-manager/templates/export-template-portal
// go to a service, 


// https://github.com/Azure/bicep/blob/main/docs/spec/resource-scopes.md
//
// GLOBAL FUNCTIONS
// tenant() // returns the tenant scope

// managementGroup() // returns the current management group scope (only from managementGroup deployments)
// managementGroup(name: string) // returns the scope for a named management group

// subscription() // returns the subscription scope for the current deployment (only from subscription & resourceGroup deployments)
// subscription(subscriptionId: string) // returns a named subscription scope (only from subscription & resourceGroup deployments)

// resourceGroup() // returns the current resource group scope (only from resourceGroup deployments)
// resourceGroup(resourceGroupName: string) // returns a named resource group scope (only from subscription & resourceGroup deployments)
// resourceGroup(subscriptionId: string, resourceGroupName: string) // returns a named resource group scope (only from subscription & resourceGroup deployments)
targetScope = 'resourceGroup'
param RGname string = 'sportsworldas2rg'

// deploy a resource group to the subscription scope
resource myRg 'Microsoft.Resources/resourceGroups@2020-10-01' = {
  name: RGname
  location: 'eastus'
  subscription: subscription()
}

param image string = 'elasticdotventures.azurecr.io/sportsworldapp:latest'
param port int = 8000
param cpuCores int = 1
param memoryinGb int = 2

@allowed([
  'Always'
  'Never'
  'OnFailure'
])
param restartPolicy string = 'Always'
param location string = resourceGroup().location


resource storageGroup 'Microsoft.Storage/storageAccounts@2019-06-01' = {
  name: '{provide-unique-name}'
  location: 'eastus'
  sku: {
    name: 'Standard_LRS'
  }
  kind: 'StorageV2'
  properties: {
    supportsHttpsTrafficOnly: true
  }
}

resource containerGroup 'Microsoft.ContainerInstance/containerGroups@2021-03-01' = {
  name: 'sportsworldapp'
  location: location
  properties: {
    containers: [
      {
        name: name
        properties: {
          image: image
          ports: [
            {
              port: port
              protocol: 'TCP'
            }
          ]
          resources: {
            requests: {
              cpu: cpuCores
              memoryInGB: memoryinGb
            }
          }
        }
      }
    ]
    osType: 'Linux'
    restartPolicy: restartPolicy
    ipAddress: {
      type: 'Public'
      ports: [
        {
          port: port
          protocol: 'TCP'
        }
      ]
    }
  }
}

output containerIPv4Address string = containerGroup.properties.ipAddress.ip

