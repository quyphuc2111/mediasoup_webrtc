# LDAP/Active Directory Authentication Implementation

## 📋 Overview

This document describes the implementation of **hybrid authentication** for the SmartLab ScreenSharing application. The system now supports **two authentication modes**:

1. **Ed25519 PKI** (Default) - Simple, no infrastructure needed
2. **LDAP/Active Directory** (Enterprise) - Centralized user management

## 🎯 Features

### Ed25519 Mode (Original)
- ✅ Cryptographic key-based authentication
- ✅ No password required
- ✅ Simple setup for small classrooms
- ✅ Teacher generates keypair, student imports public key
- ✅ Challenge-response protocol

### LDAP Mode (New!)
- ✅ Username + password authentication
- ✅ Integration with Active Directory or OpenLDAP
- ✅ Group-based access control (e.g., "Teachers" group)
- ✅ Centralized user management
- ✅ LDAP injection protection
- ✅ Identity tracking and audit capabilities

## 🏗️ Architecture

```
┌──────────────────────────────────────────────────────────┐
│                   SmartLab Application                    │
├──────────────────────────────────────────────────────────┤
│                                                           │
│  ┌─────────────────────────────────────────────────┐    │
│  │           Authentication Layer                   │    │
│  │  ┌───────────────┬──────────────────────────┐   │    │
│  │  │  Ed25519 PKI  │  LDAP/AD Integration     │   │    │
│  │  │  (crypto.rs)  │  (ldap_auth.rs)          │   │    │
│  │  └───────────────┴──────────────────────────┘   │    │
│  └─────────────────────────────────────────────────┘    │
│                      ↓                                    │
│  ┌─────────────────────────────────────────────────┐    │
│  │           Student Agent (WebSocket Server)       │    │
│  │  - Accepts connections from teachers             │    │
│  │  - Validates authentication (both modes)         │    │
│  │  - Streams screen content                        │    │
│  └─────────────────────────────────────────────────┘    │
│                                                           │
└──────────────────────────────────────────────────────────┘
```

## 📦 Implementation Details

### 1. Files Added/Modified

#### New Files:
- **`src-tauri/src/ldap_auth.rs`** (260 lines)
  - LDAP configuration management
  - Async LDAP authentication
  - Connection testing
  - User search and group validation

#### Modified Files:
- **`src-tauri/src/crypto.rs`**
  - Added `AuthMode` enum
  - Auth mode persistence functions
  
- **`src-tauri/src/lib.rs`**
  - LDAP module registration
  - Tauri command registration (6 new commands)
  
- **`src-tauri/Cargo.toml`**
  - Added `ldap3 = "0.12"` dependency

### 2. Data Structures

#### AuthMode Enum
```rust
pub enum AuthMode {
    Ed25519,  // Default: Key-based authentication
    Ldap,     // Enterprise: LDAP/AD authentication
}
```

#### LdapConfig Struct
```rust
pub struct LdapConfig {
    pub server_url: String,          // e.g., "ldap://192.168.1.10:389"
    pub base_dn: String,              // e.g., "DC=school,DC=local"
    pub user_filter: String,          // e.g., "(&(objectClass=user)(sAMAccountName={username}))"
    pub bind_dn_template: String,     // e.g., "{username}@school.local"
    pub required_group: Option<String>, // e.g., "CN=Teachers,OU=Groups,DC=school,DC=local"
    pub use_tls: bool,
}
```

#### LdapAuthResult
```rust
pub struct LdapAuthResult {
    pub success: bool,
    pub username: Option<String>,
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub groups: Vec<String>,
    pub error: Option<String>,
}
```

### 3. Tauri Commands (Frontend API)

#### Authentication Mode Management:
```typescript
// Set authentication mode
await invoke('auth_set_mode', { mode: 'Ed25519' | 'Ldap' });

// Get current mode
const mode = await invoke('auth_get_mode');
```

#### LDAP Configuration:
```typescript
// Save LDAP config
await invoke('ldap_save_config', { config: ldapConfig });

// Load LDAP config
const config = await invoke('ldap_load_config');

// Test connection
const result = await invoke('ldap_test_connection', { config });
```

#### LDAP Authentication:
```typescript
const result = await invoke('ldap_authenticate', {
  config: ldapConfig,
  username: 'john.doe',
  password: 'password123'
});

if (result.success) {
  console.log('Authenticated:', result.display_name);
  console.log('Groups:', result.groups);
}
```

## 🔧 Configuration Examples

### Active Directory (Windows Server)
```json
{
  "server_url": "ldap://dc.school.local:389",
  "base_dn": "DC=school,DC=local",
  "user_filter": "(&(objectClass=user)(sAMAccountName={username}))",
  "bind_dn_template": "{username}@school.local",
  "required_group": "CN=Teachers,OU=Groups,DC=school,DC=local",
  "use_tls": false
}
```

