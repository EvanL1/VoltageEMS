#!/usr/bin/env python3
"""
Fix common clippy warnings in Rust code
"""

import re
import os
import sys
from pathlib import Path


def fix_unused_variables(content):
    """Fix unused variable warnings by prefixing with underscore"""
    # Pattern for unused variables
    patterns = [
        (r'let\s+(\w+)\s*=', r'let _\1 ='),  # Simple let bindings
        (r'(\s+)(\w+):\s*_\s*\}', r'\1\2: _\}'),  # Pattern matching with explicit _
    ]
    
    # Known unused variables from the errors
    unused_vars = ['transport', 'can_config', 'iec104_config', 'recv_seq']
    
    for var in unused_vars:
        # Fix let bindings
        content = re.sub(rf'let\s+{var}\s*=', f'let _{var} =', content)
        # Fix in pattern matching
        content = re.sub(rf'(\s+){var}(\s*[,\}}])', rf'\1_{var}\2', content)
    
    return content


def fix_field_never_read(content):
    """Fix field never read warnings"""
    # Add #[allow(dead_code)] to structs with unused fields
    structs_with_unused_fields = [
        'PluginEntry',
        'Iec104Client', 
        'TcpBuilderImpl',
        'SerialBuilderImpl',
        'CanBuilderImpl',
        'GpioBuilderImpl'
    ]
    
    for struct_name in structs_with_unused_fields:
        # Add allow attribute before struct definition
        pattern = rf'((?:\/\/.*\n)*)((?:pub\s+)?struct\s+{struct_name})'
        replacement = r'\1#[allow(dead_code)]\n\2'
        content = re.sub(pattern, replacement, content)
    
    return content


def fix_mixed_attributes(content):
    """Fix mixed inner and outer attributes"""
    # Fix the specific case in lib.rs
    if 'Service entry point and lifecycle management' in content:
        # Remove the inner doc comments that conflict with outer ones
        content = re.sub(
            r'(/// Service entry point.*?/// for the binary crate to use\.\n)(pub mod service_impl \{[\s\S]*?)(    //! This module.*?//! lifecycle of the communication service.*?\n)',
            r'\1\2',
            content,
            flags=re.DOTALL
        )
    
    return content


def fix_never_type_fallback(content):
    """Fix never type fallback warnings"""
    # Add explicit type annotations for Redis queries
    if 'query_async' in content:
        # Fix pipe.query_async
        content = re.sub(
            r'pipe\.query_async\(&mut conn\)',
            r'pipe.query_async::<()>(&mut conn)',
            content
        )
        # Fix cmd.query_async
        content = re.sub(
            r'\.query_async\(&mut conn\)(\s*\.await)',
            r'.query_async::<()>(&mut conn)\1',
            content
        )
    
    return content


def fix_manual_strip_prefix(content):
    """Fix manual strip prefix warnings"""
    # Replace manual prefix stripping with strip_prefix
    content = re.sub(
        r'if\s+(\w+)\.starts_with\("([^"]+)"\)\s*\{\s*&(\w+)\[(\d+)\.\.\]\s*\}',
        r'\1.strip_prefix("\2").unwrap_or(\1)',
        content
    )
    
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
        # Add allow attribute before async fn or fn
        pattern = rf'(\s*)((?:pub\s+)?(?:async\s+)?fn\s+{method})'
        replacement = r'\1#[allow(dead_code)]\n\1\2'
        content = re.sub(pattern, replacement, content)
    
    return content


def fix_to_string_impl(content):
    """Fix inherent to_string implementation"""
    # Replace inherent to_string with Display trait implementation
    if 'impl ProtocolType' in content and 'fn to_string(&self) -> String' in content:
        # Find the to_string implementation
        pattern = r'(impl ProtocolType \{[\s\S]*?)(pub fn to_string\(&self\) -> String \{[\s\S]*?\})'
        
        # Extract the match arms
        match = re.search(pattern, content)
        if match:
            to_string_impl = match.group(2)
            # Convert to Display trait
            display_impl = f'''impl std::fmt::Display for ProtocolType {{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{
        let s = match self {{
            Self::ModbusTCP => "modbus_tcp",
            Self::ModbusRTU => "modbus_rtu", 
            Self::CAN => "can",
            Self::IEC104 => "iec104",
            Self::Virtual => "virtual",
            Self::GPIO => "gpio",
        }};
        write!(f, "{{}}", s)
    }}
}}'''
            # Remove the to_string method from impl block
            content = re.sub(
                r'(\s*)pub fn to_string\(&self\) -> String \{[\s\S]*?\n\1\}',
                '',
                content
            )
            # Add Display impl after the impl block
            content = re.sub(
                r'(impl ProtocolType \{[\s\S]*?\})',
                rf'\1\n\n{display_impl}',
                content
            )
    
    return content


def process_file(file_path):
    """Process a single Rust file"""
    try:
        with open(file_path, 'r', encoding='utf-8') as f:
            content = f.read()
        
        original_content = content
        
        # Apply fixes
        content = fix_unused_variables(content)
        content = fix_field_never_read(content)
        content = fix_mixed_attributes(content)
        content = fix_never_type_fallback(content)
        content = fix_manual_strip_prefix(content)
        content = fix_unused_methods(content)
        content = fix_to_string_impl(content)
        
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
    # Focus on comsrv service
    comsrv_dir = Path('/Users/lyf/dev/VoltageEMS/services/comsrv/src')
    
    fixed_count = 0
    
    # Process all Rust files
    for rust_file in comsrv_dir.rglob('*.rs'):
        if process_file(rust_file):
            fixed_count += 1
    
    print(f"\nTotal files fixed: {fixed_count}")


if __name__ == '__main__':
    main()