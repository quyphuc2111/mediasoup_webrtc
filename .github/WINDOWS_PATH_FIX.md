# Windows Build Path Fix

## Problem

The Windows build was failing with the following error during mediasoup-sys compilation:

```
fatal error C1083: Cannot open compiler generated file: '': Invalid argument
```

This occurred when the MSVC compiler tried to compile flatbuffers C++ files from the mediasoup-sys dependency. The issue was caused by **extremely long path names** in the Cargo registry:

```
C:\Users\runneradmin\.cargo\registry\src\index.crates.io-1949cf8c6b5b557f\mediasoup-sys-0.10.0\subprojects\flatbuffers-24.3.25\...
```

## Root Cause

1. **Windows Path Length Limitations**: Windows has a default MAX_PATH limit of 260 characters
2. **Long Cargo Registry Paths**: Cargo's registry structure creates very long paths
3. **Meson/Ninja Build System**: The build system generates even longer paths for intermediate files (PDB, OBJ files)
4. **MSVC Compiler Issue**: When the combined path exceeds limits, the compiler fails with an empty filename error

## Solution

We implemented **multiple path-shortening strategies** specifically for Windows builds:

### 1. Enable Git Long Paths
```yaml
git config --system core.longpaths true
```

### 2. Use Shorter Cargo Paths
Set custom CARGO_HOME and CARGO_TARGET_DIR to very short paths:
```yaml
CARGO_HOME=C:\c
CARGO_TARGET_DIR=C:\t
```

This changes paths from:
```
C:\Users\runneradmin\.cargo\registry\... (60+ characters base)
```
To:
```
C:\c\registry\... (11 characters base)
```

### 3. Disable Incremental Compilation
```yaml
CARGO_INCREMENTAL: '0'
CARGO_PROFILE_RELEASE_DEBUG: '0'
```
This reduces intermediate file generation.

## Implementation

The fix was added to the GitHub Actions workflow (`.github/workflows/release.yml`) in both jobs:
- `release` (teacher app)
- `release-student` (student app)

### New Step: Configure Windows Path Optimization
```yaml
- name: Configure Windows Path Optimization (Windows only)
  if: matrix.platform == 'windows-latest'
  shell: pwsh
  run: |
    # Enable long paths in Git
    git config --system core.longpaths true
    
    # Set shorter paths for Cargo
    echo "CARGO_HOME=C:\c" >> $env:GITHUB_ENV
    echo "CARGO_TARGET_DIR=C:\t" >> $env:GITHUB_ENV
    
    # Create directories
    New-Item -ItemType Directory -Force -Path C:\c
    New-Item -ItemType Directory -Force -Path C:\t
    
    # Copy existing Cargo registry if it exists
    if (Test-Path "$env:USERPROFILE\.cargo\registry") {
      Write-Host "Copying Cargo registry to shorter path..."
      Copy-Item -Path "$env:USERPROFILE\.cargo\registry" -Destination "C:\c\registry" -Recurse -Force
    }
    
    Write-Host "CARGO_HOME set to: C:\c"
    Write-Host "CARGO_TARGET_DIR set to: C:\t"
```

### Updated Build Steps
All Cargo and NPM build steps now use the shortened paths on Windows:
- Build Mediasoup Rust Server
- Prepare binaries

## Expected Outcome

With these changes:
1. ✅ Path lengths reduced by ~50 characters at the base
2. ✅ MSVC compiler can generate intermediate files successfully
3. ✅ mediasoup-sys (flatbuffers) compilation should succeed
4. ✅ Windows builds complete successfully

## Testing

To test this fix:
1. Push a tag to trigger the release workflow: `git tag v10.0.1 && git push origin v10.0.1`
2. Monitor the GitHub Actions workflow run
3. Check the "Configure Windows Path Optimization" step output
4. Verify the "Build Mediasoup Rust Server" step completes without the C1083 error

## Rollback

If this fix causes issues, you can revert by removing the "Configure Windows Path Optimization" step and the environment variables from the build steps.

## Related Issues

- GitHub Actions Runner Issue: [actions/runner-images#10004](https://github.com/actions/runner-images/issues/10004)
- Cargo Path Length Issue: [rust-lang/cargo#13560](https://github.com/rust-lang/cargo/issues/13560)
- MSVC Long Path Support: [microsoft/vcpkg#14469](https://github.com/microsoft/vcpkg/discussions/14469)

---
**Date**: 2026-02-03  
**Modified Files**: `.github/workflows/release.yml`  
**Status**: Testing Required
