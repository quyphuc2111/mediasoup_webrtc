import React, { useState, useEffect, useCallback, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { 
  Monitor, Grid, List, CheckCircle2, 
  Eye, MousePointer, FolderOpen, Wifi, WifiOff, RefreshCw, Loader2,
  Trash2, Link, Unlink, Maximize2, X, Minimize2, Plus, Power, RotateCcw, Lock, LogOut, MoreVertical
} from 'lucide-react';
import { RoomComputer } from '../types';
import { H264VideoPlayer } from '../components/H264VideoPlayer';
import { FileManager } from '../components/FileManager';

interface LabControlProps {
  // No props needed - all handled internally
}

interface DiscoveredDevice {
  ip: string;
  name: string;
  port: number;
  last_seen: number;
}

interface SavedDevice {
  id: number | null;
  ip: string;
  name: string;
  port: number;
  last_used: number;
}

type ConnectionStatus =
  | 'Disconnected'
  | 'Connecting'
  | 'Authenticating'
  | 'Connected'
  | 'Viewing'
  | { Error: { message: string } };

interface StudentConnection {
  id: string;
  ip: string;
  port: number;
  name: string | null;
  status: ConnectionStatus;
}

interface ScreenFrame {
  data?: string | null;
  data_binary?: number[] | null;
  sps_pps?: number[] | null;
  timestamp: number;
  width: number;
  height: number;
  is_keyframe: boolean;
  codec: string;
}

interface MouseInputEvent {
  event_type: 'move' | 'click' | 'scroll' | 'down' | 'up';
  x: number;
  y: number;
  button?: 'left' | 'right' | 'middle';
  delta_x?: number;
  delta_y?: number;
}

interface KeyboardInputEvent {
  event_type: 'keydown' | 'keyup';
  key: string;
  code: string;
  modifiers: {
    ctrl: boolean;
    alt: boolean;
    shift: boolean;
    meta: boolean;
  };
}

// Fullscreen/Modal view mode
type ViewMode = 'grid' | 'fullscreen' | 'control';

const LabControl: React.FC<LabControlProps> = () => {
  const [selectedLab, setSelectedLab] = useState('Phòng thực hành 01');
  const [viewMode, setViewMode] = useState<'grid' | 'list'>('grid');
  
  // LAN Discovery state
  const [isScanning, setIsScanning] = useState(false);
  const [discoveredDevices, setDiscoveredDevices] = useState<DiscoveredDevice[]>([]);
  const [savedDevices, setSavedDevices] = useState<SavedDevice[]>([]);
  const [scanError, setScanError] = useState<string | null>(null);
  const [lastScanTime, setLastScanTime] = useState<Date | null>(null);

  // Connection state
  const [connections, setConnections] = useState<StudentConnection[]>([]);
  const [screenFrames, setScreenFrames] = useState<Record<string, ScreenFrame>>({});
  const [isConnectingAll, setIsConnectingAll] = useState(false);

  // Fullscreen/Control modal state
  const [activeViewMode, setActiveViewMode] = useState<ViewMode>('grid');
  const [selectedComputer, setSelectedComputer] = useState<RoomComputer | null>(null);
  const [remoteControlEnabled, setRemoteControlEnabled] = useState(false);

  // FileManager state
  const [fileManagerStudent, setFileManagerStudent] = useState<StudentConnection | null>(null);

  // Manual IP input state
  const [showAddManualModal, setShowAddManualModal] = useState(false);
  const [manualIp, setManualIp] = useState('');
  const [manualPort, setManualPort] = useState('3017');
  const [manualName, setManualName] = useState('');
  const [isAddingManual, setIsAddingManual] = useState(false);

  // System commands state
  const [systemMenuStudent, setSystemMenuStudent] = useState<string | null>(null);
  const [showConfirmDialog, setShowConfirmDialog] = useState<{
    type: 'shutdown' | 'restart' | 'logout';
    studentId: string;
    studentName: string;
  } | null>(null);

  // Remote control refs
  const screenContainerRef = useRef<HTMLDivElement>(null);
  const keyboardInputRef = useRef<HTMLInputElement>(null);
  const lastMouseMoveTimeRef = useRef<number>(0);
  const pendingMouseMoveRef = useRef<{ x: number; y: number } | null>(null);
  const mouseMoveTimeoutRef = useRef<number | null>(null);
  const MOUSE_MOVE_THROTTLE_MS = 16;

  // Load saved devices on mount
  useEffect(() => {
    loadSavedDevices();
    
    // Start teacher discovery service to respond to student auto-connect
    invoke('start_teacher_discovery', { teacherName: 'Teacher' })
      .then(() => console.log('[LabControl] Teacher discovery service started'))
      .catch((e) => console.error('[LabControl] Failed to start teacher discovery:', e));
  }, []);

  // Poll connections every second
  useEffect(() => {
    const interval = setInterval(refreshConnections, 1000);
    return () => clearInterval(interval);
  }, []);

  // Listen for screen frames
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    const setupListener = async () => {
      unlisten = await listen<[string, ScreenFrame]>('screen-frame', (event) => {
        const [connId, frame] = event.payload;
        setScreenFrames(prev => ({ ...prev, [connId]: frame }));
      });
    };
    setupListener();
    return () => { if (unlisten) unlisten(); };
  }, []);

  // Focus keyboard input when remote control is active
  useEffect(() => {
    if (remoteControlEnabled && keyboardInputRef.current) {
      keyboardInputRef.current.focus();
    }
  }, [remoteControlEnabled]);

  // Cleanup timeout on unmount
  useEffect(() => {
    return () => {
      if (mouseMoveTimeoutRef.current !== null) {
        clearTimeout(mouseMoveTimeoutRef.current);
      }
    };
  }, []);

  const loadSavedDevices = async () => {
    try {
      const devices = await invoke<SavedDevice[]>('get_saved_devices');
      setSavedDevices(devices);
    } catch (error) {
      console.error('Failed to load saved devices:', error);
    }
  };

  const refreshConnections = async () => {
    try {
      const conns = await invoke<StudentConnection[]>('get_student_connections');
      setConnections(conns);
    } catch (e) { /* ignore */ }
  };

  const scanLan = useCallback(async () => {
    setIsScanning(true);
    setScanError(null);
    try {
      const port = 3017;
      const timeout = 3000;
      const devices = await invoke<DiscoveredDevice[]>('discover_lan_devices', { port, timeoutMs: timeout });
      setDiscoveredDevices(devices);
      setLastScanTime(new Date());
      
      if (devices.length === 0) {
        setScanError('Không tìm thấy thiết bị nào.');
      } else {
        let savedCount = 0;
        for (const device of devices) {
          const alreadySaved = savedDevices.some(d => d.ip === device.ip);
          if (!alreadySaved) {
            try {
              await invoke('save_device_to_db', { ip: device.ip, name: device.name, port: device.port });
              savedCount++;
            } catch (err) { console.error(`Failed to save device ${device.ip}:`, err); }
          }
        }
        await loadSavedDevices();
      }
    } catch (error) {
      setScanError(`Lỗi quét mạng: ${error}`);
    } finally {
      setIsScanning(false);
    }
  }, [savedDevices]);

  const connectToStudent = async (ip: string, port: number) => {
    try { await invoke('connect_to_student', { ip, port }); } 
    catch (e) { console.error(`Failed to connect to ${ip}:${port}:`, e); }
  };

  const disconnectStudent = async (id: string) => {
    try { await invoke('disconnect_from_student', { connectionId: id }); } 
    catch (e) { console.error('Failed to disconnect:', e); }
  };

  const connectAll = async () => {
    setIsConnectingAll(true);
    for (const device of discoveredDevices) {
      const existingConn = connections.find(c => c.ip === device.ip);
      if (!existingConn || existingConn.status === 'Disconnected') {
        try { await invoke('connect_to_student', { ip: device.ip, port: device.port }); } 
        catch (e) { console.warn(`Failed to connect to ${device.ip}:`, e); }
      }
    }
    setIsConnectingAll(false);
  };

  const disconnectAll = async () => {
    for (const conn of connections) {
      if (conn.status !== 'Disconnected') {
        try { await invoke('disconnect_from_student', { connectionId: conn.id }); } 
        catch (e) { console.warn(`Failed to disconnect ${conn.id}:`, e); }
      }
    }
  };

  const removeDevice = async (id: number) => {
    try { await invoke('remove_device_from_db', { id }); await loadSavedDevices(); } 
    catch (error) { console.error('Failed to remove device:', error); }
  };

  const getConnection = (ip: string) => connections.find(c => c.ip === ip);

  // Add device manually by IP
  const addManualDevice = async () => {
    if (!manualIp.trim()) return;
    setIsAddingManual(true);
    try {
      const port = parseInt(manualPort) || 3017;
      const name = manualName.trim() || `Máy ${manualIp}`;
      
      // Save to database
      await invoke('save_device_to_db', { ip: manualIp.trim(), name, port });
      await loadSavedDevices();
      
      // Try to connect
      await connectToStudent(manualIp.trim(), port);
      
      // Reset and close modal
      setManualIp('');
      setManualPort('3017');
      setManualName('');
      setShowAddManualModal(false);
    } catch (error) {
      console.error('Failed to add manual device:', error);
      alert(`Lỗi: ${error}`);
    } finally {
      setIsAddingManual(false);
    }
  };

  // System command functions
  const sendShutdown = async (studentId: string, delaySeconds?: number) => {
    try {
      await invoke('send_shutdown_command', { studentId, delaySeconds });
      setShowConfirmDialog(null);
      setSystemMenuStudent(null);
    } catch (error) {
      console.error('Failed to send shutdown command:', error);
      alert(`Lỗi: ${error}`);
    }
  };

  const sendRestart = async (studentId: string, delaySeconds?: number) => {
    try {
      await invoke('send_restart_command', { studentId, delaySeconds });
      setShowConfirmDialog(null);
      setSystemMenuStudent(null);
    } catch (error) {
      console.error('Failed to send restart command:', error);
      alert(`Lỗi: ${error}`);
    }
  };

  const sendLockScreen = async (studentId: string) => {
    try {
      await invoke('send_lock_screen_command', { studentId });
      setSystemMenuStudent(null);
    } catch (error) {
      console.error('Failed to send lock screen command:', error);
      alert(`Lỗi: ${error}`);
    }
  };

  const sendLogout = async (studentId: string) => {
    try {
      await invoke('send_logout_command', { studentId });
      setShowConfirmDialog(null);
      setSystemMenuStudent(null);
    } catch (error) {
      console.error('Failed to send logout command:', error);
      alert(`Lỗi: ${error}`);
    }
  };

  const getStatusText = (status: ConnectionStatus): string => {
    if (typeof status === 'string') {
      switch (status) {
        case 'Disconnected': return 'Offline';
        case 'Connecting': return 'Đang kết nối...';
        case 'Authenticating': return 'Xác thực...';
        case 'Connected': return 'Đã kết nối';
        case 'Viewing': return 'Đang xem';
        default: return status;
      }
    }
    if ('Error' in status) return 'Lỗi';
    return 'Unknown';
  };

  // Open fullscreen view
  const openFullscreen = (pc: RoomComputer) => {
    setSelectedComputer(pc);
    setActiveViewMode('fullscreen');
    setRemoteControlEnabled(false);
  };

  // Open control modal
  const openControlModal = (pc: RoomComputer) => {
    setSelectedComputer(pc);
    setActiveViewMode('control');
    setRemoteControlEnabled(true);
  };

  // Open FileManager for a student
  const openFileManager = (conn: StudentConnection) => {
    setFileManagerStudent(conn);
  };

  // Close FileManager
  const closeFileManager = () => {
    setFileManagerStudent(null);
  };

  // Close fullscreen/modal
  const closeView = () => {
    setActiveViewMode('grid');
    setSelectedComputer(null);
    setRemoteControlEnabled(false);
  };

  // Get selected connection
  const getSelectedConnection = () => {
    if (!selectedComputer) return null;
    return connections.find(c => c.ip === selectedComputer.ipAddress);
  };

  // Remote control functions
  const getNormalizedPosition = useCallback((e: React.MouseEvent): { x: number; y: number } | null => {
    const container = screenContainerRef.current;
    const conn = getSelectedConnection();
    const frame = conn ? screenFrames[conn.id] : null;
    if (!container || !frame) return null;

    const rect = container.getBoundingClientRect();
    const containerAspect = rect.width / rect.height;
    const imageAspect = frame.width / frame.height;

    let imageWidth: number, imageHeight: number, offsetX: number, offsetY: number;
    if (containerAspect > imageAspect) {
      imageHeight = rect.height;
      imageWidth = imageHeight * imageAspect;
      offsetX = (rect.width - imageWidth) / 2;
      offsetY = 0;
    } else {
      imageWidth = rect.width;
      imageHeight = imageWidth / imageAspect;
      offsetX = 0;
      offsetY = (rect.height - imageHeight) / 2;
    }

    const localX = e.clientX - rect.left - offsetX;
    const localY = e.clientY - rect.top - offsetY;
    if (localX < 0 || localX > imageWidth || localY < 0 || localY > imageHeight) return null;

    return { x: localX / imageWidth, y: localY / imageHeight };
  }, [screenFrames]);

  const sendMouseEvent = useCallback(async (event: MouseInputEvent) => {
    if (!remoteControlEnabled) return;
    const conn = getSelectedConnection();
    if (!conn) return;
    try { await invoke('send_remote_mouse_event', { studentId: conn.id, event }); } 
    catch (err) { console.error('Failed to send mouse event:', err); }
  }, [remoteControlEnabled, connections, selectedComputer]);

  const sendKeyboardEvent = useCallback(async (event: KeyboardInputEvent) => {
    if (!remoteControlEnabled) return;
    const conn = getSelectedConnection();
    if (!conn) return;
    try { await invoke('send_remote_keyboard_event', { studentId: conn.id, event }); } 
    catch (err) { console.error('Failed to send keyboard event:', err); }
  }, [remoteControlEnabled, connections, selectedComputer]);

  const handleMouseMove = useCallback((e: React.MouseEvent) => {
    const pos = getNormalizedPosition(e);
    if (!pos) return;
    const now = Date.now();
    const timeSinceLastMove = now - lastMouseMoveTimeRef.current;
    pendingMouseMoveRef.current = pos;

    if (timeSinceLastMove >= MOUSE_MOVE_THROTTLE_MS) {
      lastMouseMoveTimeRef.current = now;
      pendingMouseMoveRef.current = null;
      if (mouseMoveTimeoutRef.current !== null) { clearTimeout(mouseMoveTimeoutRef.current); mouseMoveTimeoutRef.current = null; }
      sendMouseEvent({ event_type: 'move', x: pos.x, y: pos.y });
    } else if (mouseMoveTimeoutRef.current === null) {
      const delay = MOUSE_MOVE_THROTTLE_MS - timeSinceLastMove;
      mouseMoveTimeoutRef.current = window.setTimeout(() => {
        const pending = pendingMouseMoveRef.current;
        if (pending) {
          lastMouseMoveTimeRef.current = Date.now();
          pendingMouseMoveRef.current = null;
          mouseMoveTimeoutRef.current = null;
          sendMouseEvent({ event_type: 'move', x: pending.x, y: pending.y });
        }
      }, delay);
    }
  }, [getNormalizedPosition, sendMouseEvent]);

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    const pos = getNormalizedPosition(e);
    if (pos) {
      if (mouseMoveTimeoutRef.current !== null) { clearTimeout(mouseMoveTimeoutRef.current); mouseMoveTimeoutRef.current = null; }
      pendingMouseMoveRef.current = null;
      const button = e.button === 0 ? 'left' : e.button === 2 ? 'right' : 'middle';
      sendMouseEvent({ event_type: 'down', x: pos.x, y: pos.y, button });
    }
    keyboardInputRef.current?.focus();
  }, [getNormalizedPosition, sendMouseEvent]);

  const handleMouseUp = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    const pos = getNormalizedPosition(e);
    if (pos) {
      if (mouseMoveTimeoutRef.current !== null) { clearTimeout(mouseMoveTimeoutRef.current); mouseMoveTimeoutRef.current = null; }
      pendingMouseMoveRef.current = null;
      const button = e.button === 0 ? 'left' : e.button === 2 ? 'right' : 'middle';
      sendMouseEvent({ event_type: 'up', x: pos.x, y: pos.y, button });
    }
  }, [getNormalizedPosition, sendMouseEvent]);

  const handleContextMenu = useCallback((e: React.MouseEvent) => { if (remoteControlEnabled) e.preventDefault(); }, [remoteControlEnabled]);

  const handleWheel = useCallback((e: React.WheelEvent) => {
    if (!remoteControlEnabled) return;
    e.preventDefault();
    const pos = getNormalizedPosition(e);
    if (pos) sendMouseEvent({ event_type: 'scroll', x: pos.x, y: pos.y, delta_x: e.deltaX, delta_y: e.deltaY });
  }, [remoteControlEnabled, getNormalizedPosition, sendMouseEvent]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (!remoteControlEnabled) return;
    e.preventDefault();
    sendKeyboardEvent({ event_type: 'keydown', key: e.key, code: e.code, modifiers: { ctrl: e.ctrlKey, alt: e.altKey, shift: e.shiftKey, meta: e.metaKey } });
  }, [remoteControlEnabled, sendKeyboardEvent]);

  const handleKeyUp = useCallback((e: React.KeyboardEvent) => {
    if (!remoteControlEnabled) return;
    e.preventDefault();
    sendKeyboardEvent({ event_type: 'keyup', key: e.key, code: e.code, modifiers: { ctrl: e.ctrlKey, alt: e.altKey, shift: e.shiftKey, meta: e.metaKey } });
  }, [remoteControlEnabled, sendKeyboardEvent]);

  // Build computer list
  const computers: (RoomComputer & { connection?: StudentConnection })[] = savedDevices.map((device, index) => {
    const conn = getConnection(device.ip);
    const isOnline = discoveredDevices.some(d => d.ip === device.ip);
    return {
      roomComputerId: index + 1,
      computerName: device.name,
      ipAddress: device.ip,
      status: conn?.status === 'Connected' || conn?.status === 'Viewing' ? 'Active' as const : isOnline ? 'Repairing' as const : 'Broken' as const,
      connection: conn
    };
  });

  const connectedCount = connections.filter(c => c.status === 'Connected' || c.status === 'Viewing').length;

  // Fullscreen View Component
  const renderFullscreenView = () => {
    if (!selectedComputer) return null;
    const conn = getSelectedConnection();
    const frame = conn ? screenFrames[conn.id] : null;
    const isViewing = conn?.status === 'Viewing';

    return (
      <div className="fixed inset-0 z-[9999] bg-black flex flex-col overflow-hidden" style={{ position: 'fixed', top: 0, left: 0, right: 0, bottom: 0 }}>
        {/* Header - fixed height */}
        <div className="flex-shrink-0 flex items-center justify-between px-6 py-4 bg-slate-900 border-b border-slate-800">
          <div className="flex items-center gap-4">
            <h2 className="text-xl font-black text-white">{selectedComputer.computerName}</h2>
            <span className="text-sm text-slate-400 font-mono">{selectedComputer.ipAddress}</span>
            {frame && <span className="text-xs text-slate-500">{frame.width}x{frame.height}</span>}
          </div>
          <div className="flex items-center gap-3">
            <button onClick={() => { setActiveViewMode('control'); setRemoteControlEnabled(true); }}
              className="flex items-center gap-2 px-4 py-2 bg-indigo-600 hover:bg-indigo-500 rounded-xl text-white text-sm font-bold transition-colors">
              <MousePointer className="w-4 h-4" /> Điều khiển
            </button>
            <button onClick={closeView} className="p-2 bg-slate-800 hover:bg-rose-600 rounded-xl text-white transition-colors">
              <X className="w-6 h-6" />
            </button>
          </div>
        </div>

        {/* Screen - scrollable if needed */}
        <div className="flex-1 min-h-0 overflow-auto bg-black">
          <div className="min-h-full flex items-center justify-center p-2">
            {isViewing && frame ? (
              <div 
                style={{ 
                  width: '100%',
                  height: '100%',
                  maxWidth: frame.width > 0 ? `${frame.width}px` : '100%',
                  aspectRatio: frame.width && frame.height ? `${frame.width}/${frame.height}` : 'auto'
                }}
              >
                <H264VideoPlayer frame={frame} className="w-full h-full object-contain" connectionId={conn!.id} />
              </div>
            ) : (
              <div className="flex flex-col items-center justify-center text-white">
                <Loader2 className="w-12 h-12 animate-spin mb-4 text-indigo-400" />
                <span className="text-slate-400">Đang tải màn hình...</span>
              </div>
            )}
          </div>
        </div>
      </div>
    );
  };

  // Control Modal Component
  const renderControlModal = () => {
    if (!selectedComputer) return null;
    const conn = getSelectedConnection();
    const frame = conn ? screenFrames[conn.id] : null;
    const isViewing = conn?.status === 'Viewing';

    return (
      <div className="fixed inset-0 z-[9999] bg-black flex flex-col overflow-hidden" style={{ position: 'fixed', top: 0, left: 0, right: 0, bottom: 0 }}>
        {/* Hidden keyboard input */}
        <input ref={keyboardInputRef} type="text" className="absolute opacity-0 pointer-events-none" 
          onKeyDown={handleKeyDown} onKeyUp={handleKeyUp} tabIndex={-1} />

        {/* Header - fixed height */}
        <div className="flex-shrink-0 flex items-center justify-between px-6 py-3 bg-slate-900 border-b border-slate-800">
          <div className="flex items-center gap-4">
            <h2 className="text-lg font-black text-white">{selectedComputer.computerName}</h2>
            <span className="text-sm text-slate-400 font-mono">{selectedComputer.ipAddress}</span>
            {frame && <span className="text-xs text-slate-500">{frame.width}x{frame.height}</span>}
            <div className="flex items-center gap-2 px-3 py-1 bg-rose-600 rounded-lg">
              <div className="w-2 h-2 bg-white rounded-full animate-pulse" />
              <span className="text-xs font-black text-white uppercase">Đang điều khiển</span>
            </div>
          </div>
          <div className="flex items-center gap-3">
            <button onClick={() => { setActiveViewMode('fullscreen'); setRemoteControlEnabled(false); }}
              className="flex items-center gap-2 px-4 py-2 bg-slate-700 hover:bg-slate-600 rounded-xl text-white text-sm font-bold transition-colors">
              <Minimize2 className="w-4 h-4" /> Chỉ xem
            </button>
            <button onClick={closeView} className="p-2 bg-slate-800 hover:bg-rose-600 rounded-xl text-white transition-colors">
              <X className="w-6 h-6" />
            </button>
          </div>
        </div>

        {/* Screen with remote control - scrollable if needed */}
        <div className="flex-1 min-h-0 overflow-auto bg-black">
          <div className="min-h-full flex items-center justify-center p-2">
            {isViewing && frame ? (
              <div 
                ref={screenContainerRef} 
                className="cursor-crosshair"
                style={{ 
                  width: '100%',
                  height: '100%',
                  maxWidth: frame.width > 0 ? `${frame.width}px` : '100%',
                  aspectRatio: frame.width && frame.height ? `${frame.width}/${frame.height}` : 'auto'
                }}
                onMouseMove={handleMouseMove} 
                onMouseDown={handleMouseDown} 
                onMouseUp={handleMouseUp}
                onContextMenu={handleContextMenu} 
                onWheel={handleWheel}
              >
                <H264VideoPlayer frame={frame} className="w-full h-full object-contain pointer-events-none" connectionId={conn!.id} />
              </div>
            ) : (
              <div className="flex flex-col items-center justify-center text-white">
                <Loader2 className="w-12 h-12 animate-spin mb-4 text-indigo-400" />
                <span className="text-slate-400">Đang tải màn hình...</span>
              </div>
            )}
          </div>
        </div>

        {/* Footer hint - fixed height */}
        <div className="flex-shrink-0 px-6 py-2 bg-slate-900 border-t border-slate-800 text-center">
          <span className="text-xs text-slate-500">Click vào màn hình để điều khiển chuột • Gõ phím để điều khiển bàn phím • Cuộn để xem toàn bộ</span>
        </div>
      </div>
    );
  };

  // Render fullscreen or control modal if active
  if (activeViewMode === 'fullscreen') return renderFullscreenView();
  if (activeViewMode === 'control') return renderControlModal();

  // Main grid view
  return (
    <div className="space-y-8">
      {/* Header */}
      <div className="flex flex-col md:flex-row md:items-center justify-between gap-6">
        <div className="flex items-center gap-6">
          <div className="bg-white p-2 rounded-2xl shadow-sm border border-slate-200">
            <select value={selectedLab} onChange={(e) => setSelectedLab(e.target.value)}
              className="text-xl font-black text-slate-800 bg-transparent border-none outline-none cursor-pointer px-4">
              <option>Phòng thực hành 01 (Dãy A)</option>
              <option>Phòng thực hành 02 (Dãy A)</option>
              <option>Phòng thực hành 03 (Dãy B)</option>
            </select>
          </div>
          <div className="flex items-center gap-2 px-4 py-2 bg-emerald-50 text-emerald-700 rounded-xl text-xs font-black uppercase tracking-tighter border border-emerald-100">
            <CheckCircle2 className="w-4 h-4" /> 
            {connectedCount > 0 ? `${connectedCount} máy đang xem` : discoveredDevices.length > 0 ? `${discoveredDevices.length} máy online` : 'Chưa quét mạng'}
          </div>
        </div>

        <div className="flex items-center gap-3">
          <button onClick={scanLan} disabled={isScanning}
            className={`flex items-center gap-2 px-5 py-3 rounded-2xl font-black text-xs uppercase tracking-widest transition-all ${isScanning ? 'bg-slate-100 text-slate-400 cursor-not-allowed' : 'bg-indigo-600 text-white hover:bg-indigo-700 shadow-lg shadow-indigo-600/30'}`}>
            {isScanning ? <Loader2 className="w-4 h-4 animate-spin" /> : <Wifi className="w-4 h-4" />}
            {isScanning ? 'Đang quét...' : 'Quét LAN'}
          </button>
          <button onClick={() => setShowAddManualModal(true)}
            className="flex items-center gap-2 px-5 py-3 rounded-2xl font-black text-xs uppercase tracking-widest bg-slate-600 text-white hover:bg-slate-700 transition-all">
            <Plus className="w-4 h-4" /> Thêm IP
          </button>
          <button onClick={connectAll} disabled={isConnectingAll || discoveredDevices.length === 0}
            className="flex items-center gap-2 px-5 py-3 rounded-2xl font-black text-xs uppercase tracking-widest bg-emerald-600 text-white hover:bg-emerald-700 transition-all disabled:opacity-50 disabled:cursor-not-allowed">
            <Link className="w-4 h-4" /> Kết nối tất cả
          </button>
          <button onClick={disconnectAll} disabled={connectedCount === 0}
            className="flex items-center gap-2 px-5 py-3 rounded-2xl font-black text-xs uppercase tracking-widest bg-rose-600 text-white hover:bg-rose-700 transition-all disabled:opacity-50 disabled:cursor-not-allowed">
            <Unlink className="w-4 h-4" /> Ngắt tất cả
          </button>
          <div className="flex items-center gap-1 bg-white p-1 rounded-xl border border-slate-200">
            <button onClick={() => setViewMode('grid')} className={`p-2 rounded-lg transition-all ${viewMode === 'grid' ? 'bg-indigo-600 text-white' : 'text-slate-400 hover:text-slate-600'}`}><Grid className="w-4 h-4" /></button>
            <button onClick={() => setViewMode('list')} className={`p-2 rounded-lg transition-all ${viewMode === 'list' ? 'bg-indigo-600 text-white' : 'text-slate-400 hover:text-slate-600'}`}><List className="w-4 h-4" /></button>
          </div>
        </div>
      </div>

      {/* Scan Status */}
      {scanError && (
        <div className="flex items-center gap-3 p-4 bg-amber-50 border border-amber-200 rounded-2xl text-amber-700">
          <WifiOff className="w-5 h-5 flex-shrink-0" />
          <span className="text-sm font-medium">{scanError}</span>
        </div>
      )}

      {lastScanTime && !scanError && (
        <div className="flex items-center gap-3 text-sm text-slate-500">
          <RefreshCw className="w-4 h-4" />
          <span>Quét lần cuối: {lastScanTime.toLocaleTimeString('vi-VN')}</span>
          <span className="text-slate-300">|</span>
          <span className="text-emerald-600 font-bold">{discoveredDevices.length} online</span>
          <span className="text-slate-300">|</span>
          <span className="text-indigo-600 font-bold">{connectedCount} đang xem</span>
        </div>
      )}

      {/* Computer Grid */}
      {computers.length === 0 ? (
        <div className="flex flex-col items-center justify-center py-20 bg-white rounded-3xl border-2 border-dashed border-slate-200">
          <Wifi className="w-16 h-16 text-slate-300 mb-4" />
          <h3 className="text-xl font-black text-slate-400 mb-2">Chưa có thiết bị</h3>
          <p className="text-sm text-slate-400 mb-6">Nhấn "Quét LAN" để tìm các máy học sinh</p>
          <button onClick={scanLan} disabled={isScanning}
            className="flex items-center gap-2 px-6 py-3 bg-indigo-600 text-white rounded-2xl font-black text-sm uppercase tracking-widest hover:bg-indigo-700 transition-all">
            <Wifi className="w-5 h-5" /> Bắt đầu quét
          </button>
        </div>
      ) : (
        <div className={`grid gap-6 ${viewMode === 'grid' ? 'grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4' : 'grid-cols-1'}`}>
          {computers.map((pc) => {
            const conn = pc.connection;
            const isConnected = conn?.status === 'Connected' || conn?.status === 'Viewing';
            const isViewing = conn?.status === 'Viewing';
            const isOnline = discoveredDevices.some(d => d.ip === pc.ipAddress);
            const frame = conn ? screenFrames[conn.id] : undefined;
            const savedDevice = savedDevices.find(d => d.ip === pc.ipAddress);
            
            return (
              <div key={pc.roomComputerId} className={`relative group rounded-3xl border-2 transition-all duration-300 overflow-hidden ${
                isViewing ? 'bg-slate-900 border-indigo-500 shadow-2xl shadow-indigo-500/20' 
                : isConnected ? 'bg-white border-emerald-300 shadow-lg'
                : isOnline ? 'bg-white border-amber-200 hover:border-indigo-300 hover:shadow-xl'
                : 'bg-slate-50 border-slate-200'}`}>
                
                {/* Screen Preview */}
                <div className="relative aspect-video bg-slate-900 overflow-hidden cursor-pointer" onClick={() => isViewing && openFullscreen(pc)}>
                  {isViewing && frame ? (
                    <H264VideoPlayer frame={frame} className="w-full h-full object-contain" connectionId={conn!.id} />
                  ) : isViewing ? (
                    <div className="absolute inset-0 flex flex-col items-center justify-center text-white">
                      <Loader2 className="w-8 h-8 animate-spin mb-2 text-indigo-400" />
                      <span className="text-xs font-medium text-slate-400">Đang tải...</span>
                    </div>
                  ) : isConnected ? (
                    <div className="absolute inset-0 flex flex-col items-center justify-center text-white">
                      <CheckCircle2 className="w-10 h-10 mb-2 text-emerald-400" />
                      <span className="text-xs font-medium text-slate-400">Đã kết nối</span>
                    </div>
                  ) : isOnline ? (
                    <div className="absolute inset-0 flex flex-col items-center justify-center">
                      <div className="w-12 h-12 rounded-full bg-amber-500/20 flex items-center justify-center mb-2">
                        <Monitor className="w-6 h-6 text-amber-400" />
                      </div>
                      <span className="text-xs font-medium text-amber-400">Online</span>
                    </div>
                  ) : (
                    <div className="absolute inset-0 flex flex-col items-center justify-center">
                      <div className="w-12 h-12 rounded-full bg-slate-700 flex items-center justify-center mb-2">
                        <WifiOff className="w-6 h-6 text-slate-500" />
                      </div>
                      <span className="text-xs font-medium text-slate-500">Offline</span>
                    </div>
                  )}

                  {/* Status Badge */}
                  <div className={`absolute top-3 left-3 px-2 py-1 rounded-lg text-[10px] font-black uppercase tracking-wider ${
                    isViewing ? 'bg-indigo-500 text-white' : isConnected ? 'bg-emerald-500 text-white' : isOnline ? 'bg-amber-500 text-white' : 'bg-slate-600 text-slate-300'}`}>
                    {conn ? getStatusText(conn.status) : isOnline ? 'Online' : 'Offline'}
                  </div>

                  {/* Fullscreen Button */}
                  {isViewing && (
                    <button onClick={(e) => { e.stopPropagation(); openFullscreen(pc); }}
                      className="absolute top-3 right-3 p-2 bg-black/50 hover:bg-black/70 rounded-lg text-white transition-all">
                      <Maximize2 className="w-4 h-4" />
                    </button>
                  )}
                </div>

                {/* Info Bar */}
                <div className={`p-4 ${isViewing ? 'bg-slate-800' : 'bg-white'}`}>
                  <div className="flex items-center justify-between mb-3">
                    <div>
                      <h4 className={`text-lg font-black ${isViewing ? 'text-white' : 'text-slate-800'}`}>{pc.computerName}</h4>
                      <p className={`text-xs font-mono ${isViewing ? 'text-slate-400' : 'text-slate-500'}`}>{pc.ipAddress}:3017</p>
                    </div>
                    <div className={`w-3 h-3 rounded-full ${isViewing ? 'bg-indigo-500 animate-pulse' : isConnected ? 'bg-emerald-500' : isOnline ? 'bg-amber-500' : 'bg-slate-400'}`} />
                  </div>

                  {/* Action Buttons */}
                  <div className="flex gap-2">
                    {!isConnected && isOnline && (
                      <button onClick={() => connectToStudent(pc.ipAddress, 3017)}
                        className="flex-1 flex items-center justify-center gap-2 py-2 bg-indigo-600 text-white text-[10px] font-black rounded-xl uppercase tracking-widest hover:bg-indigo-500 transition-colors">
                        <Link className="w-3 h-3" /> Kết nối
                      </button>
                    )}
                    {isConnected && !isViewing && (
                      <button onClick={() => openFullscreen(pc)}
                        className="flex-1 flex items-center justify-center gap-2 py-2 bg-indigo-600 text-white text-[10px] font-black rounded-xl uppercase tracking-widest hover:bg-indigo-500 transition-colors">
                        <Eye className="w-3 h-3" /> Xem
                      </button>
                    )}
                    {isViewing && conn && (
                      <>
                        <button onClick={() => openControlModal(pc)}
                          className="flex-1 flex items-center justify-center gap-2 py-2 bg-slate-700 text-white text-[10px] font-black rounded-xl uppercase tracking-widest hover:bg-slate-600 transition-colors">
                          <MousePointer className="w-3 h-3" /> Điều khiển
                        </button>
                        <button onClick={() => openFileManager(conn)}
                          className="flex-1 flex items-center justify-center gap-2 py-2 bg-emerald-600 text-white text-[10px] font-black rounded-xl uppercase tracking-widest hover:bg-emerald-500 transition-colors">
                          <FolderOpen className="w-3 h-3" /> File
                        </button>
                      </>
                    )}
                    {isConnected && conn && (
                      <>
                        {/* System Commands Menu */}
                        <div className="relative">
                          <button 
                            onClick={() => setSystemMenuStudent(systemMenuStudent === conn.id ? null : conn.id)}
                            className="p-2 bg-amber-600 text-white rounded-xl hover:bg-amber-500 transition-colors" 
                            title="Điều khiển hệ thống"
                          >
                            <MoreVertical className="w-3 h-3" />
                          </button>
                          
                          {/* Dropdown Menu */}
                          {systemMenuStudent === conn.id && (
                            <div className="absolute right-0 bottom-full mb-2 w-48 bg-white rounded-xl shadow-2xl border border-slate-200 overflow-hidden z-50">
                              <button
                                onClick={() => sendLockScreen(conn.id)}
                                className="w-full flex items-center gap-3 px-4 py-3 text-left text-sm font-medium text-slate-700 hover:bg-slate-50 transition-colors"
                              >
                                <Lock className="w-4 h-4 text-amber-500" />
                                Khóa màn hình
                              </button>
                              <button
                                onClick={() => setShowConfirmDialog({ type: 'logout', studentId: conn.id, studentName: pc.computerName })}
                                className="w-full flex items-center gap-3 px-4 py-3 text-left text-sm font-medium text-slate-700 hover:bg-slate-50 transition-colors"
                              >
                                <LogOut className="w-4 h-4 text-orange-500" />
                                Đăng xuất
                              </button>
                              <button
                                onClick={() => setShowConfirmDialog({ type: 'restart', studentId: conn.id, studentName: pc.computerName })}
                                className="w-full flex items-center gap-3 px-4 py-3 text-left text-sm font-medium text-slate-700 hover:bg-slate-50 transition-colors"
                              >
                                <RotateCcw className="w-4 h-4 text-blue-500" />
                                Khởi động lại
                              </button>
                              <button
                                onClick={() => setShowConfirmDialog({ type: 'shutdown', studentId: conn.id, studentName: pc.computerName })}
                                className="w-full flex items-center gap-3 px-4 py-3 text-left text-sm font-medium text-rose-600 hover:bg-rose-50 transition-colors"
                              >
                                <Power className="w-4 h-4" />
                                Tắt máy
                              </button>
                            </div>
                          )}
                        </div>
                        
                        <button onClick={() => disconnectStudent(conn.id)}
                          className="p-2 bg-rose-600 text-white rounded-xl hover:bg-rose-500 transition-colors" title="Ngắt kết nối">
                          <Unlink className="w-3 h-3" />
                        </button>
                      </>
                    )}
                    {savedDevice?.id && !isConnected && (
                      <button onClick={() => removeDevice(savedDevice.id!)}
                        className="p-2 bg-slate-200 text-slate-600 rounded-xl hover:bg-rose-100 hover:text-rose-600 transition-colors" title="Xóa">
                        <Trash2 className="w-3 h-3" />
                      </button>
                    )}
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      )}

      {/* FileManager Modal */}
      {fileManagerStudent && (
        <FileManager
          student={fileManagerStudent}
          onClose={closeFileManager}
        />
      )}

      {/* Manual IP Input Modal */}
      {showAddManualModal && (
        <div className="fixed inset-0 z-50 bg-black/50 flex items-center justify-center p-4">
          <div className="bg-white rounded-3xl shadow-2xl w-full max-w-md overflow-hidden">
            {/* Header */}
            <div className="flex items-center justify-between px-6 py-4 bg-slate-50 border-b border-slate-200">
              <h3 className="text-lg font-black text-slate-800">Thêm thiết bị thủ công</h3>
              <button onClick={() => setShowAddManualModal(false)} className="p-2 hover:bg-slate-200 rounded-xl transition-colors">
                <X className="w-5 h-5 text-slate-500" />
              </button>
            </div>

            {/* Form */}
            <div className="p-6 space-y-4">
              <div>
                <label className="block text-sm font-bold text-slate-700 mb-2">Địa chỉ IP *</label>
                <input
                  type="text"
                  value={manualIp}
                  onChange={(e) => setManualIp(e.target.value)}
                  placeholder="192.168.1.100"
                  className="w-full px-4 py-3 border-2 border-slate-200 rounded-xl focus:border-indigo-500 focus:outline-none transition-colors font-mono"
                />
              </div>

              <div>
                <label className="block text-sm font-bold text-slate-700 mb-2">Port</label>
                <input
                  type="text"
                  value={manualPort}
                  onChange={(e) => setManualPort(e.target.value)}
                  placeholder="3017"
                  className="w-full px-4 py-3 border-2 border-slate-200 rounded-xl focus:border-indigo-500 focus:outline-none transition-colors font-mono"
                />
              </div>

              <div>
                <label className="block text-sm font-bold text-slate-700 mb-2">Tên máy (tùy chọn)</label>
                <input
                  type="text"
                  value={manualName}
                  onChange={(e) => setManualName(e.target.value)}
                  placeholder="Máy học sinh 01"
                  className="w-full px-4 py-3 border-2 border-slate-200 rounded-xl focus:border-indigo-500 focus:outline-none transition-colors"
                />
              </div>
            </div>

            {/* Footer */}
            <div className="flex gap-3 px-6 py-4 bg-slate-50 border-t border-slate-200">
              <button
                onClick={() => setShowAddManualModal(false)}
                className="flex-1 py-3 bg-slate-200 text-slate-700 rounded-xl font-bold hover:bg-slate-300 transition-colors"
              >
                Hủy
              </button>
              <button
                onClick={addManualDevice}
                disabled={!manualIp.trim() || isAddingManual}
                className="flex-1 flex items-center justify-center gap-2 py-3 bg-indigo-600 text-white rounded-xl font-bold hover:bg-indigo-700 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
              >
                {isAddingManual ? <Loader2 className="w-4 h-4 animate-spin" /> : <Plus className="w-4 h-4" />}
                {isAddingManual ? 'Đang thêm...' : 'Thêm & Kết nối'}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* System Command Confirm Dialog */}
      {showConfirmDialog && (
        <div className="fixed inset-0 z-50 bg-black/50 flex items-center justify-center p-4" onClick={() => setShowConfirmDialog(null)}>
          <div className="bg-white rounded-3xl shadow-2xl w-full max-w-md overflow-hidden" onClick={(e) => e.stopPropagation()}>
            {/* Header */}
            <div className={`flex items-center gap-4 px-6 py-4 ${
              showConfirmDialog.type === 'shutdown' ? 'bg-rose-50' : 
              showConfirmDialog.type === 'restart' ? 'bg-blue-50' : 'bg-orange-50'
            }`}>
              {showConfirmDialog.type === 'shutdown' && <Power className="w-8 h-8 text-rose-500" />}
              {showConfirmDialog.type === 'restart' && <RotateCcw className="w-8 h-8 text-blue-500" />}
              {showConfirmDialog.type === 'logout' && <LogOut className="w-8 h-8 text-orange-500" />}
              <div>
                <h3 className="text-lg font-black text-slate-800">
                  {showConfirmDialog.type === 'shutdown' && 'Xác nhận tắt máy'}
                  {showConfirmDialog.type === 'restart' && 'Xác nhận khởi động lại'}
                  {showConfirmDialog.type === 'logout' && 'Xác nhận đăng xuất'}
                </h3>
                <p className="text-sm text-slate-500">{showConfirmDialog.studentName}</p>
              </div>
            </div>

            {/* Content */}
            <div className="p-6">
              <p className="text-slate-600">
                {showConfirmDialog.type === 'shutdown' && 'Bạn có chắc chắn muốn tắt máy này? Tất cả công việc chưa lưu sẽ bị mất.'}
                {showConfirmDialog.type === 'restart' && 'Bạn có chắc chắn muốn khởi động lại máy này? Tất cả công việc chưa lưu sẽ bị mất.'}
                {showConfirmDialog.type === 'logout' && 'Bạn có chắc chắn muốn đăng xuất người dùng trên máy này?'}
              </p>
            </div>

            {/* Footer */}
            <div className="flex gap-3 px-6 py-4 bg-slate-50 border-t border-slate-200">
              <button
                onClick={() => setShowConfirmDialog(null)}
                className="flex-1 py-3 bg-slate-200 text-slate-700 rounded-xl font-bold hover:bg-slate-300 transition-colors"
              >
                Hủy
              </button>
              <button
                onClick={() => {
                  if (showConfirmDialog.type === 'shutdown') {
                    sendShutdown(showConfirmDialog.studentId);
                  } else if (showConfirmDialog.type === 'restart') {
                    sendRestart(showConfirmDialog.studentId);
                  } else if (showConfirmDialog.type === 'logout') {
                    sendLogout(showConfirmDialog.studentId);
                  }
                }}
                className={`flex-1 flex items-center justify-center gap-2 py-3 text-white rounded-xl font-bold transition-colors ${
                  showConfirmDialog.type === 'shutdown' ? 'bg-rose-600 hover:bg-rose-700' :
                  showConfirmDialog.type === 'restart' ? 'bg-blue-600 hover:bg-blue-700' :
                  'bg-orange-600 hover:bg-orange-700'
                }`}
              >
                {showConfirmDialog.type === 'shutdown' && <><Power className="w-4 h-4" /> Tắt máy</>}
                {showConfirmDialog.type === 'restart' && <><RotateCcw className="w-4 h-4" /> Khởi động lại</>}
                {showConfirmDialog.type === 'logout' && <><LogOut className="w-4 h-4" /> Đăng xuất</>}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Click outside to close system menu */}
      {systemMenuStudent && (
        <div 
          className="fixed inset-0 z-40" 
          onClick={() => setSystemMenuStudent(null)}
        />
      )}
    </div>
  );
};

export default LabControl;
