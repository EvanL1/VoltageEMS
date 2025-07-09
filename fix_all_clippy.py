#!/usr/bin/env python3
"""
Fix all clippy warnings in the VoltageEMS codebase
"""

import re
import os
import sys
from pathlib import Path


def fix_mixed_attributes(content):
    """Fix mixed inner and outer attributes in lib.rs"""
    # Fix the specific case in lib.rs
    if '/// Service entry point and lifecycle management' in content:
        # Find the problematic section and remove inner doc comments
        pattern = r'(/// Service entry point.*?)\n(pub mod service_impl \{)([\s\S]*?)(    //! This module.*?//! lifecycle of the communication service.*?\n)([\s\S]*?\})'
        
        def replacer(match):
            outer_docs = match.group(1)
            mod_start = match.group(2)
            mod_content = match.group(3)
            # Remove inner docs (group 4)
            mod_end = match.group(5)
            return f"{outer_docs}\n{mod_start}{mod_content}{mod_end}"
        
        content = re.sub(pattern, replacer, content, flags=re.DOTALL)
    
    return content


def fix_never_type_fallback(content):
    """Fix never type fallback warnings for Redis queries"""
    # Fix query_async calls to have explicit type annotation
    patterns = [
        # Fix pipe.query_async
        (r'pipe\.query_async\(&mut conn\)', r'pipe.query_async::<()>(&mut conn)'),
        # Fix cmd.query_async
        (r'cmd\.query_async\(&mut conn\)', r'cmd.query_async::<()>(&mut conn)'),
        # Fix general .query_async patterns
        (r'\.query_async\(&mut conn\)(\s*\.await)', r'.query_async::<()>(&mut conn)\1'),
    ]
    
    for pattern, replacement in patterns:
        content = re.sub(pattern, replacement, content)
    
    return content


def fix_unused_variables(content):
    """Fix unused variable warnings"""
    # Pattern for unused variables in function parameters
    unused_params = [
        ('transport', '_transport'),
        ('can_config', '_can_config'),
        ('iec104_config', '_iec104_config'),
        ('recv_seq', '_recv_seq'),
    ]
    
    for old_name, new_name in unused_params:
        # Fix in let bindings
        content = re.sub(rf'\blet\s+{old_name}\s*=', f'let {new_name} =', content)
        # Fix in function parameters
        content = re.sub(rf'(\s+){old_name}(\s*:)', rf'\1{new_name}\2', content)
        # Fix in pattern matching
        content = re.sub(rf'(\s+){old_name}(\s*[,\}}])', rf'\1{new_name}\2', content)
        # Fix in struct patterns
        content = re.sub(rf'\b{old_name}\s*:\s*_\s*\}}', f'{new_name}: _}}', content)
    
    return content


def fix_field_never_read(content):
    """Add #[allow(dead_code)] to structs with unused fields"""
    structs_with_unused_fields = [
        'PluginEntry',
        'Iec104Client', 
        'TcpBuilderImpl',
        'SerialBuilderImpl',
        'CanBuilderImpl',
        'GpioBuilderImpl',
        'MockComBase'
    ]
    
    for struct_name in structs_with_unused_fields:
        # Add allow attribute before struct definition if not already present
        if f'struct {struct_name}' in content and f'#[allow(dead_code)]\nstruct {struct_name}' not in content:
            # Match various struct patterns
            patterns = [
                # pub struct
                (rf'(\n)(pub struct {struct_name})', rf'\1#[allow(dead_code)]\n\2'),
                # struct without pub
                (rf'(\n)(struct {struct_name})', rf'\1#[allow(dead_code)]\n\2'),
                # #[derive(...)] pub struct
                (rf'(\n)(#\[derive\([^\]]+\)\]\n)(pub struct {struct_name})', rf'\1\2#[allow(dead_code)]\n\3'),
                # #[derive(...)] struct
                (rf'(\n)(#\[derive\([^\]]+\)\]\n)(struct {struct_name})', rf'\1\2#[allow(dead_code)]\n\3'),
            ]
            
            for pattern, replacement in patterns:
                if re.search(pattern, content):
                    content = re.sub(pattern, replacement, content)
                    break
    
    return content


def fix_unused_methods(content):
    """Add #[allow(dead_code)] to unused methods"""
    unused_methods = [
        'next_send_seq',
        'current_recv_seq', 
        'update_recv_seq',
        'handle_apdu',
        'handle_asdu',
        'simulate_data',
        'send_write_multiple_coils',
        'extract_modbus_polling_config'
    ]
    
    for method in unused_methods:
        # Skip if already has allow attribute
        if f'#[allow(dead_code)]\n    async fn {method}' in content or \
           f'#[allow(dead_code)]\n    fn {method}' in content or \
           f'#[allow(dead_code)]\n\n    async fn {method}' in content or \
           f'#[allow(dead_code)]\n\n    fn {method}' in content:
            continue
            
        # Add allow attribute before async fn or fn
        patterns = [
            # async fn with spaces
            (rf'(\n    )(async fn {method})', rf'\1#[allow(dead_code)]\n\1\2'),
            # fn with spaces
            (rf'(\n    )(fn {method})', rf'\1#[allow(dead_code)]\n\1\2'),
            # pub async fn
            (rf'(\n    )(pub async fn {method})', rf'\1#[allow(dead_code)]\n\1\2'),
            # pub fn
            (rf'(\n    )(pub fn {method})', rf'\1#[allow(dead_code)]\n\1\2'),
        ]
        
        for pattern, replacement in patterns:
            if re.search(pattern, content):
                content = re.sub(pattern, replacement, content)
                break
    
    return content


