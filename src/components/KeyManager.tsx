import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface KeyPairInfo {
  public_key: string;
  private_key: string;
  fingerprint: string;
}

interface KeyEntry {
  name: string;
  type: 'private' | 'public';
  pairId: string;
}

interface KeyManagerProps {
  onClose: () => void;
}

export function KeyManager({ onClose }: KeyManagerProps) {
  const [keypair, setKeypair] = useState<KeyPairInfo | null>(null);
  const [keys, setKeys] = useState<KeyEntry[]>([]);
  const [selectedKey, setSelectedKey] = useState<KeyEntry | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [showExportModal, setShowExportModal] = useState(false);
  const [exportedKey, setExportedKey] = useState('');
  const [showImportModal, setShowImportModal] = useState(false);
  const [importKeyData, setImportKeyData] = useState('');
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    loadKeypair();
  }, []);

  const loadKeypair = async () => {
    setLoading(true);
    try {
      const kp = await invoke<KeyPairInfo>('crypto_load_keypair');
      setKeypair(kp);
      
      // Build key list from keypair
      const pairId = kp.fingerprint.replace('ED25519:', '');
      setKeys([
        { name: 'teacher', type: 'private', pairId },
        { name: 'teacher', type: 'public', pairId },
      ]);
    } catch (e) {
      // No keypair exists yet
      console.log('No keypair found:', e);
      setKeys([]);
    } finally {
      setLoading(false);
    }
  };

  const createKeyPair = async () => {
    setLoading(true);
    setError(null);
    try {
      const kp = await invoke<KeyPairInfo>('crypto_generate_keypair');
      setKeypair(kp);
      
      const pairId = kp.fingerprint.replace('ED25519:', '');
      setKeys([
        { name: 'teacher', type: 'private', pairId },
        { name: 'teacher', type: 'public', pairId },
      ]);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  const deleteKey = async () => {
    if (!selectedKey) return;
    
    // For now, we delete the entire keypair (both keys)
    // In a real implementation, you might handle this differently
    setKeys([]);
    setKeypair(null);
    setSelectedKey(null);
  };

  const exportKey = async () => {
    if (!selectedKey || !keypair) return;
    
    try {
      if (selectedKey.type === 'public') {
        const exported = await invoke<string>('crypto_export_public_key');
        setExportedKey(exported);
      } else {
        // For private key, we don't export it for security
        setExportedKey('⚠️ Khóa riêng tư không được export vì lý do bảo mật.');
      }
      setShowExportModal(true);
    } catch (e) {
      setError(String(e));
    }
  };

  const importKey = async () => {
    if (!importKeyData.trim()) return;
    
    try {
      await invoke('crypto_import_teacher_key', { keyData: importKeyData });
      setShowImportModal(false);
      setImportKeyData('');
      loadKeypair();
    } catch (e) {
      setError(String(e));
    }
  };

  const copyToClipboard = async () => {
    try {
      await navigator.clipboard.writeText(exportedKey);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (e) {
      console.error('Failed to copy:', e);
    }
  };

  return (
    <div className="modal-overlay">
      <div className="modal key-manager-modal veyon-style">
        <div className="modal-header">
          <h2>Authentication Keys</h2>
          <button onClick={onClose} className="btn close-btn">✕</button>
        </div>

        {/* Introduction Section */}
        <div className="intro-section">
          <h4>Hướng dẫn</h4>
          <div className="intro-content">
            <p>Thực hiện các bước sau để thiết lập xác thực bằng khóa:</p>
            <ol>
              <li>Tạo cặp khóa trên máy giáo viên (master).</li>
              <li>Export khóa public và gửi cho học sinh.</li>
              <li>Học sinh import khóa public vào máy của họ.</li>
            </ol>
          </div>
        </div>

        {/* Error Message */}
        {error && (
          <div className="error-box">
            <p>{error}</p>
            <button onClick={() => setError(null)} className="btn small">✕</button>
          </div>
        )}

        {/* Keys Table Section */}
        <div className="keys-section">
          <h4>Danh sách khóa xác thực</h4>
          
          <div className="keys-table-container">
            <table className="keys-table">
              <thead>
                <tr>
                  <th>Tên</th>
                  <th>Loại</th>
                  <th>Pair ID</th>
                </tr>
              </thead>
              <tbody>
                {loading ? (
                  <tr>
                    <td colSpan={3} className="table-empty">Đang tải...</td>
                  </tr>
                ) : keys.length === 0 ? (
                  <tr>
                    <td colSpan={3} className="table-empty">Chưa có khóa nào</td>
                  </tr>
                ) : (
                  keys.map((key, index) => (
                    <tr 
                      key={`${key.name}-${key.type}-${index}`}
                      className={selectedKey === key ? 'selected' : ''}
                      onClick={() => setSelectedKey(key)}
                    >
                      <td>{key.name}</td>
                      <td>
                        <span className={`key-type ${key.type}`}>
                          {key.type === 'private' ? '🔐 private' : '🔓 public'}
                        </span>
                      </td>
                      <td className="pair-id">{key.pairId}</td>
                    </tr>
                  ))
                )}
              </tbody>
            </table>

            {/* Action Buttons */}
            <div className="keys-actions">
              <button onClick={createKeyPair} className="btn action-btn">
                Tạo cặp khóa
              </button>
              <button 
                onClick={deleteKey} 
                className="btn action-btn"
                disabled={!selectedKey}
              >
                Xóa khóa
              </button>
              <button 
                onClick={() => setShowImportModal(true)} 
                className="btn action-btn"
              >
                Import khóa
              </button>
              <button 
                onClick={exportKey} 
                className="btn action-btn"
                disabled={!selectedKey}
              >
                Export khóa
              </button>
            </div>
          </div>
        </div>

        {/* Footer Buttons */}
        <div className="modal-footer">
          <button onClick={onClose} className="btn secondary">
            Đóng
          </button>
        </div>

        {/* Export Modal */}
        {showExportModal && (
          <div className="sub-modal-overlay">
            <div className="sub-modal">
              <h3>Export Khóa</h3>
              <p className="hint">
                {selectedKey?.type === 'public' 
                  ? 'Copy khóa công khai này và gửi cho học sinh:' 
                  : 'Khóa riêng tư:'}
              </p>
              <textarea
                readOnly
                value={exportedKey}
                rows={6}
                className="export-textarea"
              />
              <div className="sub-modal-actions">
                <button onClick={() => setShowExportModal(false)} className="btn secondary">
                  Đóng
                </button>
                {selectedKey?.type === 'public' && (
                  <button onClick={copyToClipboard} className="btn primary">
                    {copied ? '✅ Đã copy!' : '📋 Copy'}
                  </button>
                )}
              </div>
            </div>
          </div>
        )}

        {/* Import Modal */}
        {showImportModal && (
          <div className="sub-modal-overlay">
            <div className="sub-modal">
              <h3>Import Khóa</h3>
              <p className="hint">Dán khóa công khai của giáo viên vào đây:</p>
              <textarea
                value={importKeyData}
                onChange={(e) => setImportKeyData(e.target.value)}
                rows={6}
                className="import-textarea"
                placeholder="-----BEGIN SMARTLAB PUBLIC KEY-----&#10;...&#10;-----END SMARTLAB PUBLIC KEY-----"
              />
              <div className="sub-modal-actions">
                <button onClick={() => setShowImportModal(false)} className="btn secondary">
                  Hủy
                </button>
                <button 
                  onClick={importKey} 
                  className="btn primary"
                  disabled={!importKeyData.trim()}
                >
                  Import
                </button>
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

export default KeyManager;
