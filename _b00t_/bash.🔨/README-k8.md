README-k8

_b00t_ believes k8, traditional kubernetes is for applications that work best in kubernetes, as compared to cloud-native serverless functions.

_b00t_ intends to support at least minikube and AKS. This

_b00t_ is also going to use Pachyderm for lineage control 

curl -o /tmp/pachctl.tar.gz -L https://github.com/pachyderm/pachyderm/releases/download/v1.13.2/pachctl_1.13.2_linux_amd64.tar.gz && tar -xvf /tmp/pachctl.tar.gz -C /tmp && sudo cp /tmp/pachctl_1.13.2_linux_amd64/pachctl /usr/local/bin


## k8 deployments
use cases
* creating a deployments
* updating a deployments
* rolling back a deployment
* scaling a deployments
* pausing and resume a deployments
* clean up restartPolicy
* canary deployments
* writing a deployment spec
* alternative to deployments

# namespaces


```
docker run -it -v ${PWD}/golang/configs/:/configs
```
## commands

# kubectl get secrets
application reads secrets from /secrets/very.json

```yaml
apiVersion: v1
kind: Secret
metadata:
    name: yourLogicalNameForThisSecret
type: Opaque
stringData:
    very.json: |-
        {
            "api_key": "someverysecretgoeshere"
        }
```
kubectl create secret generic mysecret --from-file .\linqua\secrets\secret.json

kubectl get secrets



# kubectl version
v1.21.1

# kubectl get pods
# kubectl get namespaces
always uses default namespace unless:  -n $NAMESPACE

# kubectl describe $PODID
```
kubectl describe pod $PODID -n $NAMESPACE
```
purpose: diagnostics

# kubectl logs

# kubectl get deployments
# kubectl get services
# kubectl get secrets
# kubectl get ingress
# kubectl get configmaps
```
kubectl apply configmap
kubectl create configmap example-config -f file .. 
```


kubectl config
    ~/.kube/config

cluster = https endpoint
user = credential??
context = cluster + user
KUBECONFIG environment or  --kubeconfig="/path/to/config"

kubectl get pods -o json



## k8 ConfigMap
kubectl apply -f .\kubernetes\configmaps\configmap.yaml
kubectl create configMap example-config --from-file /


# kubectl config file:
apiVersion: v1
clusters:
contexts: 
current-context:
kind: Config
preferences:{}
users:
- name: docker-Desktop 
  user: 
    client-certificate-data: mime
    client-key-data: mime

