# Hybrid Authentication - Quick Start Guide

## 🎯 Two Authentication Modes

### 1. **Ed25519** (Default - Recommended for small classrooms)
- No password needed
- Simple setup
- Teacher generates keypair, students import public key

### 2. **LDAP/AD** (Enterprise - For schools with Active Directory)
- Username + password login
- Centralized user management
- Group-based access control

## 📋 Quick Setup

### Option A: Ed25519 (Simple)

**Teacher:**
```typescript
// 1. Generate keypair (one time)
const keypair = await invoke('crypto_generate_keypair');

// 2. Export & share public key
const publicKey = await invoke('crypto_export_public_key');
```

**Student:**
```typescript
// 1. Import teacher's public key
await invoke('crypto_import_teacher_key', { keyData: publicKey });

// 2. Start agent
await invoke('start_student_agent', { config: { port: 3017 } });
```

### Option B: LDAP (Enterprise)

**Setup LDAP Config:**
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
await invoke('auth_set_mode', { mode: 'Ldap' });
```

**Authenticate:**
```typescript
const result = await invoke('ldap_authenticate', {
  config,
  username: 'john.teacher',
  password: 'password'
});
```

## 📚 Full Documentation

See [LDAP_AUTHENTICATION.md](./LDAP_AUTHENTICATION.md) for complete details.

## 🔧 Tauri Commands Reference

### Auth Mode
- `auth_set_mode(mode: 'Ed25519' | 'Ldap')` - Switch authentication mode
- `auth_get_mode()` - Get current mode

### LDAP
- `ldap_save_config(config)` - Save LDAP configuration
- `ldap_load_config()` - Load LDAP configuration
- `ldap_test_connection(config)` - Test LDAP connection
- `ldap_authenticate(config, username, password)` - Authenticate user

### Ed25519 (Existing commands)
- `crypto_generate_keypair()` - Generate teacher keypair
- `crypto_export_public_key()` - Export public key
- `crypto_import_teacher_key(keyData)` - Import teacher's public key

## 💡 When to Use Which Mode?

| Scenario | Use Ed25519 | Use LDAP |
|----------|-------------|----------|
| Small classroom (1-30) | ✅ | ❌ |
| School-wide (100+) | ❌ | ✅ |
| Has Active Directory | Maybe | ✅ |
| No IT department | ✅ | ❌ |
| Need audit logs | ❌ | ✅ |

## 🐛 Troubleshooting

**LDAP issues:**
- Check firewall (port 389/636)
- Verify server URL
- Test with `ldapsearch` command

**Ed25519 issues:**
- Ensure keypair generated
- Check `~/.smartlab/` directory

---

**For detailed information, examples, and advanced configuration, see [LDAP_AUTHENTICATION.md](./LDAP_AUTHENTICATION.md)**
