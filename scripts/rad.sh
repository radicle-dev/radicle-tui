#!/bin/bash
extract_operation() {
    local op=$(echo $1 | jq '.operation')
    op=${op//\"/""}
    
    echo "$op"
}

extract_id() {
    local ids=$(echo $1 | jq '.ids')
    local id=$(echo $ids | jq '.[0]')
    id=${id//\"/""}
    
    echo "$id"
}

if [[ "$1" == "patch" ]] || [[ "$1" == "issue" ]] || [[ "$1" == "inbox" ]]; then
    if [[ -n "$2" ]]; then
        if [[ "$2" == "--tui" ]]; then
            # Run TUI
            { out=$(rad-tui $1 select 2>&1 >&3 3>&-); } 3>&1
            if [[ "$out" == "" ]]; then
                exit 1
            fi
            
            op=$(extract_operation $out)
            id=$(extract_id $out)
            
            echo $op
            echo $id
            
            rad $1 $op $id
        else
            # Run TUI
            args="--mode id"
            { out=$(rad-tui $1 select $args 2>&1 >&3 3>&-); } 3>&1
            id=$(extract_id $out)
            
            args=("$@")
 
            rad $1 $2 $id ${args[@]:2}
        fi
    else
        rad $@
    fi
else
    rad $@
fi