ZZ-cleanup.sh


sudo apt-get purge docker-ce docker-ce-cli containerd.io
sudo rm -rf /var/lib/docker
sudo rm -rf /var/lib/containerd

# don't leave any squid proxy laying around after build. 
rm -f /etc/apt/apt.conf.d/http_proxy_b00t_squid
