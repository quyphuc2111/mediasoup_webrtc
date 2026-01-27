# 🏢 Local LDAP Server for Testing

This directory contains a complete configuration to run a local OpenLDAP server using Docker. This allows you to test the LDAP authentication features of SmartLab without needing an enterprise infrastructure.

## 🚀 Usage

### Prerequisites
- Docker Desktop installed and running

### Start Server
Run the start script:
```bash
./start.sh
```

### Access Details
- **LDAP Server**: `ldap://localhost:389`
- **Admin UI**: [http://localhost:8080](http://localhost:8080)
- **Base DN**: `dc=school,dc=local`
- **Admin DN**: `cn=admin,dc=school,dc=local`
- **Admin Password**: `admin`

### Sample Users
Password for all sample users is `password`.

| Username | Role | Group |
|----------|------|-------|
| `teacher1` | Teacher | `cn=Teachers,ou=Groups,dc=school,dc=local` |
| `student1` | Student | `cn=Students,ou=Groups,dc=school,dc=local` |

## ⚙️ SmartLab Configuration

To use this server with SmartLab:

1. Go to **Authentication Settings**.
2. Select **LDAP/AD Mode**.
3. Use these settings (or click "OpenLDAP" example and adjust):
   - **Server URL**: `ldap://localhost:389`
   - **Base DN**: `dc=school,dc=local`
   - **User Filter**: `(&(objectClass=inetOrgPerson)(uid={username}))`
   - **Bind DN Template**: `uid={username},ou=People,dc=school,dc=local`
   - **Required Group**: `cn=Teachers,ou=Groups,dc=school,dc=local`

## 🛑 Stop Server
```bash
docker-compose down
```
