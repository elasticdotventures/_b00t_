## * * * *//
## b00t-extra.bashrc: these libraries will collect functions used outside of init. 
## at this point only used by azure.

## select a project id
# export PROJECT_ID = 
function select_project_id() {
  local selected=$(for i in {0..5}
  do
    if [ "$i" -eq "0" ] ; then 
      echo "_input_"
    else 
      echo $( Pr0J3ct1D )
    fi
  done | fzf-tmux )
  if [ "$selected" = "_input_" ] ; then
    read -p "Pr0J3ct1D:" selected
  fi 
  echo $selected
  return $?
}



# i.e.
# fuzzy_chooser 
#function fuzzy_chooser() {
#    local args=("$@")
#    local function=${args[0]}
#    local topic=${args[1]}
#    local key=${args[2]}
# .. unfinished

## test to see how hard it is use fzf
function get_true_false() {
  echo "true
false" | fzf-tmux 
  return $?
}

## someday.. 
function emoji_menu() {
  # cat ../r3src_资源/inspiration.json | jq ".[]|[.symbol,.word] | @tsv" -r | fzf-tmux
  return 0 
}


