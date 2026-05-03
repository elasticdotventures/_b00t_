# https://apt.llvm.org/

## INSTRUCTIONS: run with sudo
## TODO: check for root.


#####
## llvm
# bash -c "$(wget -O - https://apt.llvm.org/llvm.sh)"
apt-get install clang-format clang-tidy clang-tools clang clangd libc++-dev libc++1 libc++abi-dev libc++abi1 libclang-dev libclang1 liblldb-dev libllvm-ocaml-dev libomp-dev libomp5 lld lldb llvm-dev llvm-runtime llvm python3-clang


wget -O - https://apt.llvm.org/llvm-snapshot.gpg.key | sudo apt-key add -
# or
wget -qO- https://apt.llvm.org/llvm-snapshot.gpg.key | sudo tee /etc/apt/trusted.gpg.d/apt.llvm.org.asc
# Fingerprint: 6084 F3CF 814B 57C1 CF12 EFD5 15CF 4D18 AF4F 7421



#!/bin/bash

# Add the LLVM APT repository to the system
sudo tee /etc/apt/sources.list.d/llvm.list > /dev/null << EOF
deb http://apt.llvm.org/noble/ llvm-toolchain-noble main
deb-src http://apt.llvm.org/noble/ llvm-toolchain-noble main
# 18
deb http://apt.llvm.org/noble/ llvm-toolchain-noble-18 main
deb-src http://apt.llvm.org/noble/ llvm-toolchain-noble-18 main
# 19
deb http://apt.llvm.org/noble/ llvm-toolchain-noble-19 main
deb-src http://apt.llvm.org/noble/ llvm-toolchain-noble-19 main
EOF

echo "LLVM repository added successfully."




## IGNORE:


apt-get install clang lld 
# 👆 clang-20 lld-20 can be added to automatically install the most recent version of the package.
# OR
# apt-get install clang-20 lldb-20 lld-20

# LLVM
apt-get install libllvm-20-ocaml-dev libllvm20 llvm-20 llvm-20-dev llvm-20-doc llvm-20-examples llvm-20-runtime
# Clang and co
apt-get install clang-20 clang-tools-20 clang-20-doc libclang-common-20-dev libclang-20-dev libclang1-20 clang-format-20 python3-clang-20 clangd-20 clang-tidy-20
# compiler-rt
apt-get install libclang-rt-20-dev
# polly
apt-get install libpolly-20-dev
# libfuzzer
apt-get install libfuzzer-20-dev
# lldb
apt-get install lldb-20
# lld (linker)
apt-get install lld-20
# libc++
apt-get install libc++-20-dev libc++abi-20-dev
# OpenMP
apt-get install libomp-20-dev
# libclc
apt-get install libclc-20-dev
# libunwind
apt-get install libunwind-20-dev
# mlir
apt-get install libmlir-20-dev mlir-20-tools
# bolt
apt-get install libbolt-20-dev bolt-20
# flang
apt-get install flang-20
# wasm support
apt-get install libclang-rt-20-dev-wasm32 libclang-rt-20-dev-wasm64 libc++-20-dev-wasm32 libc++abi-20-dev-wasm32 libclang-rt-20-dev-wasm32 libclang-rt-20-dev-wasm64
# LLVM libc
apt-get install libllvmlibc-20-dev

# Verification

file="llvm-toolchain-10_10.0.1~%2b%2b20210327072807%2bef32c611aa21-1~exp1~20210327183412.212.dsc"
url="https://apt.llvm.org/unstable/pool/main/l/llvm-toolchain-10/$file"
sig_file="$url.asc"
wget --quiet https://apt.llvm.org/sigstore.public.key
./rekor verify --rekor_server https://rekor.sigstore.dev --signature $sig_file --public-key sigstore.public.key --artifact $url
echo $?

