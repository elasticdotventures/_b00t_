#!/bin/bash


# FUTURE: almost ready to start install requirements, for python

source "/c0de/_b00t_/_b00t_.bashrc"

txtFiles=( `/usr/bin/fdfind --color=always -t f '\.txt$'` )
for file in ${txtFiles[@]}; do
    #if [ gr]
    # echo "f: $file"
    # https://www.linuxjournal.com/content/pattern-matching-bash
    isStage=$(echo \"$file\" | grep -c ".层_")

    pipIt=false
    if [ "$isStage" -eq 1 ] ; then 
        # stage file handler
        pipIt=true
    elif [ $(basename "$file") == "requirements.txt" ]; then 
        # is a requirements file
        pipIt=true
    fi

    if [ $pipIt = true ]; then 
        echo "pipIt! $file"
        pip3 install -r $file
    fi
done

