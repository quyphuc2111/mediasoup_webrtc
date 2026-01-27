/**
 * Authentication Examples for SmartLab ScreenSharing
 * 
 * This file demonstrates how to use both Ed25519 and LDAP authentication modes
 * in the frontend application.
 */

import { invoke } from '@tauri-apps/api/core';

// ============================================================
// Type Definitions
// ============================================================

type AuthMode = 'Ed25519' | 'Ldap';

interface KeyPairInfo {
    public_key: string;
    private_key: string;
    fingerprint: string;
}

interface LdapConfig {
    server_url: string;
    base_dn: string;
    user_filter: string;
    bind_dn_template: string;
    required_group: string | null;
    use_tls: boolean;
}

interface LdapAuthResult {
    success: boolean;
    username?: string;
    display_name?: string;
    email?: string;
    groups: string[];
    error?: string;
}

// ============================================================
// Authentication Mode Management
// ============================================================

/**
 * Get current authentication mode
 */
export async function getAuthMode(): Promise<AuthMode> {
    return await invoke('auth_get_mode');
}

/**
 * Set authentication mode
 */
export async function setAuthMode(mode: AuthMode): Promise<void> {
    await invoke('auth_set_mode', { mode });
}

// ============================================================
// Ed25519 Authentication (Default Mode)
// ============================================================

/**
 * Example: Teacher generates and saves keypair (one-time setup)
 */
export async function teacherSetupEd25519(): Promise<KeyPairInfo> {
    try {
        // Generate keypair
        const keypair = await invoke<KeyPairInfo>('crypto_generate_keypair');
        console.log('Generated keypair with fingerprint:', keypair.fingerprint);

        // Export public key to share with students
        const publicKey = await invoke<string>('crypto_export_public_key');
        console.log('Share this public key with students:');
        console.log(publicKey);

        return keypair;
    } catch (error) {
        console.error('Failed to setup Ed25519:', error);
        throw error;
    }
}

/**
 * Example: Student imports teacher's public key
 */
export async function studentSetupEd25519(teacherPublicKey: string): Promise<void> {
    try {
        await invoke('crypto_import_teacher_key', { keyData: teacherPublicKey });
        console.log('Successfully imported teacher public key');
    } catch (error) {
        console.error('Failed to import teacher key:', error);
        throw error;
    }
}

/**
 * Example: Teacher connects to student (Ed25519 mode)
 */
export async function connectWithEd25519(studentIp: string, port: number = 3017): Promise<string> {
    try {
        // Ensure we're in Ed25519 mode
        await setAuthMode('Ed25519');

        // Check if keypair exists
        const hasKey = await invoke<boolean>('crypto_has_keypair');
        if (!hasKey) {
            throw new Error('No keypair found. Please generate one first.');
        }

        // Connect to student
        const connectionId = await invoke<string>('connect_to_student', {
            ip: studentIp,
            port
        });

        console.log('Connected to student:', connectionId);
        return connectionId;
    } catch (error) {
        console.error('Failed to connect:', error);
        throw error;
    }
}

// ============================================================
// LDAP/Active Directory Authentication
// ============================================================

/**
 * Example LDAP configurations for different environments
 */
export const LDAP_CONFIG_EXAMPLES = {
    activeDirectory: {
        server_url: 'ldap://dc.school.local:389',
        base_dn: 'DC=school,DC=local',
        user_filter: '(&(objectClass=user)(sAMAccountName={username}))',
        bind_dn_template: '{username}@school.local',
        required_group: 'CN=Teachers,OU=Groups,DC=school,DC=local',
        use_tls: false
    } as LdapConfig,

    openLDAP: {
        server_url: 'ldap://ldap.school.local:389',
        base_dn: 'ou=People,dc=school,dc=local',
        user_filter: '(&(objectClass=inetOrgPerson)(uid={username}))',
        bind_dn_template: 'uid={username},ou=People,dc=school,dc=local',
        required_group: 'cn=teachers,ou=Groups,dc=school,dc=local',
        use_tls: false
    } as LdapConfig,

    ldaps: {
        server_url: 'ldaps://ldap.school.local:636',
        base_dn: 'DC=school,DC=local',
        user_filter: '(&(objectClass=user)(sAMAccountName={username}))',
        bind_dn_template: '{username}@school.local',
        required_group: null, // No group requirement
        use_tls: true
    } as LdapConfig
};

