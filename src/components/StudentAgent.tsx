import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';

type AgentStatus = 
  | 'Stopped'
  | 'Starting'
  | 'WaitingForTeacher'
  | 'Authenticating'
  | { Connected: { teacher_name: string } }
  | { Error: { message: string } };

interface AgentConfig {
  port: number;
  student_name: string;
}

interface StudentAgentProps {
  onBack?: () => void;
}

export function StudentAgent({ onBack }: StudentAgentProps) {
  const [status, setStatus] = useState<AgentStatus>('Stopped');
  const [config, setConfig] = useState<AgentConfig>({ port: 3017, student_name: '' });
  const [hasTeacherKey, setHasTeacherKey] = useState(false);
  const [showKeyImport, setShowKeyImport] = useState(false);
  const [keyInput, setKeyInput] = useState('');
  const [error, setError] = useState<string | null>(null);

  // Check initial state
  useEffect(() => {
    checkTeacherKey();
    checkAgentStatus();
    
    // Poll status every second
    const interval = setInterval(checkAgentStatus, 1000);
    return () => clearInterval(interval);
  }, []);

  const checkTeacherKey = async () => {
    try {
      const hasKey = await invoke<boolean>('crypto_has_teacher_key');
      setHasTeacherKey(hasKey);
    } catch (e) {
      console.error('Failed to check teacher key:', e);
    }
  };

  const checkAgentStatus = async () => {
    try {
      const agentStatus = await invoke<AgentStatus>('get_agent_status');
      setStatus(agentStatus);
    } catch (e) {
      console.error('Failed to get agent status:', e);
    }
  };

  const importTeacherKey = async () => {
    try {
      setError(null);
      await invoke('crypto_import_teacher_key', { keyData: keyInput });
      setHasTeacherKey(true);
      setShowKeyImport(false);
      setKeyInput('');
    } catch (e) {
      setError(String(e));
    }
  };

  const startAgent = async () => {
    try {
      setError(null);
      
      // Start the student agent (WebSocket server)
      await invoke('start_student_agent', {
        port: config.port,
        studentName: config.student_name || 'Student',
      });
      
      // Also start the discovery listener so teacher can find us on LAN
      try {
        await invoke('start_discovery_listener', {
          name: config.student_name || 'Student',
          port: config.port,
        });
      } catch (discoveryError) {
        console.warn('Failed to start discovery listener:', discoveryError);
        // Don't fail the whole agent start if discovery fails
      }
    } catch (e) {
      setError(String(e));
    }
  };

  const stopAgent = async () => {
    try {
      // Stop discovery listener
      try {
        await invoke('stop_discovery_listener');
      } catch (e) {
        console.warn('Failed to stop discovery listener:', e);
      }
      
      // Stop student agent
      await invoke('stop_student_agent');
    } catch (e) {
      console.error('Failed to stop agent:', e);
    }
  };

  const getStatusText = useCallback(() => {
    if (typeof status === 'string') {
      switch (status) {
        case 'Stopped': return '🔴 Đã dừng';
        case 'Starting': return '🟡 Đang khởi động...';
        case 'WaitingForTeacher': return '🟢 Sẵn sàng - Đang chờ giáo viên kết nối...';
        case 'Authenticating': return '🟡 Đang xác thực giáo viên...';
        default: return status;
      }
    }
    if ('Connected' in status) {
      return `🟢 Đã kết nối với ${status.Connected.teacher_name}`;
    }
    if ('Error' in status) {
      return `❌ Lỗi: ${status.Error.message}`;
    }
    return 'Unknown';
  }, [status]);

  const isRunning = typeof status === 'string' 
    ? status !== 'Stopped'
    : 'Connected' in status || 'Error' in status;

  return (
    <div className="student-agent">
      {onBack && (
        <button onClick={onBack} className="btn back-btn">
          ← Quay lại
        </button>
      )}

      <h1>🖥️ Student Agent</h1>
      <p className="subtitle">Cho phép giáo viên xem màn hình của bạn</p>

      {/* Teacher Key Setup */}
      <div className="info-box">
        <h3>🔑 Khóa Giáo viên</h3>
        {hasTeacherKey ? (
          <div className="key-status success">
            <span>✅ Đã cấu hình khóa giáo viên</span>
            <button 
              onClick={() => setShowKeyImport(true)} 
              className="btn secondary small"
            >
              Đổi khóa
            </button>
          </div>
        ) : (
          <div className="key-status warning">
            <span>⚠️ Chưa có khóa giáo viên</span>
            <button 
              onClick={() => setShowKeyImport(true)} 
              className="btn primary small"
            >
              Nhập khóa
            </button>
          </div>
        )}

        {showKeyImport && (
          <div className="key-import-modal">
            <h4>Nhập khóa công khai của giáo viên</h4>
            <p className="hint">Dán khóa mà giáo viên đã chia sẻ cho bạn</p>
            <textarea
              value={keyInput}
              onChange={(e) => setKeyInput(e.target.value)}
              placeholder="-----BEGIN SMARTLAB PUBLIC KEY-----&#10;...&#10;-----END SMARTLAB PUBLIC KEY-----"
              rows={5}
            />
            <div className="modal-actions">
              <button onClick={() => setShowKeyImport(false)} className="btn secondary">
                Hủy
              </button>
              <button 
                onClick={importTeacherKey} 
                className="btn primary"
                disabled={!keyInput.trim()}
              >
                Lưu khóa
              </button>
            </div>
          </div>
        )}
      </div>

      {/* Agent Configuration */}
      <div className="form-section">
        <div className="form-group">
          <label>Tên của bạn:</label>
          <input
            type="text"
            value={config.student_name}
            onChange={(e) => setConfig({ ...config, student_name: e.target.value })}
            placeholder="Nhập tên..."
            disabled={isRunning}
          />
        </div>

        <div className="form-group">
          <label>Port:</label>
          <input
            type="number"
            value={config.port}
            onChange={(e) => setConfig({ ...config, port: parseInt(e.target.value) || 3017 })}
            placeholder="3017"
            disabled={isRunning}
          />
          <small>Port để giáo viên kết nối (mặc định: 3017)</small>
        </div>
      </div>

      {/* Status */}
      <div className="server-section">
        <h3>📡 Trạng thái Agent</h3>
        <div className={`status-indicator ${isRunning ? 'running' : 'stopped'}`}>
          {getStatusText()}
        </div>

        {error && (
          <div className="error-box">
            <p>{error}</p>
          </div>
        )}

        <div className="server-controls">
          {!isRunning ? (
            <button 
              onClick={startAgent} 
              className="btn primary full-width"
              disabled={!hasTeacherKey}
            >
              ▶️ Bắt đầu Agent
            </button>
          ) : (
            <button onClick={stopAgent} className="btn danger full-width">
              ⏹️ Dừng Agent
            </button>
          )}
        </div>

        {!hasTeacherKey && !isRunning && (
          <p className="hint warning-hint">
            ⚠️ Bạn cần nhập khóa giáo viên trước khi bắt đầu
          </p>
        )}
      </div>

      {/* Instructions */}
      {isRunning && typeof status === 'string' && status === 'WaitingForTeacher' && (
        <div className="info-box">
          <h3>📋 Hướng dẫn</h3>
          <p>Agent đang chạy và sẵn sàng nhận kết nối từ giáo viên.</p>
          <p>Giáo viên sẽ kết nối đến máy của bạn qua IP và port {config.port}.</p>
          <p>Khi giáo viên kết nối, màn hình của bạn sẽ được chia sẻ tự động.</p>
        </div>
      )}
    </div>
  );
}

export default StudentAgent;
