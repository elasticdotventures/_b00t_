####
# CRUDINI examples
# 🤓 https://github.com/pixelb/crudini/blob/master/EXAMPLES
# CRUDINI is used to store b00t config:

if n0ta_xfile_📁_好不好 "/usr/bin/crudini" ; then
    log_📢_记录 "🥳 need crudini to save data, installing now"
    $SUDO_CMD apt-get install -y crudini bc
fi

## CRUDINI helper functions:
function crudini_set() {
    local args=("$@")
    local topic=${args[0]}
    local key=${args[1]}
    local value=${args[2]}
    crudini --set $CRUDINI_CFGFILE "${topic}" "${key}" "${value}"
    return $?
}



function crudini_get() {
    local args=("$@")

    #if [[ "$#" -ne "2" ]] ; then
    #    log_📢_记录 "crudini_get topic key"
    #    exit 0
    # fi

    local topic=${args[0]}
    local key=${args[1]}
    echo $( crudini --get "$CRUDINI_CFGFILE" "${topic}" "${key}" )
    return $?
}

# _seq: get a number from a sequence in b00t
function crudini_seq() {
    local args=("$@")
    local seqlabel=${args[0]}

    local x=$( crudini_get "b00t" "$seqlabel" )
    if [ -z "$x" ] ; then x="0"; fi
    x=$(echo "$x" + 1 | bc)
    crudini_set "b00t" "$seqlabel" "$x"
    echo $x
    return 0
}

# verify integrity of crudini system
function crudini_init() {
    export CRUDINI_CFGFILE=$(expandPath "~/.b00t/config.ini")
    local CRUDINI_DIR=`dirname $CRUDINI_CFGFILE`
    if [ ! -d "$CRUDINI_DIR" ] ; then
        log_📢_记录 "🐭 no local $CRUDINI_CFGFILE"
        log_📢_记录 "🐭🥳 local dir $CRUDINI_DIR"
        if [ ! -d "$CRUDINI_DIR" ] ; then
            log_📢_记录 "🐭 creating CRUDINI dir $CRUDINI_DIR"
            /bin/mkdir -p $CRUDINI_DIR
            /bin/chmod 750 $CRUDINI_DIR
            log_📢_记录 "🐭 init CRUDINI file $CRUDINI_CFGFILE"
            crudini --set $CRUDINI_CFGFILE '_seq' "1"
        else
            #local x=$( crudini_get "b00t" "crudini_check" )
            # x=$( [ -z "$x" ] && echo "0" )
            local x=$( crudini_seq "crudini_check" )
            log_📢_记录 "🐭😃CRUDINI _seq: #$x dir: $CRUDINI_DIR existed."
        fi
    fi
    return 0
}
#  creates an export for $CRUDINI_CFGFILE
crudini_init

function crudini_ok () {
if [ -f $CRUDINI_CFGFILE ] ; then
    x=$( crudini_seq "crudini_check" )
    log_📢_记录 "🐭🥾 CRUDINI _seq: #$x $CRUDINI_CFGFILE"
    return 0
else
    log_📢_记录 "🐭🍒 CRUDINI br0ked. file: $CRUDINI_CFGFILE"
    # todo: maybe some failsafe, i.e. redis or something.
    return 1
fi
}
crudini_ok


