
// To Deploy: az deployment group create -f ./container-group.bicep -g sportsworldtest-rg 
// To Stop: az container stop -g sportsworldtest-rg -n myContainerGroup

// https://docs.microsoft.com/en-us/azure/templates/microsoft.containerinstance/containergroups?tabs=bicep#ImageRegistryCredential
// https://docs.microsoft.com/en-us/azure/container-instances/

targetScope = 'resourceGroup'
param location string = resourceGroup().location

resource symbolicname 'Microsoft.ContainerInstance/containerGroups@2019-12-01' = {
  name: 'myContainerGroup'
  location: location
  tags: {}
  identity: {
    // two types of identities:
    // * System Assigned and User Assigned. System Assigned are directly tied to the service you are using. In our case, the container group. The identities lifespan matches the lifespan of the container group.
    // * User Assigned identities allow for having an identity live past the lifetime of a single instance. These are created and can be used with multiple different services. This is especially great for container groups where an identity is create once with all the permissions, then many container groups can be quickly spun up to take advantage of the identity.
    type: 'SystemAssigned'
    // userAssignedIdentities: {}
  }
  properties: {
    containers: [
      {
        name: 'sportsworldapp1'
        properties: {
          image: 'elasticdotventures.azurecr.io/sportsworldapp'
          // command: [
          //  'string' // The command to execute within the init container in exec form. - string
          //]
          ports: [
            {
              protocol: 'TCP'
              port: 80
            }
            {
              protocol: 'TCP'
              port: 8000
            }
          ]
          environmentVariables: [
            {
              name: 'Variable1'
              value: 'VariableValue1'
              // secureValue: 'string'
            }
          ]
          resources: {
            requests: {
              memoryInGB: 2
              cpu: 1
              //gpu: {
              //  count: int
              //  sku: 'string'
              //}
            }
            //limits: {
            //  memoryInGB: any('number')
            //  cpu: any('number')
              // gpu: {
              //  count: int
              //  sku: 'string'
              // }
            //}
          }
          volumeMounts: [
            {
              name: 'cloudas2'
              mountPath: '/mnt/cloud-as2'
              readOnly: false
            }
          ]
          //livenessProbe: {
          //  exec: {
          //    command: [
          //      'string'
          //    ]
          //  }
          //  httpGet: {
          //    path: 'string'
          //    port: int
          //    scheme: 'string'
          //  }
          //  initialDelaySeconds: int
          //  periodSeconds: int
          //  failureThreshold: int
          //  successThreshold: int
          //  timeoutSeconds: int
          //}

          //readinessProbe: {
          //  exec: {
          //    command: [
          //      'string'
          //    ]
          //  }
          //  httpGet: {
          //    path: 'string'
          //    port: int
          //    scheme: 'string'
          //}
          //  initialDelaySeconds: int
          //  periodSeconds: int
          //  failureThreshold: int
          //  successThreshold: int
          //  timeoutSeconds: int
          //}
        }
      }
    ]
    imageRegistryCredentials: [
      {
        server: 'elasticdotventures.azurecr.io'
        username: 'elasticdotventures'
        password: ''
      }
    ]
    restartPolicy: 'Always'
    ipAddress: {
      ports: [
        {
          protocol: 'TCP'
          port: 80
        }
        {
          protocol: 'TCP'
          port: 8000
        }
      ]
      type: 'Public'

      // ip: 'string'   // The IP exposed to the public internet.
      dnsNameLabel: 'sportsworldapp' // The Dns name label for the IP.
    }
    osType: 'Linux'
    volumes: [
      {
        name: 'cloudas2'
        azureFile: {    // AzureFileVolume object
          shareName: 'cloud-as2' // The name of the Azure File share to be mounted as a volume.
          // readOnly: bool  // 
          storageAccountName: 'sportsworldchicago' // The name of the storage account that contains the Azure File share.
          storageAccountKey: 'gHnuEane/jQA4OSa6vN26vqUgNYTKiPpQu8zthOPpgTtB3Cj967F68+SDPKusUZVzOR2JtbnClk+1qtplP4S1g==' //  The storage account access key used to access the Azure File share.
        }
        //emptyDir: {}      // emptyDir ephermeral
        // secret: {}     // https://docs.microsoft.com/en-us/azure/container-instances/container-instances-volume-secret

        // gitRepo: { // https://docs.microsoft.com/en-us/dotnet/api/microsoft.azure.management.containerinstance.models.gitrepovolume?view=azure-dotnet
        //  directory: 'string'
        //  repository: 'string'
        //  revision: 'string'
        // }
      }
    ]

    //diagnostics: {
    //  logAnalytics: {
    //    workspaceId: 'string'
    //    workspaceKey: 'string'
    //    logType: 'string'
    //    metadata: {}
    //  }
    //}

    //networkProfile: {
    // bicep definition
    // https://docs.microsoft.com/en-us/azure/templates/microsoft.network/networkprofiles?tabs=json
    // https://docs.microsoft.com/en-us/azure/container-instances/container-instances-virtual-network-concepts      
    // There are three Azure Virtual Network resources required for deploying container groups to a virtual network: the virtual network itself, a delegated subnet within the virtual network, and a network profile.
    //  id: 'string'
    //}

    //dnsConfig: {
    //  nameServers: [
    //    '8.8.8.8'
    //    '8.8.4.4'
    //  ]
    //  // searchDomains: 'string'
    //  // options: 'string'
    //}
    sku: 'Standard'

    //encryptionProperties: {
    //  // https://docs.microsoft.com/en-us/dotnet/api/microsoft.azure.management.containerinstance.models.containergroup.encryptionproperties?view=azure-dotnet
    //  vaultBaseUrl: 'string'
    //  keyName: 'string'
    //  keyVersion: 'string'
    //}

    //initContainers: [   
      // these run to completion before the application container or containers start
      // https://kubernetes.io/docs/concepts/workloads/pods/init-containers/
      // https://docs.microsoft.com/en-us/azure/container-instances/container-instances-init-container
      // probably setting up tmpfs
      //  {
      //  name: 'sportsworldas2app'
      //  properties: {
      //    image: 'elasticdotventures.azurecr.io/sportsworldapp'
      //    command: [
      //      'string'
      //    ]
      //    environmentVariables: [
      //      {
      //      name: 'MyVariable1'
      //      value: 'string'
      //      secureValue: 'string'
      //      }
      //    ]
      //    volumeMounts: [
      //      {
      //        name: 'string'
      //        mountPath: 'string'
      //        readOnly: bool
      //      }
      //    ]
      //  }
      //}
    //]
  }
}
