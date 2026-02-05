import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { Monitor, Wifi, WifiOff, CheckCircle2, AlertCircle, Loader2, X } from 'lucide-react';

interface AgentStatus {
  status: 'Stopped' | 'Starting' | 'WaitingForTeacher' | 'Authenticating' | 
          { Connected: { teacher_name: string; teacher_ip: string } } |
          { UpdateRequired: { current_version: string; required_version: string } } |
          { Updating: { progress: number } } |
          { Error: { message: string } };
}

interface ConnectedStatus {
  Connected: {
    teacher_name: string;
    teacher_ip: string;
  };
}

interface UpdatingStatus {
  Updating: {
    progress: number;
  };
}

interface ErrorStatus {
  Error: {
    message: string;
  };
}

const StudentTrayWindow: React.FC = () => {
  const [status, setStatus] = useState<AgentStatus | null>(null);
  const [studentName, setStudentName] = useState('Student');

  useEffect(() => {
    // Get initial status
    loadStatus();

    // Poll status every second
    const interval = setInterval(loadStatus, 1000);

    return () => clearInterval(interval);
  }, []);

  const loadStatus = async () => {
    try {
      const agentStatus = await invoke<AgentStatus>('get_agent_status');
      setStatus(agentStatus);

      const config = await invoke<{ port: number; student_name: string }>('get_agent_config');
      setStudentName(config.student_name);
    } catch (error) {
      console.error('Failed to load status:', error);
    }
  };

  const handleClose = async () => {
    const window = getCurrentWindow();
    await window.hide();
  };

  const handleQuit = async () => {
    const confirmed = confirm('Bạn có chắc chắn muốn thoát ứng dụng? Giáo viên sẽ không thể kết nối với bạn nữa.');
    if (!confirmed) return;
    
    try {
      await invoke('stop_student_agent');
      await invoke('quit_app');
    } catch (error) {
      console.error('Failed to quit:', error);
    }
  };

  const getStatusInfo = () => {
    if (!status) {
      return {
        icon: <Loader2 className="w-8 h-8 animate-spin text-slate-400" />,
        text: 'Đang tải...',
        color: 'text-slate-600',
        bgColor: 'bg-slate-50',
      };
    }

    if (typeof status === 'string') {
      switch (status) {
        case 'Stopped':
          return {
            icon: <WifiOff className="w-8 h-8 text-slate-400" />,
            text: 'Đã dừng',
            color: 'text-slate-600',
            bgColor: 'bg-slate-50',
          };
        case 'Starting':
          return {
            icon: <Loader2 className="w-8 h-8 animate-spin text-indigo-500" />,
            text: 'Đang khởi động...',
            color: 'text-indigo-600',
            bgColor: 'bg-indigo-50',
          };
        case 'WaitingForTeacher':
          return {
            icon: <Wifi className="w-8 h-8 text-amber-500 animate-pulse" />,
            text: 'Đang tìm giáo viên...',
            color: 'text-amber-600',
            bgColor: 'bg-amber-50',
          };
        case 'Authenticating':
          return {
            icon: <Loader2 className="w-8 h-8 animate-spin text-blue-500" />,
            text: 'Đang xác thực...',
            color: 'text-blue-600',
            bgColor: 'bg-blue-50',
          };
      }
    }

    if ('Connected' in status) {
      const connectedStatus = status as ConnectedStatus;
      return {
        icon: <CheckCircle2 className="w-8 h-8 text-emerald-500" />,
        text: `Đã kết nối: ${connectedStatus.Connected.teacher_name}`,
        color: 'text-emerald-600',
        bgColor: 'bg-emerald-50',
      };
    }

    if ('UpdateRequired' in status) {
      return {
        icon: <AlertCircle className="w-8 h-8 text-orange-500" />,
        text: 'Cần cập nhật',
        color: 'text-orange-600',
        bgColor: 'bg-orange-50',
      };
    }

    if ('Updating' in status) {
      const updatingStatus = status as UpdatingStatus;
      return {
        icon: <Loader2 className="w-8 h-8 animate-spin text-blue-500" />,
        text: `Đang cập nhật: ${Math.round(updatingStatus.Updating.progress * 100)}%`,
        color: 'text-blue-600',
        bgColor: 'bg-blue-50',
      };
    }

    if ('Error' in status) {
      const errorStatus = status as ErrorStatus;
      return {
        icon: <AlertCircle className="w-8 h-8 text-rose-500" />,
        text: `Lỗi: ${errorStatus.Error.message}`,
        color: 'text-rose-600',
        bgColor: 'bg-rose-50',
      };
    }

    return {
      icon: <Monitor className="w-8 h-8 text-slate-400" />,
      text: 'Không xác định',
      color: 'text-slate-600',
      bgColor: 'bg-slate-50',
    };
  };

  const statusInfo = getStatusInfo();

  return (
    <div className="w-full h-full bg-white flex flex-col">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 bg-slate-50 border-b border-slate-200">
        <div className="flex items-center gap-2">
          <Monitor className="w-5 h-5 text-indigo-600" />
          <h1 className="text-sm font-bold text-slate-800">Smartlab Student</h1>
        </div>
        <button
          onClick={handleClose}
          className="p-1 hover:bg-slate-200 rounded-lg transition-colors"
        >
          <X className="w-4 h-4 text-slate-500" />
        </button>
      </div>

      {/* Status */}
      <div className="flex-1 flex flex-col items-center justify-center p-6">
        <div className={`w-20 h-20 rounded-full ${statusInfo.bgColor} flex items-center justify-center mb-4`}>
          {statusInfo.icon}
        </div>
        <h2 className={`text-lg font-bold ${statusInfo.color} text-center mb-2`}>
          {statusInfo.text}
        </h2>
        <p className="text-sm text-slate-500 text-center">
          Máy: {studentName}
        </p>
      </div>

      {/* Footer */}
      <div className="px-4 py-3 bg-slate-50 border-t border-slate-200 space-y-2">
        <button
          onClick={handleQuit}
          className="w-full py-2 bg-rose-600 hover:bg-rose-700 text-white rounded-lg text-sm font-bold transition-colors"
        >
          Thoát ứng dụng hoàn toàn
        </button>
        <p className="text-xs text-slate-400 text-center">
          Ứng dụng đang chạy ngầm. Đóng cửa sổ này sẽ không thoát app.
        </p>
        <p className="text-xs text-amber-600 text-center font-medium">
          ⚠️ Chỉ thoát khi thực sự cần thiết
        </p>
      </div>
    </div>
  );
};

export default StudentTrayWindow;
