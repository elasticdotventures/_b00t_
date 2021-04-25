# syntax=docker/dockerfile:1

## * * * *// 
#* Purpose: 🦄 b00tstraps node & typescript
#* should be called directly from ./01-start.sh 
## * * * *\\

# safely initialize _b00t_ bash
if [ `type -t "_b00t_init_🥾_开始"` == "function" ]; then 
    # detect _b00t_ environment 
    _b00t_init_🥾_开始
fi

npm i -D foy

## Yeoman is awesome. Going to use this: 
npm install -g yo generator-code

