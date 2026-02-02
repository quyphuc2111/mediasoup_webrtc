import React, { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { 
  FileText, Upload, Download, Search, 
  Trash2, FolderOpen, Folder,
  Server, Copy, ExternalLink, RefreshCw,
  CheckCircle, Loader2
} from 'lucide-react';
import { UserAccount as User, UserRole } from '../types';
import StudentDocuments from '../components/StudentDocuments';

interface Document {
  id: string;
  name: string;
  size: number;
  mime_type: string;
  uploaded_at: number;
  description?: string;
  category?: string;
}

const DocumentManager: React.FC<{ user: User; teacherIp?: string }> = ({ user, teacherIp }) => {
  const [activeFolder, setActiveFolder] = useState('Tất cả tài liệu');
  const [documents, setDocuments] = useState<Document[]>([]);
  const [isServerRunning, setIsServerRunning] = useState(false);
  const [serverUrl, setServerUrl] = useState('');
  const [uploadProgress, setUploadProgress] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState('');
  const [copySuccess, setCopySuccess] = useState(false);

  const isTeacher = user.role === UserRole.TEACHER || user.role === UserRole.ADMIN;

  // ========== STUDENT VIEW ==========
  if (!isTeacher) {
    // Auto-generate server URL from teacher IP
    const studentServerUrl = teacherIp ? `http://${teacherIp}:8765` : '';
    
    return (
      <div className="space-y-6">
        <div className="flex flex-col md:flex-row md:items-center justify-between gap-4">
          <div>
            <h1 className="text-2xl font-bold text-slate-800">📚 Tài liệu học tập</h1>
            <p className="text-slate-500">Tải tài liệu từ giáo viên</p>
          </div>
        </div>
        
        <StudentDocuments serverUrl={studentServerUrl} />
      </div>
    );
  }

  // ========== TEACHER VIEW ==========
  
  // Load documents and server status
  const loadData = useCallback(async () => {
    try {
      const docs = await invoke<Document[]>('list_documents');
      setDocuments(docs);
      
      const [running, , url] = await invoke<[boolean, number, string]>('get_document_server_status');
      setIsServerRunning(running);
      setServerUrl(url);
    } catch (err) {
      console.error('Failed to load data:', err);
    }
  }, []);

  useEffect(() => {
    loadData();
    const interval = setInterval(loadData, 5000);
    return () => clearInterval(interval);
  }, [loadData]);

  // Upload file
  const handleUpload = async () => {
    try {
      const selected = await open({
        multiple: true,
        title: 'Chọn tài liệu để tải lên',
      });
      
      if (!selected) return;
      
      const files = Array.isArray(selected) ? selected : [selected];
      
      for (const filePath of files) {
        setUploadProgress(`Đang tải: ${filePath.split('/').pop()}`);
        await invoke('upload_document_from_path', {
          filePath,
          description: null,
          category: activeFolder !== 'Tất cả tài liệu' ? activeFolder : null,
        });
      }
      
      setUploadProgress(null);
      loadData();
    } catch (err) {
      console.error('Upload failed:', err);
      setUploadProgress(null);
      alert('Tải lên thất bại: ' + err);
    }
  };

  // Delete document
  const handleDelete = async (id: string) => {
    if (!confirm('Bạn có chắc muốn xóa tài liệu này?')) return;
    
    try {
      await invoke('delete_document', { id });
      loadData();
    } catch (err) {
      console.error('Delete failed:', err);
      alert('Xóa thất bại: ' + err);
    }
  };

  // Download document
  const handleDownload = async (doc: Document, askFolder = false) => {
    try {
      let customFolder: string | undefined;
      
      // Ask for folder if requested
      if (askFolder) {
        const selected = await open({
          directory: true,
          multiple: false,
          title: 'Chọn thư mục lưu tài liệu',
        });
        
        if (!selected) return; // User cancelled
        customFolder = Array.isArray(selected) ? selected[0] : selected;
      }
      
      const url = `${serverUrl}/download/${doc.id}`;
      const filePath = await invoke<string>('download_document_to_downloads', {
        url,
        filename: doc.name,
        customFolder,
      });
      console.log('Downloaded to:', filePath);
      alert(`Đã tải xuống: ${filePath}`);
    } catch (err) {
      console.error('Download failed:', err);
      alert('Tải xuống thất bại: ' + err);
    }
  };

  // Copy URL to clipboard
  const handleCopyUrl = async () => {
    try {
      await navigator.clipboard.writeText(serverUrl);
      setCopySuccess(true);
      setTimeout(() => setCopySuccess(false), 2000);
    } catch (err) {
      console.error('Copy failed:', err);
    }
  };

  // Format file size
  const formatSize = (bytes: number) => {
    if (bytes >= 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
    if (bytes >= 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    if (bytes >= 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${bytes} B`;
  };

  // Format date
  const formatDate = (timestamp: number) => {
    return new Date(timestamp * 1000).toLocaleDateString('vi-VN', {
      day: '2-digit',
      month: '2-digit',
      year: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
    });
  };

  // Get file icon
  const getFileIcon = (mimeType: string) => {
    if (mimeType.includes('pdf')) return '📕';
    if (mimeType.includes('word') || mimeType.includes('document')) return '📘';
    if (mimeType.includes('excel') || mimeType.includes('spreadsheet')) return '📗';
    if (mimeType.includes('powerpoint') || mimeType.includes('presentation')) return '📙';
    if (mimeType.includes('image')) return '🖼️';
    if (mimeType.includes('video')) return '🎬';
    if (mimeType.includes('audio')) return '🎵';
    if (mimeType.includes('zip') || mimeType.includes('rar') || mimeType.includes('7z')) return '📦';
    return '📄';
  };

  // Filter documents
  const filteredDocs = documents.filter(doc => 
    doc.name.toLowerCase().includes(searchQuery.toLowerCase())
  );

  // Categories
  const categories = ['Tất cả tài liệu', 'Giáo trình', 'Bài tập', 'Tài liệu tham khảo', 'Source code'];

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex flex-col md:flex-row md:items-center justify-between gap-4">
        <div>
          <h1 className="text-2xl font-bold text-slate-800">Phân phối tài liệu</h1>
          <p className="text-slate-500">Chia sẻ tài liệu cho học sinh qua HTTP server</p>
        </div>
        <div className="flex gap-2">
          <button 
            onClick={handleUpload}
            disabled={!!uploadProgress}
            className="flex items-center gap-2 px-6 py-3 bg-indigo-600 text-white rounded-2xl font-bold hover:bg-indigo-700 transition shadow-lg shadow-indigo-600/20 disabled:opacity-50"
          >
            {uploadProgress ? (
              <Loader2 className="w-5 h-5 animate-spin" />
            ) : (
              <Upload className="w-5 h-5" />
            )}
            {uploadProgress || 'Tải tài liệu lên'}
          </button>
        </div>
      </div>

      {/* Server Status Panel */}
      <div className="bg-white rounded-3xl border border-slate-200 p-6 shadow-sm">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-4">
            <div className={`p-3 rounded-2xl ${isServerRunning ? 'bg-emerald-100' : 'bg-amber-100'}`}>
              <Server className={`w-6 h-6 ${isServerRunning ? 'text-emerald-600' : 'text-amber-600'}`} />
            </div>
            <div>
              <h3 className="font-bold text-slate-800">HTTP Server</h3>
              <p className="text-sm text-slate-500">
                {isServerRunning ? '✅ Đang chạy - Học sinh có thể truy cập' : '⏳ Đang khởi động...'}
              </p>
            </div>
          </div>
        </div>
        
        {/* Server URL */}
        {isServerRunning && serverUrl && (
          <div className="mt-4 p-4 bg-indigo-50 rounded-2xl">
            <p className="text-sm text-indigo-600 font-medium mb-2">
              📢 Chia sẻ link này cho học sinh để tải tài liệu:
            </p>
            <div className="flex items-center gap-2">
              <code className="flex-1 px-4 py-2 bg-white rounded-xl text-indigo-700 font-mono text-sm border border-indigo-200">
                {serverUrl}
              </code>
              <button
                onClick={handleCopyUrl}
                className={`p-2 rounded-xl transition ${
                  copySuccess 
                    ? 'bg-emerald-100 text-emerald-600' 
                    : 'bg-indigo-100 text-indigo-600 hover:bg-indigo-200'
                }`}
                title="Sao chép link"
              >
                {copySuccess ? <CheckCircle className="w-5 h-5" /> : <Copy className="w-5 h-5" />}
              </button>
              <a
                href={serverUrl}
                target="_blank"
                rel="noopener noreferrer"
                className="p-2 bg-indigo-100 text-indigo-600 rounded-xl hover:bg-indigo-200 transition"
                title="Mở trong trình duyệt"
              >
                <ExternalLink className="w-5 h-5" />
              </a>
            </div>
          </div>
        )}
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-4 gap-6">
        {/* Sidebar Folders */}
        <div className="space-y-2">
          {categories.map((f) => (
            <button 
              key={f}
              onClick={() => setActiveFolder(f)}
              className={`
                w-full flex items-center justify-between px-4 py-3 rounded-2xl transition-all
                ${activeFolder === f ? 'bg-white shadow-sm border border-slate-200 text-indigo-600 font-bold' : 'text-slate-500 hover:bg-slate-100'}
              `}
            >
              <div className="flex items-center gap-3">
                <FolderOpen className={`w-4 h-4 ${activeFolder === f ? 'text-indigo-500' : 'text-slate-400'}`} />
                <span className="text-sm">{f}</span>
              </div>
              {f === 'Tất cả tài liệu' && (
                <span className="bg-slate-100 text-slate-500 text-[10px] px-2 py-0.5 rounded-full font-bold">
                  {documents.length}
                </span>
              )}
            </button>
          ))}
          
          {/* Stats */}
          <div className="mt-6 p-4 bg-gradient-to-br from-indigo-50 to-purple-50 rounded-2xl">
            <h4 className="font-bold text-slate-700 mb-3">Thống kê</h4>
            <div className="space-y-2 text-sm">
              <div className="flex justify-between">
                <span className="text-slate-500">Tổng tài liệu:</span>
                <span className="font-bold text-slate-700">{documents.length}</span>
              </div>
              <div className="flex justify-between">
                <span className="text-slate-500">Tổng dung lượng:</span>
                <span className="font-bold text-slate-700">
                  {formatSize(documents.reduce((sum, d) => sum + d.size, 0))}
                </span>
              </div>
            </div>
          </div>
        </div>

        {/* File List */}
        <div className="lg:col-span-3 bg-white rounded-3xl border border-slate-200 overflow-hidden shadow-sm">
          <div className="p-4 border-b border-slate-100 flex items-center justify-between">
            <div className="relative w-64">
              <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-slate-400" />
              <input 
                type="text" 
                placeholder="Tìm tài liệu..." 
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                className="w-full pl-10 pr-4 py-2 bg-slate-50 border-none rounded-xl text-sm outline-none"
              />
            </div>
            <button 
              onClick={loadData}
              className="p-2 hover:bg-slate-50 rounded-xl text-slate-400 transition"
              title="Làm mới"
            >
              <RefreshCw className="w-5 h-5" />
            </button>
          </div>

          <div className="grid grid-cols-1 divide-y divide-slate-100">
            {filteredDocs.length === 0 ? (
              <div className="p-12 text-center text-slate-400">
                <FileText className="w-12 h-12 mx-auto mb-4 opacity-20" />
                <p className="text-sm">
                  {documents.length === 0 
                    ? 'Chưa có tài liệu nào. Nhấn "Tải tài liệu lên" để bắt đầu.'
                    : 'Không tìm thấy tài liệu phù hợp.'}
                </p>
              </div>
            ) : (
              filteredDocs.map((doc) => (
                <div key={doc.id} className="p-4 flex items-center gap-4 hover:bg-slate-50/50 transition group">
                  <div className="p-3 bg-indigo-50 rounded-2xl text-2xl group-hover:scale-110 transition">
                    {getFileIcon(doc.mime_type)}
                  </div>
                  <div className="flex-1 min-w-0">
                    <h4 className="font-bold text-slate-800 text-sm truncate">{doc.name}</h4>
                    <p className="text-[10px] text-slate-400 mt-0.5 uppercase tracking-wider font-bold">
                      {formatSize(doc.size)} • {doc.mime_type.split('/')[1]?.toUpperCase() || 'FILE'} • {formatDate(doc.uploaded_at)}
                    </p>
                  </div>
                  <div className="flex items-center gap-2 opacity-0 group-hover:opacity-100 transition">
                    {isServerRunning && (
                      <>
                        <button
                          onClick={() => handleDownload(doc, false)}
                          className="flex items-center gap-1 px-3 py-1.5 bg-indigo-100 text-indigo-600 rounded-lg hover:bg-indigo-200 transition text-sm font-medium"
                          title="Tải về Downloads"
                        >
                          <Download className="w-4 h-4" />
                          Tải xuống
                        </button>
                        <button
                          onClick={() => handleDownload(doc, true)}
                          className="p-2 text-slate-400 hover:text-indigo-600 transition"
                          title="Chọn thư mục"
                        >
                          <Folder className="w-4 h-4" />
                        </button>
                      </>
                    )}
                    <button 
                      onClick={() => handleDelete(doc.id)}
                      className="p-2 text-slate-400 hover:text-rose-600 transition" 
                      title="Xóa"
                    >
                      <Trash2 className="w-4 h-4" />
                    </button>
                  </div>
                </div>
              ))
            )}
          </div>
        </div>
      </div>
    </div>
  );
};

export default DocumentManager;