/**
 * Example: Save LDAP configuration
 */
export async function saveLdapConfig(config: LdapConfig): Promise<void> {
    try {
        await invoke('ldap_save_config', { config });
        console.log('LDAP configuration saved');
    } catch (error) {
        console.error('Failed to save LDAP config:', error);
        throw error;
    }
}

/**
 * Example: Load LDAP configuration
 */
export async function loadLdapConfig(): Promise<LdapConfig> {
    try {
        const config = await invoke<LdapConfig>('ldap_load_config');
        return config;
    } catch (error) {
        console.error('Failed to load LDAP config:', error);
        throw error;
    }
}

/**
 * Example: Test LDAP connection
 */
export async function testLdapConnection(config: LdapConfig): Promise<string> {
    try {
        const result = await invoke<string>('ldap_test_connection', { config });
        console.log(result);
        return result;
    } catch (error) {
        console.error('LDAP connection test failed:', error);
        throw error;
    }
}

/**
 * Example: Authenticate teacher with LDAP
 */
export async function authenticateWithLDAP(
    username: string,
    password: string
): Promise<LdapAuthResult> {
    try {
        // Load LDAP config
        const config = await loadLdapConfig();

        // Authenticate
        const result = await invoke<LdapAuthResult>('ldap_authenticate', {
            config,
            username,
            password
        });

        if (result.success) {
            console.log('Authentication successful!');
            console.log('User:', result.display_name || result.username);
            console.log('Email:', result.email);
            console.log('Groups:', result.groups);
        } else {
            console.error('Authentication failed:', result.error);
        }

        return result;
    } catch (error) {
        console.error('LDAP authentication error:', error);
        throw error;
    }
}

/**
 * Example: Complete LDAP setup flow
 */
export async function setupLDAPMode(config: LdapConfig): Promise<void> {
    try {
        // 1. Save configuration
        await saveLdapConfig(config);

        // 2. Test connection
        await testLdapConnection(config);

        // 3. Switch to LDAP mode
        await setAuthMode('Ldap');

        console.log('LDAP mode configured successfully');
    } catch (error) {
        console.error('LDAP setup failed:', error);
        throw error;
    }
}

// ============================================================
// React Component Examples
// ============================================================

/**
 * Example: Login component that supports both auth modes
 */
export function LoginComponentExample() {
    // This is pseudo-code showing the logic flow

    const _handleLogin = async () => {
        const authMode = await getAuthMode();
        const username = "user"; // Dummy
        const password = "password"; // Dummy

        if (authMode === 'Ed25519') {
            // Ed25519: No password needed, just connect
            const _connectionId = await connectWithEd25519('192.168.1.100');
            // Navigate to screen viewing
            console.log(_connectionId);

        } else if (authMode === 'Ldap') {
            // LDAP: Need username & password
            const result = await authenticateWithLDAP(username, password);

            if (result.success) {
                // Store user info
                localStorage.setItem('user', JSON.stringify(result));
                // Navigate to student selection
            } else {
                // Show error
                alert(result.error);
            }
        }
    };

    // Suppress unused warning
    void _handleLogin;
}

/**
 * Example: Settings component for auth mode selection
 */
export function SettingsComponentExample() {
    const _switchToEd25519 = async () => {
        await setAuthMode('Ed25519');
        // Show keypair generation UI
    };

    const _switchToLDAP = async () => {
        // Show LDAP configuration form
        const config = LDAP_CONFIG_EXAMPLES.activeDirectory;
        await setupLDAPMode(config);
    };

    // Suppress unused warning
    void _switchToEd25519;
    void _switchToLDAP;
}

