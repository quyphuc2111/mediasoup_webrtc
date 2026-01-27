#!/bin/bash

# Get script directory
DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

echo "🚀 Starting Local LDAP Server..."
echo "📂 Data directory: $DIR/data"

cd "$DIR"

# Check if docker is running
if ! docker info > /dev/null 2>&1; then
  echo "❌ Docker is not running. Please start Docker first."
  exit 1
fi

# Start container
docker-compose up -d

echo ""
echo "✅ LDAP Server Started!"
echo "---------------------------------------------------"
echo "🔌 LDAP URL:     ldap://localhost:389"
echo "🔐 Admin DN:     cn=admin,dc=school,dc=local"
echo "🔑 Password:     admin"
echo "---------------------------------------------------"
echo "🖥️ Admin UI:     http://localhost:8080"
echo "---------------------------------------------------"
echo "👤 Sample Users (Password: 'password'):"
echo "   - teacher1 (in Teachers group)"
echo "   - student1 (in Students group)"
echo "---------------------------------------------------"
echo "💡 To configure SmartLab:"
echo "   1. Go to Auth Settings"
echo "   2. Choose LDAP Mode"
echo "   3. Click 'OpenLDAP' Example"
echo "   4. Update Server URL to: ldap://localhost:389"
echo "   5. Base DN: dc=school,dc=local"
echo "   6. Bind DN: uid={username},ou=People,dc=school,dc=local"
echo "---------------------------------------------------"