### OpenLDAP (Linux)
```json
{
  "server_url": "ldap://ldap.school.local:389",
  "base_dn": "ou=People,dc=school,dc=local",
  "user_filter": "(&(objectClass=inetOrgPerson)(uid={username}))",
  "bind_dn_template": "uid={username},ou=People,dc=school,dc=local",
  "required_group": "cn=teachers,ou=Groups,dc=school,dc=local",
  "use_tls": false
}
```

### LDAPS (Secure LDAP over TLS)
```json
{
  "server_url": "ldaps://ldap.school.local:636",
  "base_dn": "DC=school,DC=local",
  "user_filter": "(&(objectClass=user)(sAMAccountName={username}))",
  "bind_dn_template": "{username}@school.local",
  "required_group": null,
  "use_tls": true
}
```

## 🔐 Security Features

### 1. LDAP Injection Protection
All user input is sanitized to prevent LDAP injection attacks:
```rust
fn sanitize_ldap_input(input: &str) -> String {
    input
        .replace('\\', "\\5c")
        .replace('*', "\\2a")
        .replace('(', "\\28")
        .replace(')', "\\29")
        .replace('\0', "\\00")
}
```

### 2. Group-Based Access Control
Teachers must be members of a specific LDAP group:
```rust
if let Some(required_group) = &config.required_group {
    if !groups.iter().any(|g| g.contains(required_group)) {
        return Err("User is not a member of required group");
    }
}
```

### 3. Secure Configuration Storage
Configurations are stored in `~/.smartlab/`:
- `auth_mode.json` - Current authentication mode
- `ldap_config.json` - LDAP server configuration
- `teacher_keypair.json` - Ed25519 keypair (Ed25519 mode)
- `teacher_public_key.txt` - Teacher's public key (Ed25519 mode)

## 🚀 Usage Guide

### For Small Classrooms (Ed25519 Mode - Default)

#### Teacher Setup:
1. Generate keypair (one time):
   ```typescript
   const keypair = await invoke('crypto_generate_keypair');
   console.log('Fingerprint:', keypair.fingerprint);
   ```

2. Export public key:
   ```typescript
   const publicKey = await invoke('crypto_export_public_key');
   // Share this with students
   ```

#### Student Setup:
1. Import teacher's public key:
   ```typescript
   await invoke('crypto_import_teacher_key', { keyData: publicKey });
   ```

2. Start agent:
   ```typescript
   await invoke('start_student_agent', { config: { port: 3017 } });
   ```

#### Teacher Connection:
```typescript
const connectionId = await invoke('connect_to_student', {
  ip: '192.168.1.100',
  port: 3017
});
```

### For Enterprise Environments (LDAP Mode)

#### 1. Configure LDAP:
```typescript
const config = {
  server_url: 'ldap://dc.school.local:389',
  base_dn: 'DC=school,DC=local',
  user_filter: '(&(objectClass=user)(sAMAccountName={username}))',
  bind_dn_template: '{username}@school.local',
  required_group: 'CN=Teachers,OU=Groups,DC=school,DC=local',
  use_tls: false
};

await invoke('ldap_save_config', { config });
```

#### 2. Test Connection:
```typescript
const result = await invoke('ldap_test_connection', { config });
console.log(result); // "Successfully connected to LDAP server"
```

#### 3. Switch to LDAP Mode:
```typescript
await invoke('auth_set_mode', { mode: 'Ldap' });
```

#### 4. Authenticate Teacher:
```typescript
const result = await invoke('ldap_authenticate', {
  config,
  username: 'john.teacher',
  password: 'SecurePassword123'
});

if (result.success) {
  console.log('Welcome', result.display_name);
  // Proceed with connection
}
```

## 📊 Comparison Table

| Feature | Ed25519 Mode | LDAP Mode |
|---------|--------------|-----------|
| **Setup Complexity** | ⭐ Simple | ⭐⭐⭐ Complex |
| **Infrastructure Required** | None | LDAP/AD Server |
| **User Management** | Manual | Centralized |
| **Password Required** | No | Yes |
| **Group Permissions** | No | Yes |
| **Identity Tracking** | No | Yes |
| **Audit Logs** | No | Possible via LDAP |
| **Best For** | Small classrooms | Enterprise/Schools |
| **User Experience** | Fast, no login | Familiar (username/password) |
| **Key Distribution** | Manual | Automatic |

## 🎓 Use Case Recommendations

### Use Ed25519 When:
- ✅ Small classroom (1-30 students)
- ✅ Single teacher or few teachers
- ✅ No IT infrastructure
- ✅ Quick setup needed
- ✅ Privacy-focused (no centralized user database)

