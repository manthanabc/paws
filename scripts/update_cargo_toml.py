import os
import toml

MERGED_CRATES = [
    "paws_spinner", "paws_display", "paws_select", "paws_stream",
    "paws_walker", "paws_tracker", "paws_json_repair", "paws_template",
    "paws_snaps", "paws_test_kit", "paws_fs"
]

def update_deps(deps):
    if not deps:
        return False
    
    modified = False
    to_remove = []
    
    for crate in MERGED_CRATES:
        if crate in deps:
            to_remove.append(crate)
            modified = True
            
    for crate in to_remove:
        del deps[crate]
        
    if modified:
        deps["paws_common"] = {"workspace": True}
        
    return modified

def process_file(path):
    try:
        with open(path, 'r') as f:
            data = toml.load(f)
            
        modified = False
        
        if "dependencies" in data:
            if update_deps(data["dependencies"]):
                modified = True
                
        if "dev-dependencies" in data:
            if update_deps(data["dev-dependencies"]):
                modified = True
                
        # Also check target specific dependencies if any (simplified)
        
        if modified:
            print(f"Updating {path}")
            with open(path, 'w') as f:
                toml.dump(data, f)
                
    except Exception as e:
        print(f"Error processing {path}: {e}")

def main():
    for root, dirs, files in os.walk("crates"):
        for file in files:
            if file == "Cargo.toml":
                process_file(os.path.join(root, file))

if __name__ == "__main__":
    main()
