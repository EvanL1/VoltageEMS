#!/usr/bin/env python3
"""Fix format string issues for clippy::uninlined_format_args"""

import re
import os
import sys

def fix_format_strings(file_path):
    """Fix format strings in a Rust file"""
    with open(file_path, 'r') as f:
        content = f.read()
    
    original_content = content
    
    # Pattern to match format strings with single argument
    # Match patterns like: "text {}", var  or "text {:?}", var
    patterns = [
        # Simple placeholder
        (r'(".*?)\{\}(".*?),\s*([a-zA-Z_][a-zA-Z0-9_]*)\s*\)', r'\1{\3}\2)'),
        # Debug placeholder
        (r'(".*?)\{:(\?+)\}(".*?),\s*([a-zA-Z_][a-zA-Z0-9_]*)\s*\)', r'\1{\4:\2}\3)'),
        # Hex placeholder
        (r'(".*?)\{:02X\}(".*?),\s*([a-zA-Z_][a-zA-Z0-9_]*)\s*\)', r'\1{\3:02X}\2)'),
        (r'(".*?)\{:04X\}(".*?),\s*([a-zA-Z_][a-zA-Z0-9_]*)\s*\)', r'\1{\3:04X}\2)'),
        # Width placeholder
        (r'(".*?)\{:(\d+)\}(".*?),\s*([a-zA-Z_][a-zA-Z0-9_]*)\s*\)', r'\1{\4:\2}\3)'),
        # Float precision
        (r'(".*?)\{:\.(\d+)f\}(".*?),\s*([a-zA-Z_][a-zA-Z0-9_]*)\s*\)', r'\1{\4:.\2}\3)'),
    ]
    
    for pattern, replacement in patterns:
        content = re.sub(pattern, replacement, content)
    
    # Special case for multiple arguments - only fix if there's exactly one placeholder
    # This is more complex and needs careful handling
    
    if content != original_content:
        with open(file_path, 'w') as f:
            f.write(content)
        return True
    return False

def main():
    """Main function"""
    fixed_count = 0
    
    # Walk through all Rust files in services/comsrv/src
    for root, dirs, files in os.walk('services/comsrv/src'):
        for file in files:
            if file.endswith('.rs'):
                file_path = os.path.join(root, file)
                if fix_format_strings(file_path):
                    print(f"Fixed: {file_path}")
                    fixed_count += 1
    
    print(f"\nTotal files fixed: {fixed_count}")

if __name__ == "__main__":
    main()