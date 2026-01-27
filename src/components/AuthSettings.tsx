/**
 * Authentication Settings Page
 * 
 * Allows users to:
 * - Choose between Ed25519 and LDAP authentication modes
 * - Configure LDAP server settings
 * - Test LDAP connection
 * - Manage Ed25519 keypairs
 */

import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import './AuthSettings.css';

type AuthMode = 'Ed25519' | 'Ldap';

interface LdapConfig {
    server_url: string;
    base_dn: string;
    user_filter: string;
    bind_dn_template: string;
    required_group: string | null;
    use_tls: boolean;
}

interface KeyPairInfo {
    public_key: string;
    private_key: string;
    fingerprint: string;
}

export const AuthSettings: React.FC = () => {
    const [authMode, setAuthMode] = useState<AuthMode>('Ed25519');
    const [ldapConfig, setLdapConfig] = useState<LdapConfig>({
        server_url: 'ldap://localhost:389',
        base_dn: 'DC=example,DC=com',
        user_filter: '(&(objectClass=user)(sAMAccountName={username}))',
        bind_dn_template: '{username}@example.com',
        required_group: 'CN=Teachers,OU=Groups,DC=example,DC=com',
        use_tls: false,
    });
    const [keypair, setKeypair] = useState<KeyPairInfo | null>(null);
    // const [hasKeypair, setHasKeypair] = useState(false); // Removed unused state
    const [loading, setLoading] = useState(false);
    const [message, setMessage] = useState<{ type: 'success' | 'error'; text: string } | null>(null);
    const [testingConnection, setTestingConnection] = useState(false);

    // Load current settings on mount
    useEffect(() => {
        loadCurrentSettings();
    }, []);

    const loadCurrentSettings = async () => {
        try {
            // Load auth mode
            const mode = await invoke<AuthMode>('auth_get_mode');
            setAuthMode(mode);

            // Load LDAP config if available
            try {
                const config = await invoke<LdapConfig>('ldap_load_config');
                setLdapConfig(config);
            } catch (e) {
                console.log('No LDAP config found, using defaults');
            }

            // Check if keypair exists
            const hasKey = await invoke<boolean>('crypto_has_keypair');
            // setHasKeypair(hasKey); 

            if (hasKey) {
                const kp = await invoke<KeyPairInfo>('crypto_load_keypair');
                setKeypair(kp);
            }
        } catch (error) {
            console.error('Failed to load settings:', error);
            showMessage('error', 'Failed to load current settings');
        }
    };

    const showMessage = (type: 'success' | 'error', text: string) => {
        setMessage({ type, text });
        setTimeout(() => setMessage(null), 5000);
    };

    const handleAuthModeChange = async (mode: AuthMode) => {
        setLoading(true);
        try {
            await invoke('auth_set_mode', { mode });
            setAuthMode(mode);
            showMessage('success', `Switched to ${mode} mode`);
        } catch (error) {
            showMessage('error', `Failed to switch mode: ${error}`);
        } finally {
            setLoading(false);
        }
    };

    const handleLdapConfigChange = (field: keyof LdapConfig, value: string | boolean | null) => {
        setLdapConfig(prev => ({
            ...prev,
            [field]: value,
        }));
    };

    const handleSaveLdapConfig = async () => {
        setLoading(true);
        try {
            await invoke('ldap_save_config', { config: ldapConfig });
            showMessage('success', 'LDAP configuration saved successfully');
        } catch (error) {
            showMessage('error', `Failed to save LDAP config: ${error}`);
        } finally {
            setLoading(false);
        }
    };

    const handleTestLdapConnection = async () => {
        // Validate URL format
        if (!ldapConfig.server_url.startsWith('ldap://') && !ldapConfig.server_url.startsWith('ldaps://')) {
            showMessage('error', 'Server URL must start with ldap:// or ldaps://');
            return;
        }

        setTestingConnection(true);
        try {
            const result = await invoke<string>('ldap_test_connection', { config: ldapConfig });
            showMessage('success', result);
        } catch (error) {
            showMessage('error', `Connection test failed: ${error}`);
        } finally {
            setTestingConnection(false);
        }
    };

    const handleGenerateKeypair = async () => {
        setLoading(true);
        try {
            const newKeypair = await invoke<KeyPairInfo>('crypto_generate_keypair');
            setKeypair(newKeypair);
            // setHasKeypair(true);
            showMessage('success', `Keypair generated! Fingerprint: ${newKeypair.fingerprint}`);
        } catch (error) {
            showMessage('error', `Failed to generate keypair: ${error}`);
        } finally {
            setLoading(false);
        }
    };

    const handleExportPublicKey = async () => {
        try {
            const publicKey = await invoke<string>('crypto_export_public_key');
            // Copy to clipboard
            await navigator.clipboard.writeText(publicKey);
            showMessage('success', 'Public key copied to clipboard!');
        } catch (error) {
            showMessage('error', `Failed to export public key: ${error}`);
        }
    };

    const loadExampleConfig = (type: 'activedirectory' | 'openldap' | 'ldaps') => {
        const examples = {
            activedirectory: {
                server_url: 'ldap://dc.school.local:389',
                base_dn: 'DC=school,DC=local',
                user_filter: '(&(objectClass=user)(sAMAccountName={username}))',
                bind_dn_template: '{username}@school.local',
                required_group: 'CN=Teachers,OU=Groups,DC=school,DC=local',
                use_tls: false,
            },
            openldap: {
                server_url: 'ldap://localhost:389',
                base_dn: 'dc=school,dc=local',
                user_filter: '(&(objectClass=inetOrgPerson)(uid={username}))',
                bind_dn_template: 'uid={username},ou=People,dc=school,dc=local',
                required_group: null,
                use_tls: false,
            },
            ldaps: {
                server_url: 'ldaps://ldap.school.local:636',
                base_dn: 'DC=school,DC=local',
                user_filter: '(&(objectClass=user)(sAMAccountName={username}))',
                bind_dn_template: '{username}@school.local',
                required_group: null,
                use_tls: true,
            },
        };
        setLdapConfig(examples[type]);
        showMessage('success', `Loaded ${type} example configuration`);
    };

    return (
        <div className="auth-settings">
            <h1>Authentication Settings</h1>

            {/* Message Display */}
            {message && (
                <div className={`message message-${message.type}`}>
                    {message.text}
                </div>
            )}

            {/* Auth Mode Selection */}
            <section className="settings-section">
                <h2>Authentication Mode</h2>
                <div className="auth-mode-selector">
                    <label className={`mode-option ${authMode === 'Ed25519' ? 'selected' : ''}`}>
                        <input
                            type="radio"
                            name="authMode"
                            value="Ed25519"
                            checked={authMode === 'Ed25519'}
                            onChange={() => handleAuthModeChange('Ed25519')}
                            disabled={loading}
                        />
                        <div className="mode-content">
                            <h3>🔑 Ed25519</h3>
                            <p className="mode-subtitle">Simple key-based authentication</p>
                            <p className="mode-description">Perfect for small classrooms. No server infrastructure required.</p>
                        </div>
                    </label>

                    <label className={`mode-option ${authMode === 'Ldap' ? 'selected' : ''}`}>
                        <input
                            type="radio"
                            name="authMode"
                            value="Ldap"
                            checked={authMode === 'Ldap'}
                            onChange={() => handleAuthModeChange('Ldap')}
                            disabled={loading}
                        />
                        <div className="mode-content">
                            <h3>🏢 LDAP/AD</h3>
                            <p className="mode-subtitle">Enterprise authentication</p>
                            <p className="mode-description">Centralized user management with Active Directory or OpenLDAP.</p>
                        </div>
                    </label>
                </div>
            </section>

            {/* Ed25519 Settings */}
            {authMode === 'Ed25519' && (
                <section className="settings-section">
                    <h2>Ed25519 Key Management</h2>

                    {keypair ? (
                        <div className="keypair-info">
                            <div className="info-row">
                                <strong>Fingerprint:</strong>
                                <code>{keypair.fingerprint}</code>
                            </div>
                            <div className="info-row">
                                <strong>Public Key:</strong>
                                <button onClick={handleExportPublicKey} className="btn btn-secondary">
                                    📋 Copy Public Key
                                </button>
                            </div>
                            <p className="help-text">
                                Share the public key with students so they can connect to you.
                            </p>
                        </div>
                    ) : (
                        <div className="no-keypair">
                            <p>No keypair found. Generate one to enable Ed25519 authentication.</p>
                            <button
                                onClick={handleGenerateKeypair}
                                disabled={loading}
                                className="btn btn-primary"
                            >
                                🔑 Generate Keypair
                            </button>
                        </div>
                    )}
                </section>
            )}

            {/* LDAP Settings */}
            {authMode === 'Ldap' && (
                <section className="settings-section">
                    <h2>LDAP Configuration</h2>

                    {/* Example Templates */}
                    <div className="example-configs">
                        <p>Load example configuration:</p>
                        <div className="example-buttons">
                            <button onClick={() => loadExampleConfig('activedirectory')} className="btn btn-sm">
                                Active Directory
                            </button>
                            <button onClick={() => loadExampleConfig('openldap')} className="btn btn-sm">
                                OpenLDAP
                            </button>
                            <button onClick={() => loadExampleConfig('ldaps')} className="btn btn-sm">
                                LDAPS (Secure)
                            </button>
                        </div>
                    </div>

                    {/* LDAP Form */}
                    <div className="ldap-form">
                        <div className="form-group">
                            <label htmlFor="server_url">
                                LDAP Server URL
                                <span className="help-icon" title="e.g., ldap://dc.school.local:389 or ldaps://ldap.school.local:636">
                                    ℹ️
                                </span>
                            </label>
                            <input
                                id="server_url"
                                type="text"
                                value={ldapConfig.server_url}
                                onChange={e => handleLdapConfigChange('server_url', e.target.value)}
                                placeholder="ldap://dc.school.local:389"
                            />
                        </div>

                        <div className="form-group">
                            <label htmlFor="base_dn">
                                Base DN
                                <span className="help-icon" title="The base distinguished name for searches">
                                    ℹ️
                                </span>
                            </label>
                            <input
                                id="base_dn"
                                type="text"
                                value={ldapConfig.base_dn}
                                onChange={e => handleLdapConfigChange('base_dn', e.target.value)}
                                placeholder="DC=school,DC=local"
                            />
                        </div>

                        <div className="form-group">
                            <label htmlFor="user_filter">
                                User Search Filter
                                <span className="help-icon" title="{username} will be replaced with the actual username">
                                    ℹ️
                                </span>
                            </label>
                            <input
                                id="user_filter"
                                type="text"
                                value={ldapConfig.user_filter}
                                onChange={e => handleLdapConfigChange('user_filter', e.target.value)}
                                placeholder="(&(objectClass=user)(sAMAccountName={username}))"
                            />
                        </div>

                        <div className="form-group">
                            <label htmlFor="bind_dn_template">
                                Bind DN Template
                                <span className="help-icon" title="Template for user authentication">
                                    ℹ️
                                </span>
                            </label>
                            <input
                                id="bind_dn_template"
                                type="text"
                                value={ldapConfig.bind_dn_template}
                                onChange={e => handleLdapConfigChange('bind_dn_template', e.target.value)}
                                placeholder="{username}@school.local"
                            />
                        </div>

                        <div className="form-group">
                            <label htmlFor="required_group">
                                Required Group (Optional)
                                <span className="help-icon" title="Only users in this group can authenticate. Leave empty to allow all users.">
                                    ℹ️
                                </span>
                            </label>
                            <input
                                id="required_group"
                                type="text"
                                value={ldapConfig.required_group || ''}
                                onChange={e => handleLdapConfigChange('required_group', e.target.value || null)}
                                placeholder="CN=Teachers,OU=Groups,DC=school,DC=local"
                            />
                        </div>

                        <div className="form-group checkbox">
                            <label>
                                <input
                                    type="checkbox"
                                    checked={ldapConfig.use_tls}
                                    onChange={e => handleLdapConfigChange('use_tls', e.target.checked)}
                                />
                                Use TLS/SSL (recommended for production)
                            </label>
                        </div>

                        {/* Action Buttons */}
                        <div className="form-actions">
                            <button
                                onClick={handleTestLdapConnection}
                                disabled={testingConnection || loading}
                                className="btn btn-secondary"
                            >
                                {testingConnection ? '⏳ Testing...' : '🔌 Test Connection'}
                            </button>
                            <button
                                onClick={handleSaveLdapConfig}
                                disabled={loading}
                                className="btn btn-primary"
                            >
                                💾 Save Configuration
                            </button>
                        </div>
                    </div>
                </section>
            )}

            {/* Info Panel */}
            <section className="settings-section info-panel">
                <h3>ℹ️ Important Notes</h3>
                <ul>
                    <li>
                        <strong>Ed25519 Mode:</strong> Students must import your public key before connecting.
                    </li>
                    <li>
                        <strong>LDAP Mode:</strong> Students must have LDAP configured with the same server settings.
                    </li>
                    <li>
                        Changing authentication mode requires restarting the student agent.
                    </li>
                    <li>
                        Test your configuration before deploying to production.
                    </li>
                </ul>
            </section>
        </div>
    );
};

export default AuthSettings;
