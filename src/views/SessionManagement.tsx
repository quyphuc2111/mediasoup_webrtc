import React from 'react';
import { 
  Calendar, User, Monitor, 
  ChevronRight, PlayCircle, 
  FileCheck, ShieldAlert, Users 
} from 'lucide-react';
import { UserRole } from '../types';

const SessionManagement: React.FC<{ role: UserRole }> = ({ role }) => {
  const sessions = [
    { 
      id: 1, 
      subject: 'Lập trình Web Cơ bản', 
      teacher: 'Thầy Trần Trung', 
      lab: 'Lab 02', 
      time: '07:30 - 09:00', 
      slot: 'Ca Sáng 1', 
      date: 'Hôm nay', 
      status: 'active',
      students: 24,
      totalStudents: 30
    },
    { 
      id: 2, 
      subject: 'Cơ sở dữ liệu SQL', 
      teacher: 'Cô Mai Anh', 
      lab: 'Lab 01', 
      time: '09:15 - 10:45', 
      slot: 'Ca Sáng 2', 
      date: 'Hôm nay', 
      status: 'upcoming',
      students: 0,
      totalStudents: 28
    },
  ];

  return (
    <div className="space-y-6">
      <div className="flex flex-col md:flex-row md:items-center justify-between gap-4">
        <div>
          <h1 className="text-2xl font-bold text-slate-800">Tổ chức & Quản lý Ca thực hành</h1>
          <p className="text-slate-500">Giám sát hoạt động giảng dạy theo thời gian thực và phân phối tài liệu học tập.</p>
        </div>
        {(role === UserRole.ADMIN || role === UserRole.TEACHER) && (
          <button className="flex items-center gap-2 px-6 py-3 bg-indigo-600 text-white rounded-2xl font-bold hover:bg-indigo-700 transition shadow-lg shadow-indigo-600/20 active:scale-95">
            <PlayCircle className="w-5 h-5" /> Mở ca dạy mới
          </button>
        )}
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-8">
        <div className="lg:col-span-2 space-y-6">
          <div className="flex items-center justify-between">
            <h3 className="font-bold text-slate-800 flex items-center gap-2">
              <Calendar className="w-5 h-5 text-indigo-500" /> Lịch trình ca dạy hôm nay
            </h3>
            <span className="text-[10px] font-black text-slate-400 uppercase tracking-widest">Tuần 24 • Năm học 2025-2026</span>
          </div>
          
          <div className="grid grid-cols-1 gap-4">
            {sessions.map((session) => (
              <div key={session.id} className={`bg-white p-6 rounded-[32px] border transition-all duration-300 flex flex-col sm:flex-row items-center gap-6 group relative overflow-hidden ${session.status === 'active' ? 'border-indigo-200 shadow-xl shadow-indigo-600/5' : 'border-slate-100 hover:border-slate-200 shadow-sm'}`}>
                {session.status === 'active' && <div className="absolute top-0 left-0 w-1 h-full bg-indigo-600"></div>}
                
                <div className={`w-20 h-20 rounded-[24px] flex flex-col items-center justify-center border transition-colors ${session.status === 'active' ? 'bg-indigo-600 text-white border-indigo-500' : 'bg-slate-50 text-slate-400 border-slate-100 group-hover:bg-slate-100'}`}>
                  <span className="text-[10px] font-black uppercase tracking-tighter opacity-70">START</span>
                  <span className="text-xl font-black">{session.time.split(' - ')[0]}</span>
                </div>
                
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2 mb-1">
                    <span className={`px-2 py-0.5 rounded-full text-[10px] font-black uppercase tracking-widest ${session.status === 'active' ? 'bg-emerald-100 text-emerald-600 animate-pulse' : 'bg-slate-100 text-slate-500'}`}>
                      {session.status === 'active' ? 'Đang diễn ra' : 'Sắp tới'}
                    </span>
                    <span className="text-slate-300">•</span>
                    <span className="text-[10px] font-bold text-slate-400 uppercase">{session.slot}</span>
                  </div>
                  <h4 className="font-black text-slate-800 text-lg truncate">{session.subject}</h4>
                  <div className="flex flex-wrap items-center gap-y-1 gap-x-6 mt-2">
                    <span className="flex items-center gap-2 text-xs font-bold text-slate-500">
                      <Monitor className="w-3.5 h-3.5 text-indigo-400" /> {session.lab}
                    </span>
                    <span className="flex items-center gap-2 text-xs font-bold text-slate-500">
                      <User className="w-3.5 h-3.5 text-amber-400" /> {session.teacher}
                    </span>
                    <span className="flex items-center gap-2 text-xs font-bold text-slate-500">
                      <Users className="w-3.5 h-3.5 text-emerald-400" /> {session.students}/{session.totalStudents} SV
                    </span>
                  </div>
                </div>

                <div className="flex items-center gap-3">
                   {session.status === 'active' ? (
                     <button className="px-5 py-2.5 bg-slate-900 text-white text-xs font-black rounded-xl uppercase tracking-widest hover:bg-slate-800 transition shadow-lg">Vào giám sát</button>
                   ) : (
                     <button className="p-3 bg-slate-100 text-slate-400 rounded-xl group-hover:bg-indigo-50 group-hover:text-indigo-600 transition">
                        <ChevronRight className="w-5 h-5" />
                     </button>
                   )}
                </div>
              </div>
            ))}
          </div>
        </div>

        <div className="space-y-6">
          <h3 className="font-bold text-slate-800 flex items-center gap-2">
            <FileCheck className="w-5 h-5 text-emerald-500" /> Báo cáo & Đánh giá
          </h3>
          <div className="bg-slate-900 p-8 rounded-[40px] shadow-2xl relative overflow-hidden text-white">
             <div className="absolute top-0 right-0 p-6 opacity-10"><ShieldAlert className="w-24 h-24" /></div>
             <p className="text-indigo-400 text-[10px] font-black uppercase tracking-[0.2em] mb-4">Thống kê ca dạy</p>
             <div className="space-y-6">
                <div>
                  <div className="flex justify-between text-xs font-bold mb-2">
                    <span>Tài liệu đã phát</span>
                    <span>12/12</span>
                  </div>
                  <div className="h-1.5 bg-white/10 rounded-full overflow-hidden">
                    <div className="h-full bg-emerald-500 w-full"></div>
                  </div>
                </div>
                <div>
                  <div className="flex justify-between text-xs font-bold mb-2">
                    <span>Tỉ lệ SV hoàn thành bài tập</span>
                    <span>85%</span>
                  </div>
                  <div className="h-1.5 bg-white/10 rounded-full overflow-hidden">
                    <div className="h-full bg-indigo-500 w-[85%]"></div>
                  </div>
                </div>
             </div>
             <button className="w-full mt-10 py-4 bg-white/5 hover:bg-white/10 rounded-2xl text-[10px] font-black uppercase tracking-widest transition-all border border-white/10">
               Xuất báo cáo đồng bộ
             </button>
          </div>

          <div className="bg-white p-6 rounded-3xl border border-slate-200 shadow-sm">
             <h4 className="text-sm font-black text-slate-800 uppercase tracking-tighter mb-4">Ghi chú vận hành</h4>
             <div className="space-y-4">
                <div className="p-3 bg-rose-50 border border-rose-100 rounded-xl">
                   <p className="text-[10px] text-rose-600 font-bold mb-1 italic">12:30 - Admin</p>
                   <p className="text-xs text-rose-800">Cần kiểm tra lại kết nối PC-05 tại Lab 02 trước ca chiều.</p>
                </div>
                <div className="p-3 bg-amber-50 border border-amber-100 rounded-xl">
                   <p className="text-[10px] text-amber-600 font-bold mb-1 italic">10:45 - Teacher</p>
                   <p className="text-xs text-amber-800">Môn CSDL cần thêm 15p cho bài kiểm tra cuối ca.</p>
                </div>
             </div>
          </div>
        </div>
      </div>
    </div>
  );
};

export default SessionManagement;
