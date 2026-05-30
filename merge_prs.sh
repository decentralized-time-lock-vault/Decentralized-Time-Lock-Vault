#!/bin/bash

# Array of PR numbers to merge
prs=(199 192 188 187 175 163 162 157 55)

for pr in "${prs[@]}"; do
    echo "================================"
    echo "Processing PR #$pr"
    echo "================================"
    
    # Fetch the PR branch
    git fetch origin "pull/$pr/head:pr-$pr" --force 2>/dev/null
    
    # Checkout the PR branch
    git checkout pr-$pr 2>/dev/null
    
    # Merge main into it
    if git merge main -m "Merge main into PR #$pr" 2>&1 | grep -q "conflict"; then
        echo "Conflicts detected in PR #$pr, resolving..."
        # Keep all changes from the PR branch
        git checkout --ours . 
        git add .
        git commit -m "Resolve conflicts - keep PR #$pr changes" --no-edit
    else
        echo "No conflicts in PR #$pr"
    fi
    
    # Push the resolved branch
    git push origin pr-$pr --force 2>&1 | tail -2
done

# Return to main
git checkout main
