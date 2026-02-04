#!/bin/bash

# =============================================================================
# Release Script for SmartLab
# Cập nhật version và push tag để trigger GitHub Actions build
# 
# Usage: ./scripts/release.sh <version>
# Example: ./scripts/release.sh 1.0.3
# =============================================================================

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Check if version argument is provided
if [ -z "$1" ]; then
    echo -e "${RED}Error: Version number is required${NC}"
    echo "Usage: ./scripts/release.sh <version>"
    echo "Example: ./scripts/release.sh 1.0.3"
    exit 1
fi

VERSION=$1

# Validate version format (semver: x.y.z)
if ! [[ $VERSION =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo -e "${RED}Error: Invalid version format. Use semver format: x.y.z${NC}"
    echo "Example: 1.0.3"
    exit 1
fi

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}  SmartLab Release Script v$VERSION${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# Get script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"

cd "$ROOT_DIR"

# Check if we're in a git repository
if ! git rev-parse --is-inside-work-tree > /dev/null 2>&1; then
    echo -e "${RED}Error: Not a git repository${NC}"
    exit 1
fi

# Check for uncommitted changes
if ! git diff-index --quiet HEAD --; then
    echo -e "${YELLOW}Warning: You have uncommitted changes${NC}"
    read -p "Do you want to continue? (y/n) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 1
    fi
fi

# Check if tag already exists
if git rev-parse "v$VERSION" >/dev/null 2>&1; then
    echo -e "${RED}Error: Tag v$VERSION already exists${NC}"
    echo "Use a different version number or delete the existing tag:"
    echo "  git tag -d v$VERSION"
    echo "  git push origin :refs/tags/v$VERSION"
    exit 1
fi

echo -e "${GREEN}[1/5]${NC} Updating package.json..."
# Update package.json
if [ -f "package.json" ]; then
    if [[ "$OSTYPE" == "darwin"* ]]; then
        # macOS
        sed -i '' "s/\"version\": \"[^\"]*\"/\"version\": \"$VERSION\"/" package.json
    else
        # Linux
        sed -i "s/\"version\": \"[^\"]*\"/\"version\": \"$VERSION\"/" package.json
    fi
    echo -e "  ${GREEN}✓${NC} package.json updated to $VERSION"
else
    echo -e "  ${YELLOW}⚠${NC} package.json not found"
fi

echo -e "${GREEN}[2/5]${NC} Updating Tauri configs..."
# Update src-tauri/tauri.conf.json (Teacher app)
if [ -f "src-tauri/tauri.conf.json" ]; then
    if [[ "$OSTYPE" == "darwin"* ]]; then
        sed -i '' "s/\"version\": \"[^\"]*\"/\"version\": \"$VERSION\"/" src-tauri/tauri.conf.json
    else
        sed -i "s/\"version\": \"[^\"]*\"/\"version\": \"$VERSION\"/" src-tauri/tauri.conf.json
    fi
    echo -e "  ${GREEN}✓${NC} tauri.conf.json (Teacher) updated to $VERSION"
else
    echo -e "  ${YELLOW}⚠${NC} tauri.conf.json not found"
fi

# Update src-tauri/tauri.student.conf.json (Student app)
if [ -f "src-tauri/tauri.student.conf.json" ]; then
    if [[ "$OSTYPE" == "darwin"* ]]; then
        sed -i '' "s/\"version\": \"[^\"]*\"/\"version\": \"$VERSION\"/" src-tauri/tauri.student.conf.json
    else
        sed -i "s/\"version\": \"[^\"]*\"/\"version\": \"$VERSION\"/" src-tauri/tauri.student.conf.json
    fi
    echo -e "  ${GREEN}✓${NC} tauri.student.conf.json (Student) updated to $VERSION"
else
    echo -e "  ${YELLOW}⚠${NC} tauri.student.conf.json not found"
fi

echo -e "${GREEN}[3/5]${NC} Committing version changes..."
# Stage and commit changes
git add package.json src-tauri/tauri.conf.json src-tauri/tauri.student.conf.json 2>/dev/null || true
git commit -m "chore: bump version to $VERSION" || echo -e "  ${YELLOW}⚠${NC} No changes to commit"

echo -e "${GREEN}[4/5]${NC} Creating tag v$VERSION..."
# Create annotated tag
git tag -a "v$VERSION" -m "Release v$VERSION

SmartLab version $VERSION

Apps:
- SmartlabPromax_${VERSION}_x64-setup.exe (Teacher - Windows)
- SmartlabStudent_${VERSION}_x64-setup.exe (Student - Windows)
- SmartlabPromax_${VERSION}_aarch64.dmg (Teacher - macOS ARM)
- SmartlabStudent_${VERSION}_aarch64.dmg (Student - macOS ARM)
"

echo -e "  ${GREEN}✓${NC} Tag v$VERSION created"

echo -e "${GREEN}[5/5]${NC} Pushing to remote..."
# Push commits and tag
git push origin HEAD
git push origin "v$VERSION"

echo ""
echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}  Release v$VERSION completed!${NC}"
echo -e "${GREEN}========================================${NC}"
echo ""
echo -e "GitHub Actions will now build the following artifacts:"
echo -e "  ${BLUE}Windows:${NC}"
echo -e "    - SmartlabPromax_${VERSION}_x64-setup.exe"
echo -e "    - SmartlabStudent_${VERSION}_x64-setup.exe"
echo -e "  ${BLUE}macOS:${NC}"
echo -e "    - SmartlabPromax_${VERSION}_aarch64.dmg"
echo -e "    - SmartlabStudent_${VERSION}_aarch64.dmg"
echo ""
echo -e "Check build status at:"
echo -e "  ${BLUE}https://github.com/$(git remote get-url origin | sed 's/.*github.com[:/]\(.*\)\.git/\1/')/actions${NC}"
echo ""
