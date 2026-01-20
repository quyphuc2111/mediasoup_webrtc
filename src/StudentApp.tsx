import { useState } from 'react';
import { StudentView } from './components/StudentView';
import './App.css';

function StudentApp() {
  const [joined, setJoined] = useState(false);
  const [name, setName] = useState('');
  const [roomId, setRoomId] = useState('classroom-1');
  const [serverUrl, setServerUrl] = useState('');

  const handleJoin = () => {
    if (!name.trim()) {
      alert('Vui lòng nhập tên của bạn!');
      return;
    }
    if (!serverUrl.trim()) {
      alert('Vui lòng nhập Server URL từ giáo viên!');
      return;
    }
    setJoined(true);
  };

  if (joined) {
    return (
      <StudentView
        serverUrl={serverUrl}
        roomId={roomId}
        name={name}
        onDisconnect={() => setJoined(false)}
      />
    );
  }

  return (
    <main className="container">
      <h1>🖥️ Screen Sharing</h1>
      <p className="subtitle">Học sinh - Xem màn hình giáo viên</p>

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
          <label>Server URL (từ giáo viên):</label>
          <input
            type="text"
            value={serverUrl}
            onChange={(e) => setServerUrl(e.target.value)}
            placeholder="ws://192.168.1.x:3016"
          />
        </div>
      </div>

      <button onClick={handleJoin} className="btn primary full-width">
        🚀 Vào lớp học
      </button>
    </main>
  );
}

export default StudentApp;
