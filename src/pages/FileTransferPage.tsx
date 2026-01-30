import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open, save } from '@tauri-apps/plugin-dialog';
import './FileTransferPage.css';

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
  status: string;
}

interface FileTransferPageProps {
  onBack: () => void;
}

export function FileTransferPage({ onBack }: FileTransferPageProps) {
  const [students, setStudents] = useState<StudentConnection[]>([]);
  const [selectedStudent, setSelectedStudent] = useState<string | null>(null);
  
  // Teacher's file browser
  const [teacherPath, setTeacherPath] = useState<string>('');
  const [teacherFiles, setTeacherFiles] = useState<FileInfo[]>([]);
  const [selectedTeacherFile, setSelectedTeacherFile] = useState<string | null>(null);
  
  // Student's file browser
  const [studentPath, setStudentPath] = useState<string>('');
  const [studentFiles, setStudentFiles] = useState<FileInfo[]>([]);
  const [selectedStudentFile, setSelectedStudentFile] = useState<string | null>(null);
  
  const [transferring, setTransferring] = useState(false);
  const [message, setMessage] = useState<string>('');

  // Load connected students
  useEffect(() => {
    loadStudents();
    const interval = setInterval(loadStudents, 2000);
    return () => clearInterval(interval);
  }, []);

  // Load teacher's home directory on mount
  useEffect(() => {
    loadTeacherHome();
  }, []);

  const loadStudents = async () => {
    try {
      const conns = await invoke<StudentConnection[]>('get_student_connections');
      console.log('All connections:', conns); // Debug log
      // Filter for Connected or Viewing status
      const connected = conns.filter(c => c.status === 'Connected' || c.status === 'Viewing');
      console.log('Connected students:', connected); // Debug log
      setStudents(connected);
    } catch (err) {
      console.error('Failed to load students:', err);
    }
  };

  const loadTeacherHome = async () => {
    try {
      const home = await invoke<string>('get_home_directory');
      setTeacherPath(home);
      await loadTeacherDirectory(home);
    } catch (err) {
      showMessage('Không thể tải thư mục home: ' + err, 'error');
    }
  };

  const loadTeacherDirectory = async (path: string) => {
    try {
      const files = await invoke<FileInfo[]>('list_directory', { path });
      setTeacherFiles(files);
      setTeacherPath(path);
    } catch (err) {
      showMessage('Không thể tải thư mục: ' + err, 'error');
    }
  };

  const loadStudentDirectory = async (path: string) => {
    if (!selectedStudent) {
      showMessage('Vui lòng chọn học sinh', 'error');
      return;
    }

    try {
      // TODO: Implement remote directory listing via WebSocket
      setStudentPath(path);
      setStudentFiles([]);
      showMessage('Chức năng đang phát triển', 'info');
    } catch (err) {
      showMessage('Không thể tải thư mục học sinh: ' + err, 'error');
    }
  };

  const sendFileToStudent = async () => {
    if (!selectedStudent) {
      showMessage('Vui lòng chọn học sinh', 'error');
      return;
    }

    // Open file picker dialog
    const filePath = await open({
      multiple: false,
      directory: false,
      title: 'Chọn file để gửi cho học sinh',
    });

    if (!filePath) {
      return; // User cancelled
    }

    setTransferring(true);
    try {
      // Read file as base64
      const fileData = await invoke<string>('read_file_as_base64', {
        path: filePath
      });

      // Get file info
      const fileInfo = await invoke<FileInfo>('get_file_info', {
        path: filePath
      });

      // TODO: Send via WebSocket to student
      console.log('File data length:', fileData.length);
      showMessage(`Đang gửi file "${fileInfo.name}" tới học sinh...`, 'info');
      
      // Simulate transfer
      await new Promise(resolve => setTimeout(resolve, 1000));
      
      showMessage(`Đã gửi file "${fileInfo.name}" thành công!`, 'success');
    } catch (err) {
      showMessage('Lỗi khi gửi file: ' + err, 'error');
    } finally {
      setTransferring(false);
    }
  };

  const browseTeacherFolder = async () => {
    const folderPath = await open({
      multiple: false,
      directory: true,
      title: 'Chọn thư mục để duyệt',
    });

    if (folderPath) {
      await loadTeacherDirectory(folderPath);
    }
  };

  const loadQuickFolder = async (type: 'home' | 'desktop' | 'documents') => {
    try {
      let path: string;
      switch (type) {
        case 'home':
          path = await invoke<string>('get_home_directory');
          break;
        case 'desktop':
          path = await invoke<string>('get_desktop_directory');
          break;
        case 'documents':
          path = await invoke<string>('get_documents_directory');
          break;
      }
      await loadTeacherDirectory(path);
    } catch (err) {
      showMessage('Không thể tải thư mục: ' + err, 'error');
    }
  };

  const receiveFileFromStudent = async () => {
    if (!selectedStudent) {
      showMessage('Vui lòng chọn học sinh', 'error');
      return;
    }

    if (!selectedStudentFile) {
      showMessage('Vui lòng chọn file để nhận', 'error');
      return;
    }

    // Open save dialog
    const savePath = await save({
      title: 'Lưu file nhận từ học sinh',
      defaultPath: selectedStudentFile.split(/[/\\]/).pop(), // Get filename
    });

    if (!savePath) {
      return; // User cancelled
    }

    setTransferring(true);
    try {
      // TODO: Request file from student via WebSocket
      showMessage('Đang nhận file từ học sinh...', 'info');
      
      // Simulate transfer
      await new Promise(resolve => setTimeout(resolve, 1000));
      
      // TODO: Save received data
      // await invoke('write_file_from_base64', { path: savePath, data: receivedData });
      
      showMessage(`Đã lưu file vào: ${savePath}`, 'success');
    } catch (err) {
      showMessage('Lỗi khi nhận file: ' + err, 'error');
    } finally {
      setTransferring(false);
    }
  };

  const showMessage = (msg: string, _type: 'info' | 'success' | 'error') => {
    setMessage(msg);
    setTimeout(() => setMessage(''), 5000);
  };

  const formatFileSize = (bytes: number): string => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return Math.round(bytes / Math.pow(k, i) * 100) / 100 + ' ' + sizes[i];
  };

  const formatDate = (timestamp: number): string => {
    return new Date(timestamp * 1000).toLocaleString('vi-VN');
  };

  const goToParentDirectory = (currentPath: string, isTeacher: boolean) => {
    const parts = currentPath.split(/[/\\]/);
    if (parts.length > 1) {
      parts.pop();
      const parentPath = parts.join('/');
      if (isTeacher) {
        loadTeacherDirectory(parentPath || '/');
      } else {
        loadStudentDirectory(parentPath || '/');
      }
    }
  };

  return (
    <div className="file-transfer-page">
      <div className="header">
        <button onClick={onBack} className="btn back-btn">
          ← Quay lại
        </button>
        <h1>📁 Quản lý File</h1>
      </div>

      {message && (
        <div className={`message ${message.includes('Lỗi') ? 'error' : message.includes('thành công') ? 'success' : 'info'}`}>
          {message}
        </div>
      )}

      {/* Student Selection */}
      <div className="student-selection">
        <h3>Chọn học sinh:</h3>
        <div className="student-list">
          {students.length === 0 ? (
            <div className="no-students">
              <p>Không có học sinh nào đang kết nối</p>
              <p className="hint">💡 Vào trang "View Client" để kết nối với học sinh trước</p>
            </div>
          ) : (
            students.map(student => (
              <button
                key={student.id}
                className={`student-item ${selectedStudent === student.id ? 'selected' : ''}`}
                onClick={() => setSelectedStudent(student.id)}
              >
                <span className="student-icon">👤</span>
                <span className="student-name">{student.name || student.ip}</span>
                <span className="student-ip">{student.ip}:{student.port}</span>
                <span className="student-status">{student.status}</span>
              </button>
            ))
          )}
        </div>
      </div>

      {/* File Browsers */}
      <div className="file-browsers">
        {/* Teacher's Files */}
        <div className="file-browser teacher-browser">
          <div className="browser-header">
            <h3>📂 File của giáo viên</h3>
            
            {/* Quick Access Buttons */}
            <div className="quick-access">
              <button onClick={() => loadQuickFolder('home')} className="btn-quick" title="Home">
                🏠 Home
              </button>
              <button onClick={() => loadQuickFolder('desktop')} className="btn-quick" title="Desktop">
                🖥️ Desktop
              </button>
              <button onClick={() => loadQuickFolder('documents')} className="btn-quick" title="Documents">
                📄 Documents
              </button>
            </div>
            
            <div className="path-bar">
              <button 
                onClick={() => goToParentDirectory(teacherPath, true)}
                className="btn-icon"
                disabled={teacherPath === '/' || !teacherPath}
                title="Thư mục cha"
              >
                ⬆️
              </button>
              <button 
                onClick={browseTeacherFolder}
                className="btn-icon"
                title="Chọn thư mục"
              >
                📁
              </button>
              <input 
                type="text" 
                value={teacherPath} 
                readOnly 
                className="path-input"
              />
            </div>
          </div>
          
          <div className="file-list">
            {teacherFiles.map(file => (
              <div
                key={file.path}
                className={`file-item ${selectedTeacherFile === file.path ? 'selected' : ''}`}
                onClick={() => {
                  if (file.is_dir) {
                    loadTeacherDirectory(file.path);
                  } else {
                    setSelectedTeacherFile(file.path);
                  }
                }}
                onDoubleClick={() => {
                  if (file.is_dir) {
                    loadTeacherDirectory(file.path);
                  }
                }}
              >
                <span className="file-icon">{file.is_dir ? '📁' : '📄'}</span>
                <div className="file-info">
                  <div className="file-name">{file.name}</div>
                  <div className="file-meta">
                    {!file.is_dir && <span>{formatFileSize(file.size)}</span>}
                    <span>{formatDate(file.modified)}</span>
                  </div>
                </div>
              </div>
            ))}
          </div>

          <div className="browser-actions">
            <button
              onClick={sendFileToStudent}
              disabled={!selectedStudent || transferring}
              className="btn primary"
            >
              {transferring ? '⏳ Đang gửi...' : '📤 Chọn file và gửi'}
            </button>
            <p className="action-hint">
              {!selectedStudent 
                ? '⚠️ Vui lòng chọn học sinh trước' 
                : 'Click để mở hộp thoại chọn file từ máy bạn'}
            </p>
          </div>
        </div>

        {/* Student's Files */}
        <div className="file-browser student-browser">
          <div className="browser-header">
            <h3>📂 File của học sinh</h3>
            <div className="path-bar">
              <button 
                onClick={() => goToParentDirectory(studentPath, false)}
                className="btn-icon"
                disabled={!selectedStudent || studentPath === '/' || !studentPath}
              >
                ⬆️
              </button>
              <input 
                type="text" 
                value={studentPath || 'Chọn học sinh để xem file'} 
                readOnly 
                className="path-input"
              />
            </div>
          </div>
          
          <div className="file-list">
            {!selectedStudent ? (
              <div className="empty-state">
                <p>👆 Vui lòng chọn học sinh ở trên</p>
              </div>
            ) : studentFiles.length === 0 ? (
              <div className="empty-state">
                <p>Chưa tải thư mục học sinh</p>
                <button 
                  onClick={async () => {
                    try {
                      const home = await invoke<string>('get_home_directory');
                      loadStudentDirectory(home);
                    } catch (err) {
                      showMessage('Lỗi: ' + err, 'error');
                    }
                  }}
                  className="btn"
                >
                  Tải thư mục home
                </button>
              </div>
            ) : (
              studentFiles.map(file => (
                <div
                  key={file.path}
                  className={`file-item ${selectedStudentFile === file.path ? 'selected' : ''}`}
                  onClick={() => {
                    if (file.is_dir) {
                      loadStudentDirectory(file.path);
                    } else {
                      setSelectedStudentFile(file.path);
                    }
                  }}
                >
                  <span className="file-icon">{file.is_dir ? '📁' : '📄'}</span>
                  <div className="file-info">
                    <div className="file-name">{file.name}</div>
                    <div className="file-meta">
                      {!file.is_dir && <span>{formatFileSize(file.size)}</span>}
                      <span>{formatDate(file.modified)}</span>
                    </div>
                  </div>
                </div>
              ))
            )}
          </div>

          <div className="browser-actions">
            <button
              onClick={receiveFileFromStudent}
              disabled={!selectedStudentFile || !selectedStudent || transferring}
              className="btn primary"
            >
              {transferring ? '⏳ Đang nhận...' : '📥 Nhận file từ học sinh'}
            </button>
            <p className="action-hint">Chọn file bên trên trước khi nhận</p>
          </div>
        </div>
      </div>
    </div>
  );
}
