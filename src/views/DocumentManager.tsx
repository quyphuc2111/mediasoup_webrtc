import React, { useState } from 'react';
import { 
  FileText, Upload, Download, Search, 
  Filter, MoreHorizontal, FileIcon, 
  Trash2, Share2, FolderOpen
} from 'lucide-react';
import { UserAccount as User } from '../types';

const DocumentManager: React.FC<{ user: User }> = () => {
  const [activeFolder, setActiveFolder] = useState('Tất cả tài liệu');

  const docs = [
    { id: '1', title: 'Giáo trình ReactJS Cơ bản', type: 'PDF', size: '4.5 MB', date: '12/03/2024' },
    { id: '2', title: 'Bài tập thực hành tuần 4', type: 'DOCX', size: '1.2 MB', date: '14/03/2024' },
    { id: '3', title: 'Source code mẫu Lab 01', type: 'ZIP', size: '25.8 MB', date: '15/03/2024' },
    { id: '4', title: 'Video hướng dẫn cài đặt', type: 'MP4', size: '154.0 MB', date: '16/03/2024' },
    { id: '5', title: 'Tài liệu tham khảo thêm', type: 'PDF', size: '8.1 MB', date: '18/03/2024' },
  ];

  return (
    <div className="space-y-6">
      <div className="flex flex-col md:flex-row md:items-center justify-between gap-4">
        <div>
          <h1 className="text-2xl font-bold text-slate-800">Kho tài liệu</h1>
          <p className="text-slate-500">Quản lý và chia sẻ tài liệu giảng dạy cho các lớp học.</p>
        </div>
        <div className="flex gap-2">
          <label className="flex items-center gap-2 px-6 py-3 bg-indigo-600 text-white rounded-2xl font-bold hover:bg-indigo-700 transition shadow-lg shadow-indigo-600/20 cursor-pointer">
            <Upload className="w-5 h-5" /> Tải tài liệu lên
            <input type="file" className="hidden" />
          </label>
        </div>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-4 gap-6">
        {/* Sidebar Folders */}
        <div className="space-y-2">
          {['Tất cả tài liệu', 'Của tôi', 'Được chia sẻ', 'Lớp học React', 'Lớp học NodeJS', 'Thùng rác'].map((f, i) => (
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
              {i === 0 && <span className="bg-slate-100 text-slate-500 text-[10px] px-2 py-0.5 rounded-full font-bold">24</span>}
            </button>
          ))}
        </div>

        {/* File List */}
        <div className="lg:col-span-3 bg-white rounded-3xl border border-slate-200 overflow-hidden shadow-sm">
           <div className="p-4 border-b border-slate-100 flex items-center justify-between">
              <div className="relative w-64">
                <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-slate-400" />
                <input 
                  type="text" 
                  placeholder="Tìm tài liệu..." 
                  className="w-full pl-10 pr-4 py-2 bg-slate-50 border-none rounded-xl text-sm outline-none"
                />
              </div>
              <button className="p-2 hover:bg-slate-50 rounded-xl text-slate-400 transition">
                <Filter className="w-5 h-5" />
              </button>
           </div>

           <div className="grid grid-cols-1 divide-y divide-slate-100">
              {docs.map((doc) => (
                <div key={doc.id} className="p-4 flex items-center gap-4 hover:bg-slate-50/50 transition group">
                   <div className="p-3 bg-indigo-50 rounded-2xl text-indigo-500 group-hover:scale-110 transition">
                      <FileIcon className="w-6 h-6" />
                   </div>
                   <div className="flex-1 min-w-0">
                      <h4 className="font-bold text-slate-800 text-sm truncate">{doc.title}</h4>
                      <p className="text-[10px] text-slate-400 mt-0.5 uppercase tracking-wider font-bold">
                        {doc.type} • {doc.size} • Tải lên: {doc.date}
                      </p>
                   </div>
                   <div className="flex items-center gap-2 opacity-0 group-hover:opacity-100 transition">
                      <button className="p-2 text-slate-400 hover:text-indigo-600 transition" title="Tải xuống">
                        <Download className="w-4 h-4" />
                      </button>
                      <button className="p-2 text-slate-400 hover:text-emerald-600 transition" title="Chia sẻ">
                        <Share2 className="w-4 h-4" />
                      </button>
                      <button className="p-2 text-slate-400 hover:text-rose-600 transition" title="Xóa">
                        <Trash2 className="w-4 h-4" />
                      </button>
                      <button className="p-2 text-slate-400 hover:bg-slate-100 rounded-lg">
                        <MoreHorizontal className="w-4 h-4" />
                      </button>
                   </div>
                </div>
              ))}
           </div>
           
           <div className="p-12 text-center text-slate-400 bg-slate-50/50">
             <FileText className="w-12 h-12 mx-auto mb-4 opacity-20" />
             <p className="text-sm">Hiển thị tất cả 5 tài liệu trong thư mục này.</p>
           </div>
        </div>
      </div>
    </div>
  );
};

export default DocumentManager;
