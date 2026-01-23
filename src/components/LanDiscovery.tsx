import { useState } from 'react';
import { useUdpAudio } from '../hooks/useUdpAudio';

interface LanDiscoveryProps {
  onDeviceSelected?: (ip: string, port: number) => void;
}

export function LanDiscovery({ onDeviceSelected }: LanDiscoveryProps) {
  const {
    discoveredDevices,
    savedDevices,
    isDiscovering,
    discoverDevices,
    removeDevice,
    connectToDevice,
    isClientConnected,
    clientIp,
    disconnectFromDevice,
  } = useUdpAudio();

  const [discoveryPort, setDiscoveryPort] = useState(5000);
  const [timeout, setTimeout] = useState(3000);

  const handleDiscover = () => {
    discoverDevices(discoveryPort, timeout);
  };

  const handleSelectDevice = (ip: string, port: number) => {
    connectToDevice(ip, port);
    if (onDeviceSelected) {
      onDeviceSelected(ip, port);
    }
  };

  return (
    <div className="lan-discovery">
      <div className="discovery-header">
        <h3>🔍 LAN Discovery</h3>
        <div className="discovery-controls">
          <input
            type="number"
            value={discoveryPort}
            onChange={(e) => setDiscoveryPort(Number(e.target.value))}
            placeholder="Port"
            className="port-input"
            min="1024"
            max="65535"
          />
          <input
            type="number"
            value={timeout}
            onChange={(e) => setTimeout(Number(e.target.value))}
            placeholder="Timeout (ms)"
            className="timeout-input"
            min="1000"
            max="10000"
          />
          <button
            onClick={handleDiscover}
            disabled={isDiscovering}
            className="btn primary"
          >
            {isDiscovering ? '🔍 Đang tìm kiếm...' : '🔍 Tìm kiếm thiết bị'}
          </button>
        </div>
      </div>

      {isClientConnected && (
        <div className="connected-device">
          <p>✅ Đã kết nối đến: {clientIp}</p>
          <button onClick={disconnectFromDevice} className="btn danger">
            Ngắt kết nối
          </button>
        </div>
      )}

      {discoveredDevices.length > 0 && (
        <div className="discovered-devices">
          <h4>Thiết bị đã tìm thấy ({discoveredDevices.length})</h4>
          <ul className="device-list">
            {discoveredDevices.map((device, index) => (
              <li key={index} className="device-item">
                <div className="device-info">
                  <span className="device-name">{device.name}</span>
                  <span className="device-ip">{device.ip}:{device.port}</span>
                </div>
                <button
                  onClick={() => handleSelectDevice(device.ip, device.port)}
                  className="btn secondary"
                  disabled={isClientConnected}
                >
                  Kết nối
                </button>
              </li>
            ))}
          </ul>
        </div>
      )}

      {savedDevices.length > 0 && (
        <div className="saved-devices">
          <h4>Thiết bị đã lưu ({savedDevices.length})</h4>
          <ul className="device-list">
            {savedDevices.map((device) => (
              <li key={device.id} className="device-item">
                <div className="device-info">
                  <span className="device-name">{device.name}</span>
                  <span className="device-ip">{device.ip}:{device.port}</span>
                </div>
                <div className="device-actions">
                  <button
                    onClick={() => handleSelectDevice(device.ip, device.port)}
                    className="btn secondary"
                    disabled={isClientConnected}
                  >
                    Kết nối
                  </button>
                  {device.id && (
                    <button
                      onClick={() => removeDevice(device.id!)}
                      className="btn danger"
                    >
                      Xóa
                    </button>
                  )}
                </div>
              </li>
            ))}
          </ul>
        </div>
      )}

      {discoveredDevices.length === 0 && savedDevices.length === 0 && !isDiscovering && (
        <div className="no-devices">
          <p>Chưa có thiết bị nào. Nhấn "Tìm kiếm thiết bị" để bắt đầu.</p>
          <div className="discovery-help" style={{ marginTop: '1rem', padding: '1rem', background: 'var(--bg)', borderRadius: '8px', fontSize: '0.9rem', color: 'var(--text-secondary)' }}>
            <p><strong>💡 Hướng dẫn:</strong></p>
            <ul style={{ margin: '0.5rem 0', paddingLeft: '1.5rem' }}>
              <li>Đảm bảo học sinh đã chọn chế độ "UDP Streaming"</li>
              <li>Tất cả thiết bị phải trong cùng mạng LAN</li>
              <li>Firewall không chặn UDP port {discoveryPort}</li>
              <li>Thử tăng timeout nếu mạng chậm</li>
            </ul>
          </div>
        </div>
      )}
    </div>
  );
}