### Use LDAP When:
- ✅ School-wide deployment (100+ computers)
- ✅ Existing Active Directory infrastructure
- ✅ Multiple schools/campuses
- ✅ Need user audit logs
- ✅ IT department manages users
- ✅ Group-based access control required

## 🔄 Authentication Flow Diagrams

### Ed25519 Flow (Challenge-Response):
```
Teacher                          Student
   │                                │
   │  1. WebSocket Connect          │
   │───────────────────────────────→│
   │                                │
   │  2. Welcome + Challenge        │
   │←───────────────────────────────│
   │                                │
   │  3. Sign Challenge             │
   │     with private key           │
   │                                │
   │  4. Send Signature             │
   │───────────────────────────────→│
   │                                │
   │                                │ 5. Verify with
   │                                │    teacher's public key
   │                                │
   │  6. AuthSuccess/Failed         │
   │←───────────────────────────────│
```

### LDAP Flow:
```
Teacher                  Student Agent              LDAP Server
   │                          │                          │
   │  1. Username+Password   │                          │
   │────────────────────────→│                          │
   │                          │  2. Bind Request         │
   │                          │─────────────────────────→│
   │                          │                          │
   │                          │  3. Bind Success/Fail    │
   │                          │←─────────────────────────│
   │                          │                          │
   │                          │  4. Search User Details  │
   │                          │─────────────────────────→│
   │                          │                          │
   │                          │  5. User Info + Groups   │
   │                          │←─────────────────────────│
   │                          │                          │
   │                          │  6. Check Group          │
   │                          │     Membership           │
   │                          │                          │
   │  7. AuthSuccess/Failed   │                          │
   │←────────────────────────│                          │
```

## 🛠️ Troubleshooting

### LDAP Connection Issues:

1. **"Connection failed" error**:
   - Check firewall: port 389 (LDAP) or 636 (LDAPS)
   - Verify server URL and network connectivity
   - Test with: `ldapsearch -H ldap://server -x`

2. **"Authentication failed" error**:
   - Verify username format (SAMAccountName vs UPN)
   - Check bind DN template matches your AD structure
   - Test bind: `ldapwhoami -H ldap://server -D "user@domain" -W`

3. **"User not found in directory" error**:
   - Verify base DN is correct
   - Check user filter syntax
   - Test search: `ldapsearch -H ldap://server -D "..." -W -b "dc=..." "(sAMAccountName=user)"`

4. **"Not a member of required group" error**:
   - Verify group DN is correct
   - Check user's group memberships
   - Consider making `required_group` optional (null)

### Ed25519 Issues:

1. **"No keypair found" error**:
   - Generate keypair: `invoke('crypto_generate_keypair')`
   - Check `~/.smartlab/teacher_keypair.json` exists

2. **"No teacher public key found" error**:
   - Import key: `invoke('crypto_import_teacher_key', { keyData })`
   - Check `~/.smartlab/teacher_public_key.txt` exists

## 📝 Development Notes

### File Locations:
- **Backend (Rust)**:
  - `src-tauri/src/ldap_auth.rs` - LDAP authentication logic
  - `src-tauri/src/crypto.rs` - Ed25519 + AuthMode management
  - `src-tauri/src/lib.rs` - Tauri command handlers
  
- **Configuration Storage**:
  - `~/.smartlab/auth_mode.json`
  - `~/.smartlab/ldap_config.json`
  - `~/.smartlab/teacher_keypair.json` (Ed25519 mode)
  - `~/.smartlab/teacher_public_key.txt` (Ed25519 mode)

### Dependencies Added:
```toml
ldap3 = "0.12"  # Pure-Rust LDAP client
```

### Build & Test:
```bash
cd src-tauri
cargo build
cargo test
```

## 🔮 Future Enhancements

Potential improvements:
1. **SAML/OAuth Integration** - Single Sign-On
2. **Kerberos Support** - Windows integrated auth
3. **Certificate-based Auth** - Smart cards
4. **Multi-Factor Authentication** - OTP/TOTP
5. **Session Management** - Token-based auth
6. **LDAP Write Operations** - User account management
7. **Connection Pooling** - Reuse LDAP connections
8. **Caching** - Cache LDAP results

## 📄 License & Credits

This hybrid authentication system was designed for **SmartLab ScreenSharing** application.

**Technologies**:
- [ldap3](https://crates.io/crates/ldap3) - Pure-Rust LDAP client
- [ed25519-dalek](https://crates.io/crates/ed25519-dalek) - Ed25519 signatures
- [Tauri](https://tauri.app/) - Desktop application framework

---

**Version**: 1.0.0  
**Last Updated**: 2026-01-27  
**Author**: SmartLab Development Team