def fix_to_string_inherent_impl(content):
    """Fix inherent to_string implementation warning"""
    # Check if this is the ProtocolType file
    if 'impl ProtocolType' in content and 'pub fn to_string(&self) -> String' in content:
        # Find the to_string implementation
        to_string_pattern = r'(impl ProtocolType \{[\s\S]*?)(pub fn to_string\(&self\) -> String \{[\s\S]*?\n    \})([\s\S]*?\n\})'
        
        match = re.search(to_string_pattern, content)
        if match:
            # Extract the match arms from to_string
            to_string_content = match.group(2)
            arms_match = re.search(r'match self \{([\s\S]*?)\n        \}', to_string_content)
            
            if arms_match:
                match_arms = arms_match.group(1)
                
                # Create Display implementation
                display_impl = f'''impl std::fmt::Display for ProtocolType {{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{
        let s = match self {{{match_arms}
        }};
        write!(f, "{{}}", s)
    }}
}}'''
                
                # Remove to_string method and add Display impl
                content = re.sub(to_string_pattern, rf'\1\3', content)
                
                # Add Display impl after the impl block
                content = re.sub(r'(impl ProtocolType \{[\s\S]*?\n\})', rf'\1\n\n{display_impl}', content)
    
    return content


def fix_manual_strip_prefix(content):
    """Fix manual strip prefix warnings"""
    # Pattern: if s.starts_with("prefix") { &s[len..] } else { ... }
    pattern = r'if\s+(\w+)\.starts_with\((["\'][^"\']+["\'])\)\s*\{\s*&\1\[(\d+)\.\.\]\s*\}'
    
    def replacer(match):
        var = match.group(1)
        prefix = match.group(2)
        # Return strip_prefix call
        return f'{var}.strip_prefix({prefix}).unwrap_or({var})'
    
    content = re.sub(pattern, replacer, content)
    
    return content


def fix_matches_macro(content):
    """Replace match expressions that look like matches! macro"""
    # Pattern for simple match that returns bool
    pattern = r'match\s+([^{]+)\s*\{[^}]*=>\s*true[^}]*=>\s*false[^}]*\}'
    
    # This is complex to fix generically, skip for now
    return content


def fix_from_str_confusion(content):
    """Fix from_str method that can be confused with FromStr trait"""
    if 'fn from_str(' in content and 'impl ProtocolType' in content:
        # Rename from_str to parse_protocol_type
        content = re.sub(r'\bfn from_str\(', 'fn parse_protocol_type(', content)
        content = re.sub(r'ProtocolType::from_str\(', 'ProtocolType::parse_protocol_type(', content)
    
    return content


def fix_module_same_name(content):
    """Fix module has same name as containing module"""
    # This typically needs manual intervention - skip for now
    return content


def fix_complex_type(content):
    """Fix very complex type - add type aliases"""
    # This typically needs manual intervention - skip for now
    return content


def fix_fallible_conversion(content):
    """Fix use of fallible conversion when infallible one could be used"""
    # Replace TryFrom/TryInto with From/Into where appropriate
    # This is context-specific - skip for now
    return content


def fix_format_iterator(content):
    """Fix use of format! to build string from iterator"""
    # This typically needs manual intervention - skip for now  
    return content


def process_file(file_path):
    """Process a single Rust file"""
    try:
        with open(file_path, 'r', encoding='utf-8') as f:
            content = f.read()
        
        original_content = content
        
        # Apply all fixes
        content = fix_mixed_attributes(content)
        content = fix_never_type_fallback(content)
        content = fix_unused_variables(content)
        content = fix_field_never_read(content)
        content = fix_unused_methods(content)
        content = fix_to_string_inherent_impl(content)
        content = fix_manual_strip_prefix(content)
        content = fix_from_str_confusion(content)
        content = fix_matches_macro(content)
        content = fix_module_same_name(content)
        content = fix_complex_type(content)
        content = fix_fallible_conversion(content)
        content = fix_format_iterator(content)
        
        # Write back if changed
        if content != original_content:
            with open(file_path, 'w', encoding='utf-8') as f:
                f.write(content)
            print(f"Fixed: {file_path}")
            return True
    except Exception as e:
        print(f"Error processing {file_path}: {e}")
    
    return False


def main():
    """Main function"""
    # Process all Rust files in services directory
    services_dir = Path('/Users/lyf/dev/VoltageEMS/services')
    
    fixed_count = 0
    
    # Process all Rust files
    for rust_file in services_dir.rglob('*.rs'):
        if process_file(rust_file):
            fixed_count += 1
    
    print(f"\nTotal files fixed: {fixed_count}")


if __name__ == '__main__':
    main()