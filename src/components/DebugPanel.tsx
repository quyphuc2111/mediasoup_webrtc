import { useState, useEffect, useRef } from 'react';
import { listen } from '@tauri-apps/api/event';

interface LogEntry {
  id: number;
  timestamp: string;
  level: 'info' | 'warn' | 'error' | 'success';
  message: string;
}

export function DebugPanel() {
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [isOpen, setIsOpen] = useState(false);
  const [autoScroll, setAutoScroll] = useState(true);
  const logEndRef = useRef<HTMLDivElement>(null);
  const logIdRef = useRef(0);

  useEffect(() => {
    // Intercept console methods
    const originalLog = console.log;
    const originalWarn = console.warn;
    const originalError = console.error;

    const addLog = (level: 'info' | 'warn' | 'error' | 'success', ...args: any[]) => {
      const message = args.map(arg => {
        if (typeof arg === 'object') {
          try {
            return JSON.stringify(arg, null, 2);
          } catch {
            return String(arg);
          }
        }
        return String(arg);
      }).join(' ');

      const now = new Date();
      const timestamp = `${now.getHours().toString().padStart(2, '0')}:${now.getMinutes().toString().padStart(2, '0')}:${now.getSeconds().toString().padStart(2, '0')}.${now.getMilliseconds().toString().padStart(3, '0')}`;

      setLogs(prev => {
        const newLogs = [...prev, {
          id: logIdRef.current++,
          timestamp,
          level,
          message,
        }];
        // Keep only last 100 logs
        return newLogs.slice(-100);
      });
    };

    console.log = (...args: any[]) => {
      originalLog(...args);
      // Filter out noisy logs - only show important ones
      const msg = args.join(' ');
      const importantKeywords = [
        '[Discovery]',
        '[UDP Audio]',
        '[Student]',
        '[Teacher]',
        '[MediasoupClient]',
        '[ScreenShare]',
        '[ToggleScreenAudio]',
        '[TeacherConnector]',
        '[StudentAgent]',
        '[Crypto]',
        '[ViewClient]',
        '[H264Player]',
        '[H264Encoder]',
        '[ScreenCapture]',
        '[H264]',
        'Discovery',
        'UDP',
        'LAN',
        'device',
        'listener',
        'broadcast',
        'responded',
        'Found device',
        'Error',
        'Failed',
        'Started',
        'Stopped',
        'Connecting',
        'Connected',
        'WebSocket',
        'Authentication',
        'keypair',
        'challenge',
        'signature',
        'H.264',
        'keyframe',
        'SPS',
        'PPS',
        'AVCC',
        'WebCodecs',
        'decoder',
        'encoder',
        'Frame',
        'bitstream',
        'NAL',
      ];
      
      if (importantKeywords.some(keyword => msg.includes(keyword))) {
        addLog('info', ...args);
      }
    };

    console.warn = (...args: any[]) => {
      originalWarn(...args);
      addLog('warn', ...args);
    };

    console.error = (...args: any[]) => {
      originalError(...args);
      addLog('error', ...args);
    };

    // Custom debug log helpers for UI components (bypass filters)
    (window as any).debugInfo = (...args: any[]) => {
      addLog('info', ...args);
    };
    (window as any).debugWarn = (...args: any[]) => {
      addLog('warn', ...args);
    };
    (window as any).debugError = (...args: any[]) => {
      addLog('error', ...args);
    };
    (window as any).debugSuccess = (...args: any[]) => {
      addLog('success', ...args);
    };

    // Listen for logs from Rust backend
    const setupTauriLogListener = async () => {
      try {
        const unlisten = await listen<{ level: string; message: string; timestamp: number }>('debug-log', (event) => {
          const { level, message } = event.payload;
          const levelMap: Record<string, 'info' | 'warn' | 'error' | 'success'> = {
            'info': 'info',
            'warn': 'warn',
            'error': 'error',
            'success': 'success',
          };
          addLog(levelMap[level] || 'info', message);
        });
        return unlisten;
      } catch (e) {
        console.warn('Failed to setup Tauri log listener:', e);
        return () => {};
      }
    };

    let unlistenTauri: (() => void) | null = null;
    setupTauriLogListener().then(unlisten => {
      unlistenTauri = unlisten;
    });

    return () => {
      console.log = originalLog;
      console.warn = originalWarn;
      console.error = originalError;
      if (unlistenTauri) {
        unlistenTauri();
      }
      delete (window as any).debugInfo;
      delete (window as any).debugWarn;
      delete (window as any).debugError;
      delete (window as any).debugSuccess;
    };
  }, []);

  useEffect(() => {
    if (autoScroll && logEndRef.current) {
      logEndRef.current.scrollIntoView({ behavior: 'smooth' });
    }
  }, [logs, autoScroll]);

  const clearLogs = () => {
    setLogs([]);
    logIdRef.current = 0;
  };

  const getLogColor = (level: string) => {
    switch (level) {
      case 'error': return 'var(--danger)';
      case 'warn': return 'var(--warning)';
      case 'success': return 'var(--success)';
      default: return 'var(--text)';
    }
  };

  const getLogIcon = (level: string) => {
    switch (level) {
      case 'error': return '❌';
      case 'warn': return '⚠️';
      case 'success': return '✅';
      default: return 'ℹ️';
    }
  };

  return (
    <>
      <button
        onClick={() => setIsOpen(!isOpen)}
        className="debug-toggle-btn"
        style={{
          position: 'fixed',
          bottom: '20px',
          right: '20px',
          zIndex: 10000,
          padding: '0.5rem 1rem',
          background: isOpen ? 'var(--danger)' : 'var(--bg-secondary)',
          color: 'var(--text)',
          border: '1px solid var(--border)',
          borderRadius: '8px',
          cursor: 'pointer',
          fontSize: '0.9rem',
          boxShadow: '0 2px 8px rgba(0,0,0,0.3)',
        }}
      >
        {isOpen ? '🔴 Đóng Debug' : '🐛 Debug'}
        {logs.length > 0 && (
          <span style={{
            marginLeft: '0.5rem',
            background: 'var(--danger)',
            color: 'white',
            padding: '0.1rem 0.4rem',
            borderRadius: '10px',
            fontSize: '0.75rem',
          }}>
            {logs.length}
          </span>
        )}
      </button>

      {isOpen && (
        <div className="debug-panel" style={{
          position: 'fixed',
          bottom: '70px',
          right: '20px',
          width: '600px',
          maxHeight: '500px',
          background: 'var(--bg)',
          border: '2px solid var(--border)',
          borderRadius: '12px',
          zIndex: 9999,
          display: 'flex',
          flexDirection: 'column',
          boxShadow: '0 4px 20px rgba(0,0,0,0.5)',
        }}>
          <div style={{
            padding: '1rem',
            borderBottom: '1px solid var(--border)',
            display: 'flex',
            justifyContent: 'space-between',
            alignItems: 'center',
            background: 'var(--bg-secondary)',
            borderRadius: '12px 12px 0 0',
          }}>
            <h3 style={{ margin: 0, fontSize: '1rem' }}>🐛 Debug Console</h3>
            <div style={{ display: 'flex', gap: '0.5rem', alignItems: 'center' }}>
              <label style={{ fontSize: '0.85rem', display: 'flex', alignItems: 'center', gap: '0.25rem' }}>
                <input
                  type="checkbox"
                  checked={autoScroll}
                  onChange={(e) => setAutoScroll(e.target.checked)}
                  style={{ cursor: 'pointer' }}
                />
                Auto-scroll
              </label>
              <button
                onClick={clearLogs}
                className="btn danger"
                style={{ padding: '0.25rem 0.75rem', fontSize: '0.85rem' }}
              >
                Xóa
              </button>
            </div>
          </div>

          <div style={{
            flex: 1,
            overflowY: 'auto',
            padding: '0.5rem',
            fontFamily: 'monospace',
            fontSize: '0.85rem',
            background: '#1a1a1a',
            color: '#e0e0e0',
            wordBreak: 'break-word',
          }}>
            {logs.length === 0 ? (
              <div style={{ padding: '2rem', textAlign: 'center', color: 'var(--text-secondary)' }}>
                Chưa có logs. Thực hiện các thao tác để xem logs...
              </div>
            ) : (
              logs.map((log) => (
                <div
                  key={log.id}
                  style={{
                    marginBottom: '0.25rem',
                    padding: '0.25rem 0.5rem',
                    borderRadius: '4px',
                    background: log.level === 'error' ? 'rgba(239, 68, 68, 0.1)' :
                                log.level === 'warn' ? 'rgba(245, 158, 11, 0.1)' :
                                log.level === 'success' ? 'rgba(34, 197, 94, 0.1)' :
                                'transparent',
                    borderLeft: `3px solid ${getLogColor(log.level)}`,
                  }}
                >
                  <span style={{ color: 'var(--text-secondary)', fontSize: '0.75rem' }}>
                    [{log.timestamp}]
                  </span>
                  <span style={{ margin: '0 0.5rem', color: getLogColor(log.level) }}>
                    {getLogIcon(log.level)}
                  </span>
                  <span style={{ color: getLogColor(log.level), whiteSpace: 'pre-wrap' }}>
                    {log.message}
                  </span>
                </div>
              ))
            )}
            <div ref={logEndRef} />
          </div>

          <div style={{
            padding: '0.5rem 1rem',
            borderTop: '1px solid var(--border)',
            fontSize: '0.75rem',
            color: 'var(--text-secondary)',
            background: 'var(--bg-secondary)',
            borderRadius: '0 0 12px 12px',
          }}>
            Tổng: {logs.length} logs | 
            Errors: {logs.filter(l => l.level === 'error').length} | 
            Warnings: {logs.filter(l => l.level === 'warn').length}
          </div>
        </div>
      )}
    </>
  );
}
