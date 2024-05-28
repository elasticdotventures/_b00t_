## * * * *// 
#* Purpose: 🏎️ b00tstraps go
#* should be called directly from ./01-start.sh 
## * * * *\\

#* 进口v2 🥾 ALWAYS load c0re Libraries!
source "./_b00t_.bashrc"

# future, go support. podman needed go.

## step2B: install/upgrade go
# https://www.vultr.com/docs/install-the-latest-version-of-golang-on-ubuntu
#WORKDIR /tmp
#RUN wget https://golang.org/dl/go1.16.3.linux-amd64.tar.gz
#RUN tar -C /usr/local -xzf go1.16.3.linux-amd64.tar.gz
#RUN echo "export PATH=$PATH:/usr/local/go/bin" >> ~/.profile
#RUN echo "export GOPATH=~/.go" >> ~/.profile
#RUN /bin/sh ~/.profile
#RUN rm -f /usr/bin/go
#RUN ln -s /usr/local/go/bin/go /usr/bin/go



# FUTURE: https://github.com/peergos/peergos

# export JAVA_HOME="$(/usr/libexec/java_home 2>/dev/null)"
