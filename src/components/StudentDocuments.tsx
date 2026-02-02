import React, { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { 
  FileText, Download, Search, RefreshCw,
  Loader2, Server, AlertCircle, Folder
} from 'lucide-react';

interface Document {
  id: string;
  name: string;
  size: number;
  mime_type: string;
  uploaded_at: number;
  description?: string;
  category?: string;
}

interface StudentDocumentsProps {
  serverUrl: string;
}

const StudentDocuments: React.FC<StudentDocumentsProps> = ({ serverUrl }) => {
  const [documents, setDocuments] = useState<Document[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState('');

  // Load documents from server
  const loadDocuments = useCallback(async () => {
    if (!serverUrl) {
      setError('Chưa có địa chỉ server');
      return;
    }

    setIsLoading(true);
    setError(null);

    try {
      const response = await fetch(`${serverUrl}/api/documents`);
      if (!response.ok) {
        throw new Error(`HTTP ${response.status}`);
      }
      const docs = await response.json();
      setDocuments(docs);
    } catch (err) {
      console.error('Failed to load documents:', err);
      setError('Không thể kết nối đến server tài liệu');
    } finally {
      setIsLoading(false);
    }
  }, [serverUrl]);

  useEffect(() => {
    if (serverUrl) {
      loadDocuments();
    }
  }, [serverUrl, loadDocuments]);

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

  if (!serverUrl) {
    return (
      <div className="flex flex-col items-center justify-center p-12 text-slate-400">
        <Server className="w-16 h-16 mb-4 opacity-30" />
        <p className="text-lg font-medium">Chưa kết nối server tài liệu</p>
        <p className="text-sm mt-2">Vui lòng nhập địa chỉ server từ giáo viên</p>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div className="p-2 bg-indigo-100 rounded-xl">
            <FileText className="w-5 h-5 text-indigo-600" />
          </div>
          <div>
            <h3 className="font-bold text-slate-800">Tài liệu từ giáo viên</h3>
            <p className="text-xs text-slate-500">{documents.length} tài liệu</p>
          </div>
        </div>
        <button
          onClick={loadDocuments}
          disabled={isLoading}
          className="p-2 hover:bg-slate-100 rounded-xl transition"
          title="Làm mới"
        >
          <RefreshCw className={`w-5 h-5 text-slate-400 ${isLoading ? 'animate-spin' : ''}`} />
        </button>
      </div>

      {/* Search */}
      <div className="relative">
        <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-slate-400" />
        <input 
          type="text" 
          placeholder="Tìm tài liệu..." 
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          className="w-full pl-10 pr-4 py-2 bg-slate-50 border border-slate-200 rounded-xl text-sm outline-none focus:border-indigo-300"
        />
      </div>

      {/* Error */}
      {error && (
        <div className="flex items-center gap-2 p-3 bg-rose-50 text-rose-600 rounded-xl text-sm">
          <AlertCircle className="w-4 h-4" />
          {error}
        </div>
      )}

      {/* Document List */}
      {isLoading ? (
        <div className="flex items-center justify-center p-8">
          <Loader2 className="w-8 h-8 text-indigo-500 animate-spin" />
        </div>
      ) : filteredDocs.length === 0 ? (
        <div className="text-center p-8 text-slate-400">
          <FileText className="w-12 h-12 mx-auto mb-3 opacity-30" />
          <p className="text-sm">
            {documents.length === 0 
              ? 'Chưa có tài liệu nào' 
              : 'Không tìm thấy tài liệu phù hợp'}
          </p>
        </div>
      ) : (
        <div className="space-y-2">
          {filteredDocs.map((doc) => (
            <div 
              key={doc.id} 
              className="flex items-center gap-3 p-3 bg-white border border-slate-200 rounded-xl hover:border-indigo-300 hover:shadow-sm transition group"
            >
              <div className="text-2xl">{getFileIcon(doc.mime_type)}</div>
              <div className="flex-1 min-w-0">
                <h4 className="font-medium text-slate-800 text-sm truncate">{doc.name}</h4>
                <p className="text-xs text-slate-400">
                  {formatSize(doc.size)} • {formatDate(doc.uploaded_at)}
                </p>
              </div>
              <div className="flex items-center gap-2">
                <button
                  onClick={() => handleDownload(doc, false)}
                  className="p-2 text-slate-400 hover:text-indigo-600 hover:bg-indigo-50 rounded-lg transition"
                  title="Tải về Downloads"
                >
                  <Download className="w-5 h-5" />
                </button>
                <button
                  onClick={() => handleDownload(doc, true)}
                  className="p-2 text-slate-400 hover:text-indigo-600 hover:bg-indigo-50 rounded-lg transition"
                  title="Chọn thư mục"
                >
                  <Folder className="w-4 h-4" />
                </button>
              </div>
            </div>
          ))}
        </div>
      )}

    </div>
  );
};

export default StudentDocuments;
