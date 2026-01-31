

Pachyderm Documentation - Use Pachctl Shell
The Pachyderm Shell is a special-purpose shell for Pachyderm that provides auto-suggesting as you type. New Pachyderm…docs.pachyderm.com


# https://docs.pachyderm.com/latest/getting_started/local_installation/
minikube start
pachctl deploy local
pachctl deploy local - dry-run > pachyderm.json
Pachyderm is launching. Check its status with "kubectl get all"
Once launched, access the dashboard by running "pachctl port-forward"

# then:
pachctl port-forward
minikube ip
pachctl config update context `pachctl config get active-context` --pachd-address=192.168.49.2:3000



pachctl create repo b00t

