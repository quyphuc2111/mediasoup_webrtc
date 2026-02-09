import React, { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getVersion } from '@tauri-apps/api/app';
import { 
  LayoutDashboard, Users, Monitor, Calendar, 
  MessageSquare, FileText, LogOut, 
  Bell, 
  MonitorPlay, Menu, Database, Loader2, AlertCircle, Wifi, WifiOff, Download, X
} from 'lucide-react';
import Dashboard from './views/Dashboard';
import LabControl from './views/LabControl';
import UserManagement from './views/UserManagement';
import SessionManagement from './views/SessionManagement';
import OnlineClassroom from './views/OnlineClassroom';
import DocumentManager from './views/DocumentManager';
import Messaging from './views/Messaging';
import SystemConfig from './views/SystemConfig';
import UpdatesPage from './views/UpdatesPage';
import UpdateRequiredScreen from './components/UpdateRequiredScreen';
import { ScreenSharingPage } from './pages/ScreenSharingPage';
import { ViewClientPage } from './pages/ViewClientPage';
import { FileTransferPage } from './pages/FileTransferPage';
import { UserAccount as User, UserRole } from './types';
import './App.css';

// Update info type for notifications
interface UpdateInfo {
  version: string;
  published_at: string;
  release_notes: string;
}

// Backend response types
interface LoginResponse {
  success: boolean;
  message: string;
  user: {
    user_id: number;
    user_name: string;
    role: string;
    status: boolean;
    created_at: string;
  } | null;
}

// Agent status types
interface AgentStatus {
  Stopped?: null;
  Starting?: null;
  WaitingForTeacher?: null;
  Authenticating?: null;
  Connected?: { teacher_name: string };
  Error?: { message: string };
}

type SubPage = 'none' | 'screen-sharing' | 'view-client' | 'file-transfer';

