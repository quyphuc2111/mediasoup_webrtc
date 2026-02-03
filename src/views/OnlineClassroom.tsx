import React, { useState, useEffect, useRef, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { 
  MonitorPlay, MessageCircle, Hand, 
  Users, MicOff, VideoOff, Mic,
  ScreenShare, X, Send, Bot, Server, Loader2, AlertCircle, CheckCircle2,
  Wifi, WifiOff, RefreshCw
} from 'lucide-react';
import { UserAccount as UserType, Message, UserRole } from '../types';
import { useMediasoup } from '../hooks/useMediasoup';
import { VideoPlayer } from '../components/VideoPlayer';
import { SharingIndicator } from '../components/SharingIndicator';

interface ServerInfo {
  url: string;
  ip: string;
  port: number;
}

interface OnlineClassroomProps {
  user: UserType;
  onStartScreenShare?: () => void;
  onStopScreenShare?: () => void;
}

const OnlineClassroom: React.FC<OnlineClassroomProps> = ({ user, onStartScreenShare, onStopScreenShare }) => {
  const [messages, setMessages] = useState<Message[]>([
    { id: '1', senderId: 'system', senderName: 'Hệ thống', content: 'Chào mừng đến với lớp học trực tuyến!', timestamp: '14:00', role: UserRole.ADMIN }
  ]);
  const [inputMessage, setInputMessage] = useState('');
  const scrollRef = useRef<HTMLDivElement>(null);
  
  // Mediasoup Server state (for Teacher)
  const [serverStatus, setServerStatus] = useState<'stopped' | 'starting' | 'running' | 'error'>('stopped');
  const [serverInfo, setServerInfo] = useState<ServerInfo | null>(null);
  const [serverError, setServerError] = useState<string | null>(null);
  const serverStarted = useRef(false);
  
  // Student connection state
  const [teacherServerUrl, setTeacherServerUrl] = useState<string>('');
  const [studentConnectStatus, setStudentConnectStatus] = useState<'disconnected' | 'connecting' | 'connected' | 'error'>('disconnected');
  const [studentConnectError, setStudentConnectError] = useState<string | null>(null);
  const [connectRetryCount, setConnectRetryCount] = useState(0);
  const studentConnectStarted = useRef(false);
  const retryIntervalRef = useRef<ReturnType<typeof setInterval> | null>(null);
  
  // Mediasoup client hook
  const {
    connectionState,
    error: mediasoupError,
    peers,
    localStream,
    remoteStream,
    isSharing,
    isMicActive,
    isPushToTalkActive,
    connect,
    disconnect,
    startScreenShare,
    startMicrophone,
    stopMicrophone,
    stopScreenShare,
    enablePushToTalk,
    disablePushToTalk,
    sendChatMessage,
    onChatMessage,
  } = useMediasoup();

  const roomId = 'classroom-main';

  // Setup chat message listener
  useEffect(() => {
    onChatMessage((message) => {
      const newMsg: Message = {
        id: Date.now().toString(),
        senderId: message.senderId,
        senderName: message.senderName,
        content: message.content,
        timestamp: new Date(message.timestamp).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }),
        role: message.isTeacher ? UserRole.TEACHER : UserRole.STUDENT
      };
      setMessages(prev => [...prev, newMsg]);
    });
  }, [onChatMessage]);

  // Auto-start Mediasoup Server for Teacher role
  useEffect(() => {
    if (user.role !== UserRole.TEACHER || serverStarted.current) {
      return;
    }

    const initServer = async () => {
      // Check if server is already running
      try {
        const info = await invoke<ServerInfo>('get_server_info');
        setServerInfo(info);
        setServerStatus('running');
        serverStarted.current = true;
        console.log('[OnlineClassroom] Server already running:', info);
        
        // Auto-connect to server with retry
        await autoConnectWithRetry(info.url);
        return;
      } catch {
        // Server not running, start it
      }

      // Start server
      setServerStatus('starting');
      setServerError(null);
      try {
        const info = await invoke<ServerInfo>('start_server');
        console.log('[OnlineClassroom] Server start command returned:', info);
        setServerInfo(info);
        
        // Wait for server to be fully ready (npm run dev + tsx watch needs more time)
        // We'll try to connect with retries instead of fixed wait
        serverStarted.current = true;
        
        // Auto-connect with retry (server may need 5-10 seconds to fully start)
        await autoConnectWithRetry(info.url);
        
        setServerStatus('running');
        console.log('[OnlineClassroom] Server ready and connected:', info);
        
        // Add system message
        setMessages(prev => [...prev, {
          id: Date.now().toString(),
          senderId: 'system',
          senderName: 'Hệ thống',
          content: `Server đã khởi động. Học sinh có thể kết nối qua: ${info.url}`,
          timestamp: new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }),
          role: UserRole.ADMIN
        }]);
      } catch (error) {
        console.error('[OnlineClassroom] Failed to start server:', error);
        setServerStatus('error');
        setServerError(String(error));
      }
    };

    // Helper function to connect with retries
    const autoConnectWithRetry = async (url: string) => {
      const maxRetries = 10;
      const retryDelay = 2000; // 2 seconds between retries
      
      for (let i = 0; i < maxRetries; i++) {
        try {
          console.log(`[OnlineClassroom] Auto-connect attempt ${i + 1}/${maxRetries}...`);
          await connect(url, roomId, user.userName, true);
          console.log('[OnlineClassroom] Auto-connected successfully!');
          return;
        } catch (err) {
          console.warn(`[OnlineClassroom] Connect attempt ${i + 1} failed:`, err);
          if (i < maxRetries - 1) {
            await new Promise(resolve => setTimeout(resolve, retryDelay));
          }
        }
      }
      console.warn('[OnlineClassroom] All auto-connect attempts failed, user can try manually');
    };

    initServer();

    // Cleanup on unmount - no dependencies needed here
  }, [user.role, connect, user.userName]);

  // Stop server when component unmounts (only if not sharing)
  useEffect(() => {
    return () => {
      // Only stop server if not currently sharing
      // We check isSharing via a ref to avoid dependency issues
      if (serverStarted.current) {
        invoke('stop_server').catch(console.error);
        serverStarted.current = false;
      }
      // Clear retry interval for student
      if (retryIntervalRef.current) {
        clearInterval(retryIntervalRef.current);
        retryIntervalRef.current = null;
      }
    };
  }, []); // Empty dependency - only run on unmount

  // Auto-connect for Student role
  useEffect(() => {
    if (user.role !== UserRole.STUDENT || studentConnectStarted.current) {
      return;
    }

    // Try to get teacher server URL from localStorage or use default
    const savedUrl = localStorage.getItem('teacherServerUrl');
    const defaultUrl = savedUrl || 'ws://192.168.1.36:3016'; // Default to common LAN IP
    setTeacherServerUrl(defaultUrl);

    const connectToTeacher = async (url: string) => {
      setStudentConnectStatus('connecting');
      setStudentConnectError(null);
      
      try {
        console.log(`[Student] Connecting to teacher at ${url}...`);
        await connect(url, roomId, user.userName, false);
        setStudentConnectStatus('connected');
        setConnectRetryCount(0);
        studentConnectStarted.current = true;
        
        // Save successful URL
        localStorage.setItem('teacherServerUrl', url);
        
        // Add system message
        setMessages(prev => [...prev, {
          id: Date.now().toString(),
          senderId: 'system',
          senderName: 'Hệ thống',
          content: 'Đã kết nối tới lớp học. Chờ giáo viên bắt đầu chia sẻ màn hình.',
          timestamp: new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }),
          role: UserRole.ADMIN
        }]);
        
        console.log('[Student] Connected to teacher successfully!');
        return true;
      } catch (err) {
        console.warn('[Student] Connection failed:', err);
        return false;
      }
    };

    // Start auto-connect with retry
    const startAutoConnect = async () => {
      studentConnectStarted.current = true;
      
      // Initial connection attempt
      const connected = await connectToTeacher(defaultUrl);
      
      if (!connected) {
        // Start retry loop
        setStudentConnectStatus('error');
        setStudentConnectError('Không thể kết nối tới giáo viên. Đang thử lại...');
        
        let retryCount = 1;
        retryIntervalRef.current = setInterval(async () => {
          if (connectionState === 'connected') {
            if (retryIntervalRef.current) {
              clearInterval(retryIntervalRef.current);
              retryIntervalRef.current = null;
            }
            return;
          }
          
          setConnectRetryCount(retryCount);
          console.log(`[Student] Retry attempt ${retryCount}...`);
          
          const success = await connectToTeacher(teacherServerUrl || defaultUrl);
          if (success) {
            if (retryIntervalRef.current) {
              clearInterval(retryIntervalRef.current);
              retryIntervalRef.current = null;
            }
          } else {
            retryCount++;
            if (retryCount > 30) { // Stop after 30 retries (1 minute)
              setStudentConnectError('Không thể kết nối sau nhiều lần thử. Vui lòng kiểm tra địa chỉ server.');
              if (retryIntervalRef.current) {
                clearInterval(retryIntervalRef.current);
                retryIntervalRef.current = null;
              }
            }
          }
        }, 2000); // Retry every 2 seconds
      }
    };

    startAutoConnect();

    return () => {
      if (retryIntervalRef.current) {
        clearInterval(retryIntervalRef.current);
        retryIntervalRef.current = null;
      }
    };
  }, [user.role, user.userName, connect, connectionState, teacherServerUrl]);

  // Update student connect status based on connectionState
  useEffect(() => {
    if (user.role === UserRole.STUDENT) {
      if (connectionState === 'connected') {
        setStudentConnectStatus('connected');
        setStudentConnectError(null);
      } else if (connectionState === 'connecting') {
        setStudentConnectStatus('connecting');
      } else if (connectionState === 'disconnected' && studentConnectStarted.current) {
        // Connection lost, try to reconnect
        setStudentConnectStatus('error');
        setStudentConnectError('Mất kết nối. Đang thử kết nối lại...');
      }
    }
  }, [connectionState, user.role]);

  // Manual reconnect for student
  const handleStudentReconnect = useCallback(async () => {
    if (!teacherServerUrl) return;
    
    setStudentConnectStatus('connecting');
    setStudentConnectError(null);
    
    try {
      // Disconnect first if needed
      if (connectionState !== 'disconnected') {
        disconnect();
        await new Promise(resolve => setTimeout(resolve, 500));
      }
      
      await connect(teacherServerUrl, roomId, user.userName, false);
      setStudentConnectStatus('connected');
      localStorage.setItem('teacherServerUrl', teacherServerUrl);
    } catch (err) {
      setStudentConnectStatus('error');
      setStudentConnectError(String(err));
    }
  }, [teacherServerUrl, connectionState, disconnect, connect, user.userName]);

  const studentCount = peers.filter(p => !p.isTeacher).length;

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [messages]);

  const handleSendMessage = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!inputMessage.trim()) return;

    // Add message locally for immediate feedback
    const newMsg: Message = {
      id: Date.now().toString(),
      senderId: user.userId.toString(),
      senderName: user.userName,
      content: inputMessage,
      timestamp: new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }),
      role: user.role
    };
    setMessages(prev => [...prev, newMsg]);

    // Send via Mediasoup if connected (server will broadcast to others, not back to sender)
    if (connectionState === 'connected') {
      sendChatMessage(inputMessage);
    }
    
    setInputMessage('');
  };

  const handleStartTeaching = useCallback(async () => {
    if (!serverInfo) {
      setServerError('Server chưa sẵn sàng');
      return;
    }

    try {
      // If not connected, try to connect first
      if (connectionState === 'disconnected') {
        console.log('[OnlineClassroom] Not connected, connecting now...');
        await connect(serverInfo.url, roomId, user.userName, true);
        // Small delay to ensure connection is stable
        await new Promise(resolve => setTimeout(resolve, 300));
      }
      
      // Start screen sharing - getDisplayMedia is called here
      // Must be in direct response to user click (no retry loops before this)
      console.log('[OnlineClassroom] Starting screen share...');
      await startScreenShare(true);
      // isSharing will be set by the hook automatically
      
      // Add system message
      setMessages(prev => [...prev, {
        id: Date.now().toString(),
        senderId: 'system',
        senderName: 'Hệ thống',
        content: 'Giáo viên đã bắt đầu chia sẻ màn hình. Tất cả học sinh đang xem.',
        timestamp: new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }),
        role: UserRole.ADMIN
      }]);
      
      onStartScreenShare?.();
    } catch (error) {
      console.error('[OnlineClassroom] Failed to start teaching:', error);
      setServerError(String(error));
    }
  }, [serverInfo, connectionState, connect, startScreenShare, user.userName, onStartScreenShare]);

  const handleStopTeaching = useCallback(async () => {
    try {
      await stopScreenShare();
      // isSharing will be set to false by the hook automatically
      
      // Add system message
      setMessages(prev => [...prev, {
        id: Date.now().toString(),
        senderId: 'system',
        senderName: 'Hệ thống',
        content: 'Giáo viên đã kết thúc chia sẻ màn hình.',
        timestamp: new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }),
        role: UserRole.ADMIN
      }]);
      
      onStopScreenShare?.();
    } catch (error) {
      console.error('[OnlineClassroom] Failed to stop teaching:', error);
    }
  }, [stopScreenShare, onStopScreenShare]);

  const handleToggleTeaching = () => {
    if (isSharing) {
      handleStopTeaching();
    } else {
      handleStartTeaching();
    }
  };

  const handleToggleMic = async () => {
    if (isMicActive) {
      await stopMicrophone();
    } else {
      await startMicrophone();
    }
  };

  return (
    <div className="h-[calc(100vh-10rem)] flex flex-col gap-4">
      {/* Server Status Bar for Teacher */}
      {user.role === UserRole.TEACHER && (
        <div className={`flex items-center justify-center gap-3 py-2 text-xs font-bold rounded-xl ${
          serverStatus === 'running' && connectionState === 'connected' ? 'bg-emerald-500 text-white' :
          serverStatus === 'running' && connectionState === 'connecting' ? 'bg-blue-500 text-white' :
          serverStatus === 'running' ? 'bg-amber-500 text-white' :
          serverStatus === 'starting' ? 'bg-amber-500 text-white' :
          serverStatus === 'error' ? 'bg-rose-500 text-white' :
          'bg-slate-500 text-white'
        }`}>
          {serverStatus === 'running' && connectionState === 'connected' && (
              <>
                <CheckCircle2 className="w-4 h-4" />
                <span>Sẵn sàng giảng dạy</span>
                <span className="px-2 py-0.5 bg-white/20 rounded">{serverInfo?.url}</span>
              </>
            )}
            {serverStatus === 'running' && connectionState === 'connecting' && (
              <>
                <Loader2 className="w-4 h-4 animate-spin" />
                <span>Đang kết nối tới server...</span>
              </>
            )}
            {serverStatus === 'running' && connectionState === 'disconnected' && (
              <>
                <Server className="w-4 h-4" />
                <span>Server đang chạy - Chờ kết nối...</span>
                <span className="px-2 py-0.5 bg-white/20 rounded">{serverInfo?.url}</span>
              </>
            )}
            {serverStatus === 'starting' && (
              <>
                <Loader2 className="w-4 h-4 animate-spin" />
                <span>Đang khởi động Mediasoup Server...</span>
              </>
            )}
            {serverStatus === 'error' && (
              <>
                <AlertCircle className="w-4 h-4" />
                <span>Lỗi: {serverError}</span>
              </>
            )}
            {serverStatus === 'stopped' && (
              <>
                <Server className="w-4 h-4" />
                <span>Server chưa khởi động</span>
              </>
            )}
        </div>
      )}

      {/* Connection Status Bar for Student */}
      {user.role === UserRole.STUDENT && (
        <div className={`flex items-center justify-center gap-3 py-3 text-xs font-bold rounded-xl ${
          studentConnectStatus === 'connected' ? 'bg-emerald-500 text-white' :
          studentConnectStatus === 'connecting' ? 'bg-blue-500 text-white' :
          studentConnectStatus === 'error' ? 'bg-amber-500 text-white' :
          'bg-slate-500 text-white'
        }`}>
          {studentConnectStatus === 'connected' && (
              <>
                <Wifi className="w-4 h-4" />
                <span>Đã kết nối</span>
              </>
            )}
            {studentConnectStatus === 'connecting' && (
              <>
                <Loader2 className="w-4 h-4 animate-spin" />
                <span>Đang kết nối...</span>
                {connectRetryCount > 0 && <span className="px-2 py-0.5 bg-white/20 rounded">Lần {connectRetryCount}</span>}
              </>
            )}
            {studentConnectStatus === 'error' && (
              <>
                <WifiOff className="w-4 h-4" />
                <span title={studentConnectError || undefined}>Lỗi kết nối</span>
              </>
            )}
            {studentConnectStatus === 'disconnected' && (
              <>
                <WifiOff className="w-4 h-4" />
                <span>Chưa kết nối</span>
              </>
            )}
            
            {/* Server URL Input - Always visible */}
            <div className="flex items-center gap-2 ml-2 pl-2 border-l border-white/30">
              <span className="text-white/70">Server:</span>
              <input
                type="text"
                value={teacherServerUrl}
                onChange={(e) => setTeacherServerUrl(e.target.value)}
                placeholder="ws://192.168.1.x:3016"
                className="px-2 py-1 bg-white/20 rounded text-white placeholder:text-white/50 text-xs w-44 focus:outline-none focus:ring-1 focus:ring-white/50"
                disabled={studentConnectStatus === 'connecting'}
              />
              {studentConnectStatus === 'connected' ? (
                <button 
                  onClick={() => {
                    disconnect();
                    setStudentConnectStatus('disconnected');
                    studentConnectStarted.current = false;
                  }}
                  className="px-2 py-1 bg-rose-500/80 rounded hover:bg-rose-500 transition text-xs"
                >
                  Ngắt
                </button>
              ) : (
                <button 
                  onClick={handleStudentReconnect}
                  disabled={studentConnectStatus === 'connecting' || !teacherServerUrl}
                  className="px-2 py-1 bg-white/30 rounded hover:bg-white/40 transition text-xs disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-1"
                >
                  {studentConnectStatus === 'connecting' ? (
                    <Loader2 className="w-3 h-3 animate-spin" />
                  ) : (
                    <RefreshCw className="w-3 h-3" />
                  )}
                  Kết nối
                </button>
              )}
            </div>
        </div>
      )}

      {/* Main Content Area */}
      <div className="flex-1 flex flex-col lg:flex-row gap-6">
        {/* Main Stream Area */}
        <div className="flex-1 bg-slate-900 rounded-3xl overflow-hidden flex flex-col relative">
          <div className="absolute top-4 left-4 z-10 flex items-center gap-2">
            {(isSharing || (user.role === UserRole.STUDENT && remoteStream)) && (
              <div className="bg-rose-600 text-white text-[10px] font-bold px-2 py-0.5 rounded animate-pulse">LIVE</div>
            )}
          {user.role === UserRole.TEACHER && (
            <div className="bg-slate-800/80 backdrop-blur-md text-white text-xs px-3 py-1 rounded-full flex items-center gap-2 border border-white/10">
              <Users className="w-3 h-3" /> {studentCount > 0 ? `${studentCount} học sinh đang xem` : 'Chưa có học sinh'}
            </div>
          )}
          {connectionState === 'connected' && (
            <div className="bg-emerald-600/80 backdrop-blur-md text-white text-[10px] px-2 py-0.5 rounded-full">
              🟢 Đã kết nối
            </div>
          )}
        </div>

        <div className="flex-1 flex items-center justify-center bg-slate-950">
          {/* Teacher View - Show local stream when sharing */}
          {user.role === UserRole.TEACHER && isSharing && localStream ? (
            <div className="w-full h-full relative">
              <VideoPlayer 
                stream={localStream} 
                muted={true} 
                label=""
                className="w-full h-full object-contain"
              />
              <div className="absolute bottom-6 right-6 w-48 aspect-video bg-slate-800 rounded-xl border-2 border-white/20 overflow-hidden shadow-2xl">
                 <img src={`https://api.dicebear.com/7.x/avataaars/svg?seed=${user.userName}`} className="w-full h-full object-cover" alt="Teacher" />
                 <div className="absolute bottom-2 left-2 text-[10px] text-white bg-black/50 px-1 rounded">Bạn (Giáo viên)</div>
              </div>
            </div>
          ) : user.role === UserRole.TEACHER && isSharing ? (
            <div className="text-center">
              <div className="w-20 h-20 bg-emerald-600/20 rounded-full flex items-center justify-center mx-auto mb-4 border border-emerald-500/30">
                <ScreenShare className="w-10 h-10 text-emerald-400" />
              </div>
              <h3 className="text-white text-xl font-bold">Đang chia sẻ màn hình</h3>
              <p className="text-slate-400 mt-2">Học sinh đang xem màn hình của bạn</p>
              <p className="text-amber-400 mt-2 text-sm">⚠️ Preview không khả dụng trên thiết bị này</p>
            </div>
          ) : user.role === UserRole.TEACHER ? (
            <div className="text-center p-8">
              <div className="w-20 h-20 bg-indigo-600/20 rounded-full flex items-center justify-center mx-auto mb-4 border border-indigo-500/30">
                <MonitorPlay className="w-10 h-10 text-indigo-400" />
              </div>
              <h3 className="text-white text-xl font-bold">Chưa bắt đầu chia sẻ</h3>
              <p className="text-slate-400 mt-2 max-w-sm">
                {serverStatus === 'running' 
                  ? 'Nhấn nút "Bắt đầu ca dạy" để chia sẻ màn hình và bắt đầu bài giảng của bạn.'
                  : 'Đang chờ server khởi động...'}
              </p>
              {mediasoupError && (
                <p className="text-rose-400 mt-4 text-sm">{mediasoupError}</p>
              )}
            </div>
          ) : null}

          {/* Student View - Show remote stream from teacher */}
          {user.role === UserRole.STUDENT && remoteStream && (
            <div className="w-full h-full relative">
              <VideoPlayer 
                stream={remoteStream} 
                muted={false} 
                label=""
                className="w-full h-full object-contain"
              />
            </div>
          )}

          {/* Student View - Waiting for teacher */}
          {user.role === UserRole.STUDENT && !remoteStream && (
            <div className="text-center p-8">
              <div className="w-20 h-20 bg-indigo-600/20 rounded-full flex items-center justify-center mx-auto mb-4 border border-indigo-500/30">
                <MonitorPlay className="w-10 h-10 text-indigo-400" />
              </div>
              <h3 className="text-white text-xl font-bold">
                {studentConnectStatus === 'connected' ? 'Chờ giáo viên chia sẻ màn hình' : 'Đang kết nối...'}
              </h3>
              <p className="text-slate-400 mt-2 max-w-sm">
                {studentConnectStatus === 'connected' 
                  ? 'Giáo viên chưa bắt đầu chia sẻ màn hình. Vui lòng chờ...'
                  : studentConnectStatus === 'connecting'
                    ? 'Đang kết nối tới máy giáo viên...'
                    : 'Không thể kết nối. Vui lòng kiểm tra địa chỉ server.'}
              </p>
              {mediasoupError && (
                <p className="text-rose-400 mt-4 text-sm">{mediasoupError}</p>
              )}
            </div>
          )}
        </div>

        {/* Controls - Different for Teacher and Student */}
        <div className="bg-slate-900/90 backdrop-blur-md border-t border-white/10 p-6 flex items-center justify-center gap-4">
          {/* Teacher Controls */}
          {user.role === UserRole.TEACHER && (
            <>
              <button 
                onClick={handleToggleMic}
                disabled={!isSharing}
                className={`p-3 rounded-full transition ${
                  isMicActive 
                    ? 'bg-emerald-600 text-white' 
                    : 'bg-slate-800 text-slate-400 hover:text-white'
                } ${!isSharing ? 'opacity-50 cursor-not-allowed' : ''}`}
                title={isMicActive ? 'Tắt microphone' : 'Bật microphone'}
              >
                {isMicActive ? <Mic className="w-6 h-6" /> : <MicOff className="w-6 h-6" />}
              </button>
              <button className="p-3 rounded-full bg-slate-800 text-slate-400 hover:text-white transition">
                <VideoOff className="w-6 h-6" />
              </button>
              <button 
                onClick={handleToggleTeaching}
                disabled={serverStatus !== 'running' || (connectionState !== 'connected' && !isSharing)}
                className={`px-8 py-3 rounded-2xl font-bold transition flex items-center gap-3 ${
                  isSharing 
                    ? 'bg-rose-600 text-white' 
                    : serverStatus === 'running' && connectionState === 'connected'
                      ? 'bg-indigo-600 text-white hover:bg-indigo-700'
                      : 'bg-slate-600 text-slate-300 cursor-not-allowed'
                }`}
              >
                {isSharing ? (
                  <><X className="w-5 h-5" /> Kết thúc ca dạy</>
                ) : serverStatus === 'starting' ? (
                  <><Loader2 className="w-5 h-5 animate-spin" /> Đang khởi động...</>
                ) : connectionState === 'connecting' ? (
                  <><Loader2 className="w-5 h-5 animate-spin" /> Đang kết nối...</>
                ) : connectionState !== 'connected' ? (
                  <><Server className="w-5 h-5" /> Chờ kết nối...</>
                ) : (
                  <><ScreenShare className="w-5 h-5" /> Bắt đầu ca dạy</>
                )}
              </button>
              <button className="p-3 rounded-full bg-slate-800 text-slate-400 hover:text-white transition">
                <Hand className="w-6 h-6" />
              </button>
            </>
          )}

          {/* Student Controls */}
          {user.role === UserRole.STUDENT && (
            <>
              {/* Push-to-talk button */}
              <button 
                onMouseDown={enablePushToTalk}
                onMouseUp={disablePushToTalk}
                onMouseLeave={disablePushToTalk}
                onTouchStart={enablePushToTalk}
                onTouchEnd={disablePushToTalk}
                disabled={studentConnectStatus !== 'connected'}
                className={`px-6 py-3 rounded-2xl font-bold transition flex items-center gap-2 ${
                  isPushToTalkActive 
                    ? 'bg-emerald-600 text-white scale-105' 
                    : studentConnectStatus === 'connected'
                      ? 'bg-slate-700 text-white hover:bg-slate-600'
                      : 'bg-slate-800 text-slate-500 cursor-not-allowed'
                }`}
                title="Giữ để nói"
              >
                {isPushToTalkActive ? <Mic className="w-5 h-5 animate-pulse" /> : <MicOff className="w-5 h-5" />}
                <span>{isPushToTalkActive ? 'Đang nói...' : 'Giữ để nói'}</span>
              </button>

              {/* Raise hand button */}
              <button 
                className="p-3 rounded-full bg-slate-800 text-slate-400 hover:text-amber-400 hover:bg-amber-500/20 transition"
                title="Giơ tay"
              >
                <Hand className="w-6 h-6" />
              </button>

              {/* Reconnect button if disconnected */}
              {studentConnectStatus !== 'connected' && (
                <button 
                  onClick={handleStudentReconnect}
                  className="px-6 py-3 rounded-2xl font-bold bg-indigo-600 text-white hover:bg-indigo-700 transition flex items-center gap-2"
                >
                  <RefreshCw className="w-5 h-5" />
                  <span>Kết nối lại</span>
                </button>
              )}
            </>
          )}
        </div>
      </div>

      {/* Sidebar Chat & Users */}
      <div className="w-full lg:w-80 flex flex-col gap-4 min-h-0">
        <div className="flex-1 bg-white rounded-3xl border border-slate-200 flex flex-col overflow-hidden shadow-sm min-h-0">
          <div className="p-4 border-b border-slate-100 flex items-center justify-between flex-shrink-0">
            <div className="flex items-center gap-2">
              <MessageCircle className="w-5 h-5 text-indigo-500" />
              <h3 className="font-bold text-slate-800">Trao đổi lớp học</h3>
            </div>
            <div className="flex items-center gap-1 text-[10px] text-slate-400 font-bold uppercase">
               <Bot className="w-3 h-3" /> Chat
            </div>
          </div>

          <div 
            ref={scrollRef}
            className="flex-1 p-4 space-y-4 overflow-y-auto bg-slate-50 min-h-0"
            style={{ maxHeight: '350px' }}
          >
            {messages.map((msg) => (
              <div key={msg.id} className={`flex flex-col ${msg.senderId === user.userId.toString() ? 'items-end' : 'items-start'}`}>
                <div className="flex items-center gap-1.5 mb-1 px-1">
                  <span className="text-[10px] font-bold text-slate-500">{msg.senderName}</span>
                  <span className="text-[10px] text-slate-400">{msg.timestamp}</span>
                </div>
                <div className={`
                  max-w-[85%] p-3 rounded-2xl text-sm shadow-sm
                  ${msg.senderId === user.userId.toString() 
                    ? 'bg-indigo-600 text-white rounded-tr-none' 
                    : msg.senderId === 'system' 
                      ? 'bg-emerald-50 text-emerald-800 border border-emerald-100 rounded-tl-none italic'
                      : 'bg-white text-slate-800 border border-slate-200 rounded-tl-none'}
                `}>
                  {msg.content}
                </div>
              </div>
            ))}
          </div>

          <form onSubmit={handleSendMessage} className="p-4 border-t border-slate-100 flex-shrink-0 bg-white">
            <div className="relative">
              <input 
                type="text" 
                value={inputMessage}
                onChange={(e) => setInputMessage(e.target.value)}
                placeholder="Nhập tin nhắn..."
                className="w-full bg-slate-100 border border-slate-200 rounded-2xl pl-4 pr-12 py-3 text-sm text-slate-800 placeholder:text-slate-400 focus:ring-2 focus:ring-indigo-500 focus:bg-white focus:border-indigo-300 transition-all outline-none"
              />
              <button type="submit" className="absolute right-2 top-2 p-1.5 bg-indigo-600 text-white rounded-xl hover:bg-indigo-700 transition">
                <Send className="w-4 h-4" />
              </button>
            </div>
          </form>
        </div>

        <div className="bg-white p-4 rounded-3xl border border-slate-200 shadow-sm">
           <div className="flex items-center gap-2 mb-4">
              <Users className="w-4 h-4 text-slate-400" />
              <span className="text-xs font-bold text-slate-500 uppercase tracking-wider">Học sinh đang xem ({studentCount})</span>
           </div>
           <div className="grid grid-cols-4 gap-2">
             {peers.filter(p => !p.isTeacher).length > 0 ? (
               peers.filter(p => !p.isTeacher).map((peer) => (
                 <div key={peer.id} className="relative group">
                   <img src={`https://i.pravatar.cc/150?u=${peer.id}`} className="w-full aspect-square rounded-xl object-cover ring-2 ring-transparent group-hover:ring-indigo-500 transition cursor-pointer" alt={peer.name} title={peer.name} />
                   <div className="absolute bottom-0 right-0 w-3 h-3 bg-emerald-500 border-2 border-white rounded-full"></div>
                   <div className="absolute inset-0 bg-black/50 opacity-0 group-hover:opacity-100 transition rounded-xl flex items-center justify-center">
                     <span className="text-white text-[10px] font-bold">{peer.name}</span>
                   </div>
                 </div>
               ))
             ) : (
               Array.from({length: 8}).map((_, i) => (
                 <div key={i} className="relative group">
                   <div className="w-full aspect-square rounded-xl bg-slate-100 flex items-center justify-center">
                     <Users className="w-4 h-4 text-slate-300" />
                   </div>
                 </div>
               ))
             )}
             {peers.filter(p => !p.isTeacher).length > 8 && (
               <div className="w-full aspect-square bg-slate-100 rounded-xl flex items-center justify-center text-slate-400 text-xs font-bold cursor-pointer hover:bg-slate-200 transition">
                 +{peers.filter(p => !p.isTeacher).length - 8}
               </div>
             )}
           </div>
        </div>
      </div>
      </div>

      {/* Custom Sharing Indicator - only show for Teacher when sharing */}
      {user.role === UserRole.TEACHER && isSharing && (
        <SharingIndicator onStop={stopScreenShare} />
      )}
    </div>
  );
};

export default OnlineClassroom;
