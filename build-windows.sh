#!/bin/bash

set -e

target="x86_64-pc-windows-msvc"

export RUSTFLAGS="-C link-arg=/LIBPATH:. -C link-arg=/LIBPATH:deps"
cargo xwin build -j 30 --target $target

# Use objdump to find exactly which DLLs the executable needs
exe_path="target/$target/debug/sregame.exe"
required_dlls=$(objdump -p "$exe_path" 2>/dev/null | grep -i "DLL Name" | awk '{print $3}' | grep -E '\.(dll|DLL)$')

# Get the sysroot for standard library DLLs
sysroot=$(rustc --print sysroot)
target_lib_dir="$sysroot/lib/rustlib/$target/lib"

# Copy each required DLL
for dll_name in $required_dlls; do
    # Skip system DLLs (Windows will provide these)
    if [[ "$dll_name" =~ ^(KERNEL32|USER32|ADVAPI32|GDI32|SHLWAPI|VCRUNTIME|api-ms-win|msvcrt).*\.dll$ ]]; then
        continue
    fi

    if [[ -f "target/$target/debug/$dll_name" ]]; then
        continue
    fi

    # Look for the DLL in various locations
    found=false

    # Check deps directory first (for versioned Bevy DLLs)
    if [[ -f "target/$target/debug/deps/$dll_name" ]]; then
        cp "target/$target/debug/deps/$dll_name" "target/$target/debug/"
        echo "Copied from deps: $dll_name"
        found=true
    # Check target lib directory (for std DLLs)
    elif [[ -f "$target_lib_dir/$dll_name" ]]; then
        cp "$target_lib_dir/$dll_name" "target/$target/debug/"
        echo "Copied from sysroot: $dll_name"
        found=true
    fi

    if [[ "$found" == "false" ]]; then
        echo "Warning: Could not find required DLL: $dll_name"
    fi
done

echo "Build complete: target/x86_64-pc-windows-msvc/debug/sregame.exe"