/**
 * Example: LDAP Configuration Form Handler
 */
export function LdapConfigFormExample() {
    const _handleSubmit = async (formData: LdapConfig) => {
        try {
            // Test connection first
            await testLdapConnection(formData);

            // If successful, save
            await saveLdapConfig(formData);

            alert('LDAP configuration saved successfully!');
        } catch (error) {
            alert('Configuration test failed: ' + error);
        }
    };

    // Suppress unused warning
    void _handleSubmit;
}

// ============================================================
// Utility Functions
// ============================================================

/**
 * Check authentication readiness
 */
export async function isAuthReady(): Promise<{ ready: boolean; mode: AuthMode; message: string }> {
    const mode = await getAuthMode();

    if (mode === 'Ed25519') {
        const hasKey = await invoke<boolean>('crypto_has_keypair');
        return {
            ready: hasKey,
            mode,
            message: hasKey ? 'Ed25519 keypair ready' : 'Please generate keypair first'
        };
    } else {
        try {
            const config = await loadLdapConfig();
            return {
                ready: !!config.server_url,
                mode,
                message: config.server_url ? 'LDAP configured' : 'Please configure LDAP settings'
            };
        } catch {
            return {
                ready: false,
                mode,
                message: 'LDAP not configured'
            };
        }
    }
}

/**
 * Export all authentication functions
 */
export const Auth = {
    // Mode management
    getMode: getAuthMode,
    setMode: setAuthMode,
    isReady: isAuthReady,

    // Ed25519
    ed25519: {
        teacherSetup: teacherSetupEd25519,
        studentSetup: studentSetupEd25519,
        connect: connectWithEd25519
    },

    // LDAP
    ldap: {
        examples: LDAP_CONFIG_EXAMPLES,
        saveConfig: saveLdapConfig,
        loadConfig: loadLdapConfig,
        testConnection: testLdapConnection,
        authenticate: authenticateWithLDAP,
        setup: setupLDAPMode
    }
};

// ============================================================
// Usage Examples in Application
// ============================================================

/**
 * Example 1: Teacher App Startup
 */
export async function teacherAppStartup() {
    const { ready, mode, message } = await isAuthReady();

    if (!ready) {
        console.log('Auth not ready:', message);
        // Show setup wizard
        if (mode === 'Ed25519') {
            await teacherSetupEd25519();
        } else {
            // Show LDAP config form
        }
    } else {
        console.log('Auth ready:', message);
        // Continue to main app
    }
}

/**
 * Example 2: Student App Startup
 */
export async function studentAppStartup() {
    const mode = await getAuthMode();

    if (mode === 'Ed25519') {
        const hasKey = await invoke<boolean>('crypto_has_teacher_key');
        if (!hasKey) {
            // Show "Import Teacher Key" dialog
        }
    } else {
        // LDAP mode - no student-side setup needed
    }

    // Start student agent
    await invoke('start_student_agent', {
        config: { port: 3017, student_name: 'Student PC' }
    });
}

/**
 * Example 3: Admin Panel - Switch Authentication Mode
 */
export async function adminSwitchAuthMode(newMode: AuthMode) {
    const currentMode = await getAuthMode();

    if (currentMode === newMode) {
        console.log('Already in', newMode, 'mode');
        return;
    }

    // Confirm switch
    const confirmed = confirm(
        `Switch from ${currentMode} to ${newMode}? ` +
        `This will require reconfiguration.`
    );

    if (confirmed) {
        await setAuthMode(newMode);
        console.log('Switched to', newMode, 'mode');

        if (newMode === 'Ldap') {
            // Show LDAP configuration wizard
        } else {
            // Show Ed25519 keypair generation
        }
    }
}