const App: React.FC = () => {
  const [isSidebarOpen, setIsSidebarOpen] = useState(true);
  const [activeTab, setActiveTab] = useState('dashboard');
  const [currentUser, setCurrentUser] = useState<User | null>(null);
  const [isLoginView, setIsLoginView] = useState(true);
  const [subPage, setSubPage] = useState<SubPage>('none');
  
  // Login form state
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [loginError, setLoginError] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const [dbInitialized, setDbInitialized] = useState(false);

  // Student Agent state
  const [agentStatus, setAgentStatus] = useState<string>('Stopped');
  const [agentPort, setAgentPort] = useState<number>(3017);
  const [teacherIp, setTeacherIp] = useState<string>('');
  const agentStarted = useRef(false);

  // Document Server state (for Teacher)
  const [docServerUrl, setDocServerUrl] = useState<string>('');
  const docServerStarted = useRef(false);

  // Student Update state
  const [updateRequired, setUpdateRequired] = useState<boolean>(false);
  const [checkingUpdate, setCheckingUpdate] = useState<boolean>(false);

  // Teacher Update notification state (Requirements: 2.1)
  const [updateAvailable, setUpdateAvailable] = useState<UpdateInfo | null>(null);
  const [showUpdateNotification, setShowUpdateNotification] = useState<boolean>(false);
  const updateCheckDone = useRef(false);

  // App version state
  const [appVersion, setAppVersion] = useState<string>('...');

  // Get app version on mount
  useEffect(() => {
    getVersion().then(setAppVersion).catch(() => setAppVersion('unknown'));
  }, []);

  // Initialize database on app start
  useEffect(() => {
    const initDb = async () => {
      try {
        await invoke('init_db');
        setDbInitialized(true);
        console.log('✅ Database initialized');
      } catch (error) {
        console.error('❌ Failed to initialize database:', error);
        setLoginError('Không thể khởi tạo cơ sở dữ liệu');
      }
    };
    initDb();
  }, []);

  // Auto-start Student Agent for Student role
  useEffect(() => {
    if (!currentUser || currentUser.role !== UserRole.STUDENT || agentStarted.current) {
      return;
    }

    const startAgent = async () => {
      try {
        // Check if agent is already running before starting
        try {
          const currentStatus = await invoke<string>('get_agent_status');
          if (currentStatus !== 'Stopped') {
            console.log('[StudentAgent] Agent already running, skipping auto-start');
            agentStarted.current = true;
            return;
          }
        } catch (_) { /* ignore */ }

        console.log('[StudentAgent] Auto-starting agent for student...');
        setAgentStatus('Starting');
        
        // Start student agent (WebSocket server + integrated UDP discovery listener)
        await invoke('start_student_agent', {
          port: 3017,
          studentName: currentUser.userName
        });
        
        agentStarted.current = true;
        console.log('[StudentAgent] Agent started successfully (with integrated discovery)');
      } catch (error) {
        console.error('[StudentAgent] Failed to start agent:', error);
        setAgentStatus(`Error: ${error}`);
        // Retry after 3 seconds if failed
        setTimeout(() => {
          agentStarted.current = false;
          startAgent();
        }, 3000);
      }
    };

    // Small delay to ensure app is fully loaded
    const timeoutId = setTimeout(startAgent, 500);

    // Cleanup on unmount
    return () => {
      clearTimeout(timeoutId);
      if (agentStarted.current) {
        invoke('stop_student_agent').catch(console.error);
        invoke('stop_discovery_listener').catch(console.error);
        agentStarted.current = false;
      }
    };
  }, [currentUser]);

  // Auto-start Document Server for Teacher/Admin role
  useEffect(() => {
    if (!currentUser || (currentUser.role !== UserRole.TEACHER && currentUser.role !== UserRole.ADMIN) || docServerStarted.current) {
      return;
    }

    const startDocServer = async () => {
      try {
        console.log('[DocumentServer] Auto-starting document server for teacher...');
        
        const url = await invoke<string>('start_document_server', { port: 8765 });
        setDocServerUrl(url);
        docServerStarted.current = true;
        console.log('[DocumentServer] Server started at:', url);
      } catch (error) {
        console.error('[DocumentServer] Failed to start server:', error);
        // Retry after 3 seconds if failed
        setTimeout(() => {
          docServerStarted.current = false;
          startDocServer();
        }, 3000);
      }
    };

    // Small delay to ensure app is fully loaded
    const timeoutId = setTimeout(startDocServer, 500);

    // Cleanup on unmount
    return () => {
      clearTimeout(timeoutId);
      if (docServerStarted.current) {
        invoke('stop_document_server').catch(console.error);
        docServerStarted.current = false;
      }
    };
  }, [currentUser]);

  // Check for updates on startup for Teacher/Admin role (Requirements: 2.1)
  useEffect(() => {
    if (!currentUser || (currentUser.role !== UserRole.TEACHER && currentUser.role !== UserRole.ADMIN) || updateCheckDone.current) {
      return;
    }

    const checkForUpdates = async () => {
      try {
        console.log('[AutoUpdate] Checking for updates on startup...');
        const info = await invoke<UpdateInfo | null>('check_for_updates');
        
        if (info) {
          console.log('[AutoUpdate] Update available:', info.version);
          setUpdateAvailable(info);
          setShowUpdateNotification(true);
        } else {
          console.log('[AutoUpdate] No updates available');
        }
        
        updateCheckDone.current = true;
      } catch (error) {
        console.error('[AutoUpdate] Failed to check for updates:', error);
        // Don't show error to user - update check is non-critical
        updateCheckDone.current = true;
      }
    };

    // Delay update check to not interfere with app startup
    const timeoutId = setTimeout(checkForUpdates, 2000);

    return () => {
      clearTimeout(timeoutId);
    };
  }, [currentUser]);

  // Track if we've already triggered update required to avoid duplicate calls
  const updateRequiredTriggered = useRef(false);

  // Poll agent status for Student role
  useEffect(() => {
    if (!currentUser || currentUser.role !== UserRole.STUDENT) {
      return;
    }

    const pollStatus = async () => {
      try {
        const status = await invoke<AgentStatus>('get_agent_status');
        console.log('[App] Agent status:', status);
        
        // Status can be either a string or an object
        if (typeof status === 'string') {
          // Simple string status like "Stopped", "Starting", "WaitingForTeacher"
          if (status === 'Stopped') setAgentStatus('Stopped');
          else if (status === 'Starting') setAgentStatus('Starting');
          else if (status === 'WaitingForTeacher') setAgentStatus('Waiting');
          else if (status === 'Authenticating') setAgentStatus('Authenticating');
          else setAgentStatus(status);
          setTeacherIp('');
        } else if (typeof status === 'object' && status !== null) {
          // Object status like { Connected: { teacher_name: "...", teacher_ip: "..." } }
          if ('Stopped' in status) {
            setAgentStatus('Stopped');
            setTeacherIp('');
          }
          else if ('Starting' in status) setAgentStatus('Starting');
          else if ('WaitingForTeacher' in status) setAgentStatus('Waiting');
          else if ('Authenticating' in status) setAgentStatus('Authenticating');
          else if ('Connected' in status && (status as any).Connected) {
            const connected = (status as any).Connected;
            setAgentStatus(`Connected: ${connected.teacher_name}`);
            setTeacherIp(connected.teacher_ip || '');
          }
          else if ('UpdateRequired' in status && (status as any).UpdateRequired) {
            // Bridge AgentStatus::UpdateRequired to StudentUpdateCoordinator
            // This triggers the update modal to show
            const updateInfo = (status as any).UpdateRequired;
            console.log('[App] Agent status is UpdateRequired:', updateInfo);
            setAgentStatus('UpdateRequired');
            
            // Only trigger once to avoid duplicate calls
            if (!updateRequiredTriggered.current) {
              updateRequiredTriggered.current = true;
              console.log('[App] Triggering StudentUpdateCoordinator with update info...');
              
              try {
                // Call set_student_update_required to trigger StudentUpdateCoordinator
                // This will emit 'student-update-state-changed' event that UpdateRequiredScreen listens to
                await invoke('set_student_update_required', {
                  requiredVersion: updateInfo.required_version,
                  updateUrl: updateInfo.update_url || null,
                  sha256: updateInfo.sha256 || null,
                });
                console.log('[App] StudentUpdateCoordinator triggered successfully');
              } catch (error) {
                console.error('[App] Failed to trigger StudentUpdateCoordinator:', error);
              }
            }
          }
          else if ('Updating' in status && (status as any).Updating) {
            const updating = (status as any).Updating;
            setAgentStatus(`Updating: ${(updating.progress * 100).toFixed(0)}%`);
          }
          else if ('Error' in status && (status as any).Error) {
            setAgentStatus(`Error: ${(status as any).Error.message}`);
            setTeacherIp('');
          }
        }

        // Get port from config
        const config = await invoke<{ port: number; student_name: string }>('get_agent_config');
        setAgentPort(config.port);
      } catch (error) {
        console.error('[App] Error polling agent status:', error);
      }
    };

    // Poll immediately and then every 1 second (faster polling)
    pollStatus();
    const interval = setInterval(pollStatus, 1000);
    return () => clearInterval(interval);
  }, [currentUser]);

  // Check for student update requirements
  useEffect(() => {
    if (!currentUser || currentUser.role !== UserRole.STUDENT) {
      return;
    }

    // Check initial update state
    const checkUpdateState = async () => {
      try {
        setCheckingUpdate(true);
        const state = await invoke<any>('get_student_update_state');
        console.log('[App] Student update state:', state);
        
        // Check if update is required
        const requiresUpdate = state.type === 'UpdateRequired' || 
                              state.type === 'Downloading' || 
                              state.type === 'Verifying' || 
                              state.type === 'ReadyToInstall' || 
                              state.type === 'Installing' ||
                              state.type === 'Restarting' ||
                              state.type === 'Failed';
        
        setUpdateRequired(requiresUpdate);
      } catch (error) {
        console.error('[App] Failed to check update state:', error);
      } finally {
        setCheckingUpdate(false);
      }
    };

    checkUpdateState();

    // Listen for update state changes
    const unlisten = listen<{ state: any; timestamp: number }>('student-update-state-changed', (event) => {
      console.log('[App] Student update state changed:', event.payload);
      const state = event.payload.state;
      
      const requiresUpdate = state.type === 'UpdateRequired' || 
                            state.type === 'Downloading' || 
                            state.type === 'Verifying' || 
                            state.type === 'ReadyToInstall' || 
                            state.type === 'Installing' ||
                            state.type === 'Restarting' ||
                            state.type === 'Failed';
      
      setUpdateRequired(requiresUpdate);
      
      // If update is done, clear the flag
      if (state.type === 'Done' || state.type === 'Idle') {
        setUpdateRequired(false);
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [currentUser]);

  // Convert backend role string to UserRole enum
  const mapRole = (role: string): UserRole => {
    switch (role) {
      case 'Administrator': return UserRole.ADMIN;
      case 'Teacher': return UserRole.TEACHER;
      case 'Student': return UserRole.STUDENT;
      default: return UserRole.STUDENT;
    }
  };

  const handleLogin = async (e: React.FormEvent) => {
    e.preventDefault();
    setLoginError('');
    setIsLoading(true);

    try {
      const response = await invoke<LoginResponse>('login', { 
        username: username.trim(), 
        password 
      });

      if (response.success && response.user) {
        setCurrentUser({
          userId: response.user.user_id,
          userName: response.user.user_name,
          role: mapRole(response.user.role),
          status: response.user.status
        });
        setIsLoginView(false);
        setUsername('');
        setPassword('');
      } else {
        setLoginError(response.message || 'Đăng nhập thất bại');
      }
    } catch (error) {
      console.error('Login error:', error);
      setLoginError('Lỗi kết nối. Vui lòng thử lại.');
    } finally {
      setIsLoading(false);
    }
  };

  const handleLogout = async () => {
    // Stop student agent if running
    if (currentUser?.role === UserRole.STUDENT && agentStarted.current) {
      try {
        await invoke('stop_student_agent');
        agentStarted.current = false;
      } catch (error) {
        console.error('Failed to stop agent:', error);
      }
    }
    
    // Stop document server if running
    if ((currentUser?.role === UserRole.TEACHER || currentUser?.role === UserRole.ADMIN) && docServerStarted.current) {
      try {
        await invoke('stop_document_server');
        docServerStarted.current = false;
        setDocServerUrl('');
      } catch (error) {
        console.error('Failed to stop document server:', error);
      }
    }
    
    // Reset update required trigger flag
    updateRequiredTriggered.current = false;
    
    setCurrentUser(null);
    setIsLoginView(true);
    setSubPage('none');
    setAgentStatus('Stopped');
    setUpdateRequired(false);
  };

  const handleBackFromSubPage = () => {
    setSubPage('none');
  };

  // Render sub pages (full screen)
  if (subPage === 'screen-sharing') {
    return <ScreenSharingPage onBack={handleBackFromSubPage} />;
  }

  if (subPage === 'view-client') {
    return <ViewClientPage onBack={handleBackFromSubPage} />;
  }

  if (subPage === 'file-transfer') {
    return <FileTransferPage onBack={handleBackFromSubPage} />;
  }

  // Show update required screen for students when update is needed
  // Requirements: 6.1, 6.2 - Block main functionality when update required
  if (currentUser?.role === UserRole.STUDENT && updateRequired && !checkingUpdate) {
    return <UpdateRequiredScreen onUpdateComplete={() => setUpdateRequired(false)} />;
  }

  if (isLoginView) {
    return (
      <div className="min-h-screen bg-slate-950 flex items-center justify-center p-4">
        <div className="max-w-md w-full bg-white rounded-[40px] shadow-2xl overflow-hidden">
          <div className="p-12 bg-indigo-600 text-white text-center relative overflow-hidden">
            <div className="absolute top-0 right-0 p-8 opacity-10"><Monitor className="w-32 h-32" /></div>
            <Monitor className="w-16 h-16 mx-auto mb-6 bg-white/20 p-4 rounded-2xl" />
            <h1 className="text-3xl font-black uppercase tracking-tight">Smart Lab ProMax</h1>
            <p className="text-indigo-100 mt-2 text-sm font-medium">Hệ thống quản lý phòng máy số hóa</p>
            <p className="text-indigo-200 mt-1 text-xs font-bold">v{appVersion}</p>
          </div>
          <form onSubmit={handleLogin} className="p-12 space-y-6 bg-slate-50">
            {!dbInitialized && (
              <div className="flex items-center justify-center gap-2 text-slate-500 py-4">
                <Loader2 className="w-5 h-5 animate-spin" />
                <span className="text-sm">Đang khởi tạo...</span>
              </div>
            )}
            
            {loginError && (
              <div className="flex items-center gap-3 p-4 bg-rose-50 border border-rose-200 rounded-2xl text-rose-600">
                <AlertCircle className="w-5 h-5 flex-shrink-0" />
                <span className="text-sm font-medium">{loginError}</span>
              </div>
            )}

            <div className="space-y-2">
              <label className="block text-xs font-black text-slate-500 uppercase tracking-widest">
                Tên đăng nhập
              </label>
              <input
                type="text"
                value={username}
                onChange={(e) => setUsername(e.target.value)}
                placeholder="Nhập tên đăng nhập"
                className="w-full px-5 py-4 bg-white border-2 border-slate-200 rounded-2xl text-slate-800 font-medium placeholder:text-slate-400 focus:border-indigo-500 focus:outline-none transition-colors"
                disabled={!dbInitialized || isLoading}
                required
              />
            </div>

            <div className="space-y-2">
              <label className="block text-xs font-black text-slate-500 uppercase tracking-widest">
                Mật khẩu
              </label>
              <input
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                placeholder="Nhập mật khẩu (để trống nếu không có)"
                className="w-full px-5 py-4 bg-white border-2 border-slate-200 rounded-2xl text-slate-800 font-medium placeholder:text-slate-400 focus:border-indigo-500 focus:outline-none transition-colors"
                disabled={!dbInitialized || isLoading}
              />
            </div>

            <button 
              type="submit"
              disabled={!dbInitialized || isLoading}
              className="w-full py-4 bg-indigo-600 text-white rounded-2xl font-black uppercase tracking-widest hover:bg-indigo-700 hover:scale-[1.02] transition active:scale-95 flex items-center justify-center gap-3 shadow-xl shadow-indigo-600/20 disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:scale-100"
            >
              {isLoading ? (
                <>
                  <Loader2 className="w-5 h-5 animate-spin" />
                  Đang đăng nhập...
                </>
              ) : (
                'Đăng nhập'
              )}
            </button>

            <div className="pt-4 border-t border-slate-200">
              <p className="text-xs text-slate-400 text-center font-medium">
                Tài khoản mặc định:
              </p>
              <div className="mt-3 grid grid-cols-3 gap-2 text-[10px] text-slate-500">
                <div className="bg-white p-2 rounded-xl text-center">
                  <div className="font-black text-slate-700">admin</div>
                  <div className="text-slate-400">admin123</div>
                </div>
                <div className="bg-white p-2 rounded-xl text-center">
                  <div className="font-black text-slate-700">teacher</div>
                  <div className="text-slate-400">teacher123</div>
                </div>
                <div className="bg-white p-2 rounded-xl text-center">
                  <div className="font-black text-slate-700">student</div>
                  <div className="text-slate-400">student123</div>
                </div>
              </div>
            </div>
          </form>
        </div>
      </div>
    );
  }

  const menuItems = [
    { id: 'dashboard', label: 'Trung tâm điều khiển', icon: LayoutDashboard, roles: [UserRole.ADMIN, UserRole.TEACHER] },
    { id: 'system', label: 'Danh mục & Mapping', icon: Database, roles: [UserRole.ADMIN] },
    { id: 'users', label: 'Người dùng & Phân quyền', icon: Users, roles: [UserRole.ADMIN] },
    { id: 'labs', label: 'Quản lý Phòng máy', icon: Monitor, roles: [UserRole.ADMIN, UserRole.TEACHER] },
    { id: 'sessions', label: 'Ca thực hành', icon: Calendar, roles: [UserRole.ADMIN, UserRole.TEACHER, UserRole.STUDENT] },
    { id: 'classroom', label: 'Hỗ trợ giảng dạy', icon: MonitorPlay, roles: [UserRole.TEACHER, UserRole.STUDENT] },
    { id: 'messaging', label: 'Hệ thống nhắn tin', icon: MessageSquare, roles: [UserRole.ADMIN, UserRole.TEACHER, UserRole.STUDENT] },
    { id: 'documents', label: 'Phân phối tài liệu', icon: FileText, roles: [UserRole.ADMIN, UserRole.TEACHER, UserRole.STUDENT] },
    { id: 'updates', label: 'Cập nhật hệ thống', icon: Download, roles: [UserRole.ADMIN, UserRole.TEACHER] },
  ];

  const filteredMenuItems = menuItems.filter(item => item.roles.includes(currentUser!.role));

  return (
    <div className="flex h-screen bg-slate-50 overflow-hidden">
      <aside className={`${isSidebarOpen ? 'w-80' : 'w-24'} transition-all duration-500 bg-slate-950 flex flex-col z-50`}>
        <div className="p-10 flex items-center gap-4">
          <div className="p-3 bg-indigo-500 rounded-2xl shadow-lg shadow-indigo-500/40 animate-pulse"><Monitor className="text-white w-6 h-6" /></div>
          {isSidebarOpen && (
            <div>
              <span className="font-black text-white text-2xl tracking-tighter italic">SMART LAB <span className='text-green-600'>ProMax</span></span>
              <p className="text-[10px] text-slate-500 font-bold mt-1">v{appVersion}</p>
            </div>
          )}
        </div>
        <nav className="flex-1 px-5 space-y-2 overflow-y-auto scrollbar-hide">
          {filteredMenuItems.map(item => (
            <button 
              key={item.id} 
              onClick={() => setActiveTab(item.id)}
              className={`w-full flex items-center gap-5 p-4 rounded-[24px] font-black transition-all group ${activeTab === item.id ? 'bg-indigo-600 text-white shadow-2xl shadow-indigo-600/40' : 'text-slate-500 hover:bg-white/5 hover:text-white'}`}
            >
              <item.icon className={`w-5 h-5 min-w-[20px] transition-transform group-hover:scale-110 ${activeTab === item.id ? 'text-white' : 'text-slate-600'}`} />
              {isSidebarOpen && <span className="text-sm uppercase tracking-widest">{item.label}</span>}
            </button>
          ))}
        </nav>
        <div className="p-8">
          <button onClick={handleLogout} className="w-full flex items-center gap-5 p-4 rounded-[24px] font-black text-rose-500 hover:bg-rose-500/10 transition-all uppercase tracking-widest text-xs">
            <LogOut className="w-5 h-5 min-w-[20px]" />
            {isSidebarOpen && <span>Thoát Smart Lab</span>}
          </button>
        </div>
      </aside>

      <main className="flex-1 flex flex-col min-w-0 bg-[#f8fafc]">
        <header className="h-24 bg-white border-b border-slate-200 flex items-center justify-between px-12 sticky top-0 z-40">
          <div className="flex items-center gap-8">
            <button onClick={() => setIsSidebarOpen(!isSidebarOpen)} className="p-3 hover:bg-slate-100 rounded-2xl text-slate-400 transition-colors"><Menu className="w-6 h-6" /></button>
            <div>
              <h2 className="text-2xl font-black text-slate-800 uppercase tracking-tighter">{menuItems.find(i => i.id === activeTab)?.label}</h2>
              <p className="text-[10px] font-bold text-slate-400 uppercase tracking-[0.3em] mt-1">Smart Lab Management System</p>
            </div>
          </div>
          <div className="flex items-center gap-8">
            {/* Agent Status for Students */}
            {currentUser?.role === UserRole.STUDENT && (
              <div className={`flex items-center gap-2 px-4 py-2 rounded-xl text-xs font-black uppercase tracking-tighter ${
                agentStatus === 'Waiting' 
                  ? 'bg-emerald-50 text-emerald-700 border border-emerald-100'
                  : agentStatus.startsWith('Connected')
                    ? 'bg-indigo-50 text-indigo-700 border border-indigo-100'
                    : agentStatus.startsWith('Error')
                      ? 'bg-rose-50 text-rose-700 border border-rose-100'
                      : 'bg-slate-50 text-slate-500 border border-slate-100'
              }`}>
                {agentStatus === 'Waiting' ? (
                  <><Wifi className="w-4 h-4" /> Agent: Port {agentPort}</>
                ) : agentStatus.startsWith('Connected') ? (
                  <><Wifi className="w-4 h-4 animate-pulse" /> {agentStatus}</>
                ) : agentStatus.startsWith('Error') ? (
                  <><WifiOff className="w-4 h-4" /> Agent Error</>
                ) : (
                  <><Loader2 className="w-4 h-4 animate-spin" /> {agentStatus}</>
                )}
              </div>
            )}

            {/* Document Server Status for Teachers */}
            {(currentUser?.role === UserRole.TEACHER || currentUser?.role === UserRole.ADMIN) && docServerUrl && (
              <div className="flex items-center gap-2 px-4 py-2 rounded-xl text-xs font-bold bg-emerald-50 text-emerald-700 border border-emerald-100">
                <FileText className="w-4 h-4" />
                <span>Tài liệu: {docServerUrl.replace('http://', '')}</span>
              </div>
            )}
            
            <button className="p-3 bg-slate-50 text-slate-400 rounded-2xl relative hover:bg-slate-100 transition-colors group">
              <Bell className="w-6 h-6 group-hover:rotate-12 transition-transform" />
              <span className="absolute top-2.5 right-2.5 w-3 h-3 bg-rose-500 rounded-full border-2 border-white"></span>
            </button>

            {/* Update Available Notification (Requirements: 2.1, 2.4) */}
            {showUpdateNotification && updateAvailable && (currentUser?.role === UserRole.TEACHER || currentUser?.role === UserRole.ADMIN) && (
              <div className="fixed top-28 right-12 z-50 animate-in slide-in-from-right duration-300">
                <div className="bg-white rounded-2xl shadow-2xl border border-slate-200 p-4 max-w-sm">
                  <div className="flex items-start gap-3">
                    <div className="p-2 bg-indigo-100 rounded-xl flex-shrink-0">
                      <Download className="w-5 h-5 text-indigo-600" />
                    </div>
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center justify-between gap-2">
                        <h3 className="text-sm font-bold text-slate-800">Có bản cập nhật mới!</h3>
                        <button
                          onClick={() => setShowUpdateNotification(false)}
                          className="p-1 hover:bg-slate-100 rounded-lg transition-colors"
                        >
                          <X className="w-4 h-4 text-slate-400" />
                        </button>
                      </div>
                      <p className="text-xs text-slate-500 mt-1">
                        Phiên bản {updateAvailable.version} đã sẵn sàng
                      </p>
                      <div className="flex items-center gap-2 mt-3">
                        <button
                          onClick={() => {
                            setActiveTab('updates');
                            setShowUpdateNotification(false);
                          }}
                          className="px-3 py-1.5 bg-indigo-600 text-white text-xs font-bold rounded-lg hover:bg-indigo-700 transition-colors"
                        >
                          Xem chi tiết
                        </button>
                        <button
                          onClick={() => setShowUpdateNotification(false)}
                          className="px-3 py-1.5 text-slate-500 text-xs font-bold hover:text-slate-700 transition-colors"
                        >
                          Để sau
                        </button>
                      </div>
                    </div>
                  </div>
                </div>
              </div>
            )}
            <div className="flex items-center gap-5 pl-8 border-l border-slate-200">
              <div className="text-right">
                <p className="text-sm font-black text-slate-800 leading-none">{currentUser?.userName}</p>
                <p className="text-[10px] font-black text-indigo-500 uppercase tracking-widest mt-1.5">{currentUser?.role}</p>
              </div>
              <div className="w-14 h-14 rounded-2xl bg-slate-900 border-4 border-white shadow-xl overflow-hidden transform hover:rotate-6 transition-transform cursor-pointer">
                <img src={`https://api.dicebear.com/7.x/bottts/svg?seed=${currentUser?.userName}`} alt="avatar" />
              </div>
            </div>
          </div>
        </header>

        <div className="flex-1 overflow-y-auto p-12">
          <div className="max-w-7xl mx-auto">
            {activeTab === 'dashboard' && <Dashboard role={currentUser!.role} />}
            {activeTab === 'system' && <SystemConfig />}
            {activeTab === 'labs' && <LabControl />}
            {activeTab === 'users' && <UserManagement />}
            {activeTab === 'sessions' && <SessionManagement role={currentUser!.role} />}
            {activeTab === 'classroom' && (
              <OnlineClassroom user={currentUser!} />
            )}
            {activeTab === 'documents' && <DocumentManager user={currentUser!} teacherIp={teacherIp} />}
            {activeTab === 'messaging' && <Messaging user={currentUser!} />}
            {activeTab === 'updates' && <UpdatesPage />}
          </div>
        </div>
      </main>
    </div>
  );
};

export default App;
