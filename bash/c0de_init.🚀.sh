
## * * * *// 
#* Purpose: imports standard bash behaviors
#*          for consistent handling of parameters
#*
## * * * *//

IS_supported=`cat /etc/os-release | grep "Ubuntu 20.04.2 LTS"`
if [ -z "$IS_supported" ] ; then
    cat /etc/os-release
    echo "👽不支持  OS not yet supported." && exit 0
fi

function rand0 () {
    max="$1"
    rand0=$( bc <<< "scale=2; $(printf '%d' $(( $RANDOM % $max)))" ) ;
    # rand0=$( echo $RANDOM % $max ) ; 
    echo $rand0
}

rand0_result="$(rand0 100)"
echo $rand0_result