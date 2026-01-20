import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { TeacherView } from './components/TeacherView';
import { StudentView } from './components/StudentView';
import './App.css';

type Mode = 'select' | 'teacher' | 'student';

interface ServerInfo {
  url: string;
  ip: string;
  port: number;
}

function App() {
  const [mode, setMode] = useState<Mode>('select');
  const [name, setName] = useState('');
  const [roomId, setRoomId] = useState('classroom-1');
  const [serverUrl, setServerUrl] = useState('ws://localhost:3016');
  const [serverInfo, setServerInfo] = useState<ServerInfo | null>(null);
  const [serverStatus, setServerStatus] = useState<'starting' | 'running' | 'stopped' | 'error'>('stopped');
  const [serverError, setServerError] = useState<string | null>(null);

  useEffect(() => {
    // Check if server is already running
    checkServerStatus();
  }, []);

  const checkServerStatus = async () => {
    try {
      const info = await invoke<ServerInfo>('get_server_info');
      setServerInfo(info);
      setServerUrl(info.url);
      setServerStatus('running');
    } catch {
      setServerStatus('stopped');
    }
  };

  const startServer = async () => {
    setServerStatus('starting');
    setServerError(null);
    try {
      const info = await invoke<ServerInfo>('start_server');
      setServerInfo(info);
      setServerUrl(info.url);
      setServerStatus('running');
    } catch (error) {
      console.error('Failed to start server:', error);
      setServerStatus('error');
      setServerError(String(error));
    }
  };

  const stopServer = async () => {
    try {
      await invoke('stop_server');
      setServerInfo(null);
      setServerStatus('stopped');
    } catch (error) {
      console.error('Failed to stop server:', error);
    }
  };

  const handleSelectMode = (selectedMode: 'teacher' | 'student') => {
    if (!name.trim()) {
      alert('Vui lòng nhập tên của bạn!');
      return;
    }
    if (!serverUrl.trim()) {
      alert('Vui lòng nhập Server URL!');
      return;
    }
    setMode(selectedMode);
  };

  const handleBack = () => {
    setMode('select');
  };

  if (mode === 'teacher') {
    return (
      <TeacherView
        serverUrl={serverUrl}
        roomId={roomId}
        name={name}
        onDisconnect={handleBack}
      />
    );
  }

  if (mode === 'student') {
    return (
      <StudentView
        serverUrl={serverUrl}
        roomId={roomId}
        name={name}
        onDisconnect={handleBack}
      />
    );
  }

  return (
    <main className="container">
      <h1>🖥️ Screen Sharing</h1>
      <p className="subtitle">Chia sẻ màn hình cho lớp học</p>

      <div className="server-section">
        <h3>🖧 Mediasoup Server</h3>
        <div className={`status-indicator ${serverStatus}`}>
          {serverStatus === 'running' && '🟢 Server đang chạy'}
          {serverStatus === 'starting' && '🟡 Đang khởi động...'}
          {serverStatus === 'stopped' && '🔴 Server chưa chạy'}
          {serverStatus === 'error' && '❌ Lỗi khởi động server'}
        </div>
        
        {serverError && (
          <div className="error-box">
            <p>{serverError}</p>
          </div>
        )}

        {serverInfo && (
          <div className="server-info">
            <p>IP: <strong>{serverInfo.ip}</strong></p>
            <p>Port: <strong>{serverInfo.port}</strong></p>
            <p className="share-url">
              Học sinh kết nối: <code>{serverInfo.url}</code>
            </p>
          </div>
        )}

        <div className="server-controls">
          {serverStatus === 'stopped' && (
            <button onClick={startServer} className="btn primary">
              ▶️ Khởi động Server
            </button>
          )}
          {serverStatus === 'error' && (
            <button onClick={startServer} className="btn primary">
              🔄 Thử lại
            </button>
          )}
          {serverStatus === 'running' && (
            <button onClick={stopServer} className="btn danger">
              ⏹️ Dừng Server
            </button>
          )}
        </div>
      </div>

      <div className="form-section">
        <div className="form-group">
          <label>Tên của bạn:</label>
          <input
            type="text"
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="Nhập tên..."
          />
        </div>

        <div className="form-group">
          <label>Room ID:</label>
          <input
            type="text"
            value={roomId}
            onChange={(e) => setRoomId(e.target.value)}
            placeholder="classroom-1"
          />
        </div>

        <div className="form-group">
          <label>Server URL:</label>
          <input
            type="text"
            value={serverUrl}
            onChange={(e) => setServerUrl(e.target.value)}
            placeholder="ws://192.168.1.x:3016"
          />
          <small>Giáo viên: dùng URL ở trên | Học sinh: nhập URL từ giáo viên</small>
        </div>
      </div>

      <div className="mode-selection">
        <button 
          onClick={() => handleSelectMode('teacher')} 
          className="btn mode-btn teacher"
          disabled={serverStatus !== 'running'}
        >
          👨‍🏫 Giáo viên
          <span>Chia sẻ màn hình</span>
        </button>
        
        <button 
          onClick={() => handleSelectMode('student')} 
          className="btn mode-btn student"
        >
          👨‍🎓 Học sinh
          <span>Xem màn hình</span>
        </button>
      </div>
    </main>
  );
}

export default App;
