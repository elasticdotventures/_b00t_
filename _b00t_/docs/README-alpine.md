Alpine is cloud-Android

_b00t_ attempts to support Alpine deployments, however
the resulting system will look a lot like ubuntu. 

```
docker container run -it alpine /bin/sh
```

* Presently alpine is not supported because of webi requirement
https://github.com/webinstall/webi-installers/issues/259

```
apk --no-cache add curl bash
bash 
curl -sS https://webinstall.dev/webi | bash
webi 
```


