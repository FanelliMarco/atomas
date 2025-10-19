#!/bin/bash

# Function to delete all Android Virtual Devices (AVDs)
delete_all_avds() {
    echo "Checking for existing AVDs..."
    
    # Get list of AVD names
    avd_list=$(avdmanager list avds | grep "Name:" | sed 's/.*Name: //')
    
    # Check if any AVDs exist
    if [ -z "$avd_list" ]; then
        echo "No AVDs found."
        return 0
    fi
    
    echo "Found the following AVDs:"
    echo "$avd_list"
    echo ""
    
    # Ask for confirmation
    read -p "Are you sure you want to delete ALL AVDs? This action cannot be undone. (y/N): " confirm
    
    if [[ ! $confirm =~ ^[Yy]$ ]]; then
        echo "Operation cancelled."
        return 1
    fi
    
    echo ""
    echo "Deleting AVDs..."
    
    # Delete each AVD
    while IFS= read -r avd_name; do
        if [ -n "$avd_name" ]; then
            echo "Deleting AVD: $avd_name"
            avdmanager delete avd -n "$avd_name"
            
            if [ $? -eq 0 ]; then
                echo "✓ Successfully deleted: $avd_name"
            else
                echo "✗ Failed to delete: $avd_name"
            fi
            echo ""
        fi
    done <<< "$avd_list"
    
    echo "AVD deletion process completed."
}

# Function to list AVDs (helper function)
list_avds() {
    echo "Current AVDs:"
    avdmanager list avds
}

# Main execution
case "${1:-}" in
    "list")
        list_avds
        ;;
    "delete")
        delete_all_avds
        ;;
    *)
        echo "Usage: $0 {list|delete}"
        echo ""
        echo "Commands:"
        echo "  list   - List all existing AVDs"
        echo "  delete - Delete all existing AVDs (with confirmation)"
        echo ""
        echo "Examples:"
        echo "  $0 list"
        echo "  $0 delete"
        exit 1
        ;;
esac
