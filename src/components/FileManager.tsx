import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

interface FileInfo {
  name: string;
  path: string;
  is_dir: boolean;
  size: number;
  modified: number;
}

interface StudentConnection {
  id: string;
  ip: string;
  port: number;
  name: string | null;
}

type TransferStatus = 
  | 'Pending'
  | 'Connecting'
  | 'Transferring'
  | 'Completed'
  | 'Cancelled'
  | { Failed: { error: string } };

interface FileTransferProgress {
  job_id: string;
  file_name: string;
  file_size: number;
  transferred: number;
  progress: number;
  status: TransferStatus;
  student_id: string;
}

interface FileManagerProps {
  student: StudentConnection;
  onClose: () => void;
}

export function FileManager({ student, onClose }: FileManagerProps) {
  // Teacher (local) file browser state
  const [teacherPath, setTeacherPath] = useState<string>('');
  const [teacherFiles, setTeacherFiles] = useState<FileInfo[]>([]);
  const [teacherSelected, setTeacherSelected] = useState<Set<string>>(new Set());
  const [teacherLoading, setTeacherLoading] = useState(false);

  // Student (remote) file browser state
  const [studentPath, setStudentPath] = useState<string>('');
  const [studentFiles, setStudentFiles] = useState<FileInfo[]>([]);
  const [studentSelected, setStudentSelected] = useState<Set<string>>(new Set());
  const [studentLoading, setStudentLoading] = useState(false);
  const [studentError, setStudentError] = useState<string | null>(null);

  // Transfer state
  const [transfers, setTransfers] = useState<Record<string, FileTransferProgress>>({});
  const [error, setError] = useState<string | null>(null);

  // Initialize paths
  useEffect(() => {
    initializePaths();
  }, []);

  // Listen for transfer progress
  useEffect(() => {
    let unlisten: (() => void) | null = null;

    const setup = async () => {
      unlisten = await listen<FileTransferProgress>('file-transfer-progress', (event) => {
        const progress = event.payload;
        if (progress.student_id === student.id || progress.student_id === 'local') {
          setTransfers(prev => ({
            ...prev,
            [progress.job_id]: progress
          }));

          // Remove completed transfers after 3 seconds
          if (progress.status === 'Completed' || progress.status === 'Cancelled' ||
              (typeof progress.status === 'object' && 'Failed' in progress.status)) {
            setTimeout(() => {
              setTransfers(prev => {
                const newState = { ...prev };
                delete newState[progress.job_id];
                return newState;
              });
              // Refresh student files after transfer completes
              if (progress.status === 'Completed') {
                loadStudentFiles(studentPath);
              }
            }, 3000);
          }
        }
      });
    };

    setup();
    return () => { if (unlisten) unlisten(); };
  }, [student.id, studentPath]);

  const initializePaths = async () => {
    try {
      // Get teacher's home directory
      const homePath = await invoke<string>('get_home_directory');
      setTeacherPath(homePath);
      loadTeacherFiles(homePath);

      // Get student's Downloads directory (default)
      loadStudentFiles('');
    } catch (e) {
      setError(`Lỗi khởi tạo: ${e}`);
    }
  };

  const loadTeacherFiles = async (path: string) => {
    setTeacherLoading(true);
    try {
      const files = await invoke<FileInfo[]>('list_directory', { path });
      setTeacherFiles(files);
      setTeacherPath(path);
      setTeacherSelected(new Set());
    } catch (e) {
      setError(`Lỗi đọc thư mục: ${e}`);
    } finally {
      setTeacherLoading(false);
    }
  };

  const loadStudentFiles = async (path: string) => {
    setStudentLoading(true);
    setStudentError(null);
    try {
      // Request student's directory listing via WebSocket
      const files = await invoke<FileInfo[]>('get_student_directory', { 
        studentId: student.id,
        path: path || '' 
      });
      setStudentFiles(files);
      setStudentPath(path);
      setStudentSelected(new Set());
    } catch (e) {
      setStudentError(`${e}`);
      setStudentFiles([]);
    } finally {
      setStudentLoading(false);
    }
  };

  const navigateTeacher = (file: FileInfo) => {
    if (file.is_dir) {
      loadTeacherFiles(file.path);
    }
  };

  const navigateTeacherUp = () => {
    const parentPath = teacherPath.split(/[/\\]/).slice(0, -1).join('/') || '/';
    loadTeacherFiles(parentPath);
  };

  const navigateStudent = (file: FileInfo) => {
    if (file.is_dir) {
      loadStudentFiles(file.path);
    }
  };

  const navigateStudentUp = () => {
    const parentPath = studentPath.split(/[/\\]/).slice(0, -1).join('/') || '';
    loadStudentFiles(parentPath);
  };

  const toggleTeacherSelect = (file: FileInfo, e: React.MouseEvent) => {
    e.stopPropagation();
    setTeacherSelected(prev => {
      const newSet = new Set(prev);
      if (newSet.has(file.path)) {
        newSet.delete(file.path);
      } else {
        newSet.add(file.path);
      }
      return newSet;
    });
  };

  const toggleStudentSelect = (file: FileInfo, e: React.MouseEvent) => {
    e.stopPropagation();
    setStudentSelected(prev => {
      const newSet = new Set(prev);
      if (newSet.has(file.path)) {
        newSet.delete(file.path);
      } else {
        newSet.add(file.path);
      }
      return newSet;
    });
  };

  // Send selected files from teacher to student
  const sendToStudent = async () => {
    if (teacherSelected.size === 0) return;

    for (const filePath of teacherSelected) {
      try {
        await invoke<string>('send_file_to_student', {
          studentId: student.id,
          filePath: filePath,
        });
      } catch (e) {
        setError(`Lỗi gửi file: ${e}`);
      }
    }
    setTeacherSelected(new Set());
  };

  // Request files from student (not implemented yet - would need student-side upload)
  const receiveFromStudent = async () => {
    if (studentSelected.size === 0) return;
    setError('Tính năng nhận file từ học sinh chưa được hỗ trợ');
  };

  const formatSize = (bytes: number): string => {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
  };

  const formatDate = (timestamp: number): string => {
    if (!timestamp) return '';
    return new Date(timestamp * 1000).toLocaleDateString('vi-VN');
  };

  const getTransferStatusText = (status: TransferStatus): string => {
    if (typeof status === 'string') {
      switch (status) {
        case 'Pending': return 'Đang chờ...';
        case 'Connecting': return 'Kết nối...';
        case 'Transferring': return 'Đang truyền...';
        case 'Completed': return '✅ Xong';
        case 'Cancelled': return '❌ Hủy';
        default: return status;
      }
    }
    if ('Failed' in status) return `❌ ${status.Failed.error}`;
    return '';
  };

  const activeTransfers = Object.values(transfers);

  return (
    <div className="file-manager-overlay">
      <div className="file-manager-modal">
        {/* Header */}
        <div className="fm-header">
          <h2>📂 Quản lý File - {student.name || student.ip}</h2>
          <button onClick={onClose} className="close-btn">✕</button>
        </div>

        {/* Error message */}
        {error && (
          <div className="fm-error">
            {error}
            <button onClick={() => setError(null)}>✕</button>
          </div>
        )}

        {/* Main content - Two panels */}
        <div className="fm-content">
          {/* Teacher (Local) Panel */}
          <div className="fm-panel teacher-panel">
            <div className="fm-panel-header">
              <span className="panel-title">🖥️ Máy Giáo viên</span>
              <div className="fm-path">
                <button onClick={navigateTeacherUp} className="btn-icon" title="Lên thư mục cha">⬆️</button>
                <input 
                  type="text" 
                  value={teacherPath} 
                  onChange={(e) => loadTeacherFiles(e.target.value)}
                  className="path-input"
                />
                <button onClick={() => loadTeacherFiles(teacherPath)} className="btn-icon" title="Làm mới">🔄</button>
              </div>
            </div>
            <div className="fm-file-list">
              {teacherLoading ? (
                <div className="fm-loading">Đang tải...</div>
              ) : (
                teacherFiles.map((file) => (
                  <div
                    key={file.path}
                    className={`fm-file-item ${teacherSelected.has(file.path) ? 'selected' : ''}`}
                    onClick={(e) => toggleTeacherSelect(file, e)}
                    onDoubleClick={() => navigateTeacher(file)}
                  >
                    <span className="file-icon">{file.is_dir ? '📁' : '📄'}</span>
                    <span className="file-name">{file.name}</span>
                    <span className="file-size">{file.is_dir ? '' : formatSize(file.size)}</span>
                    <span className="file-date">{formatDate(file.modified)}</span>
                  </div>
                ))
              )}
            </div>
            <div className="fm-panel-footer">
              {teacherSelected.size} đã chọn
            </div>
          </div>

          {/* Transfer buttons */}
          <div className="fm-transfer-buttons">
            <button 
              onClick={sendToStudent}
              disabled={teacherSelected.size === 0}
              className="btn transfer-btn"
              title="Gửi file đã chọn sang máy học sinh"
            >
              ➡️
              <span>Gửi</span>
            </button>
            <button 
              onClick={receiveFromStudent}
              disabled={studentSelected.size === 0}
              className="btn transfer-btn"
              title="Nhận file từ máy học sinh (chưa hỗ trợ)"
            >
              ⬅️
              <span>Nhận</span>
            </button>
          </div>

          {/* Student (Remote) Panel */}
          <div className="fm-panel student-panel">
            <div className="fm-panel-header">
              <span className="panel-title">👨‍🎓 Máy Học sinh</span>
              <div className="fm-path">
                <button onClick={navigateStudentUp} className="btn-icon" title="Lên thư mục cha">⬆️</button>
                <input 
                  type="text" 
                  value={studentPath} 
                  onChange={(e) => loadStudentFiles(e.target.value)}
                  className="path-input"
                  placeholder="Downloads"
                />
                <button onClick={() => loadStudentFiles(studentPath)} className="btn-icon" title="Làm mới">🔄</button>
              </div>
            </div>
            <div className="fm-file-list">
              {studentLoading ? (
                <div className="fm-loading">Đang tải...</div>
              ) : studentError ? (
                <div className="fm-error-inline">{studentError}</div>
              ) : studentFiles.length === 0 ? (
                <div className="fm-empty">Thư mục trống hoặc chưa kết nối</div>
              ) : (
                studentFiles.map((file) => (
                  <div
                    key={file.path}
                    className={`fm-file-item ${studentSelected.has(file.path) ? 'selected' : ''}`}
                    onClick={(e) => toggleStudentSelect(file, e)}
                    onDoubleClick={() => navigateStudent(file)}
                  >
                    <span className="file-icon">{file.is_dir ? '📁' : '📄'}</span>
                    <span className="file-name">{file.name}</span>
                    <span className="file-size">{file.is_dir ? '' : formatSize(file.size)}</span>
                    <span className="file-date">{formatDate(file.modified)}</span>
                  </div>
                ))
              )}
            </div>
            <div className="fm-panel-footer">
              {studentSelected.size} đã chọn
            </div>
          </div>
        </div>

        {/* Transfer progress */}
        {activeTransfers.length > 0 && (
          <div className="fm-transfers">
            <h4>📤 Đang truyền</h4>
            {activeTransfers.map((t) => (
              <div key={t.job_id} className="fm-transfer-item">
                <span className="transfer-name">{t.file_name}</span>
                <div className="transfer-progress-bar">
                  <div className="progress-fill" style={{ width: `${t.progress}%` }} />
                </div>
                <span className="transfer-status">{getTransferStatusText(t.status)}</span>
                <span className="transfer-percent">{t.progress.toFixed(0)}%</span>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

export default FileManager;
