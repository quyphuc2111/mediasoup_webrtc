import React from 'react';
import { Users, AlertCircle, Database, Book, Globe } from 'lucide-react';
import { UserRole } from '../types';

const data = [
  { name: 'Thứ 2', usage: 45 }, { name: 'Thứ 3', usage: 62 }, { name: 'Thứ 4', usage: 55 },
  { name: 'Thứ 5', usage: 80 }, { name: 'Thứ 6', usage: 95 }, { name: 'Thứ 7', usage: 30 }, { name: 'CN', usage: 10 },
];

const StatCard: React.FC<{ title: string; value: string; icon: any; color: string; subtitle?: string }> = ({ title, value, icon: Icon, color, subtitle }) => (
  <div className="bg-white p-8 rounded-3xl border border-slate-200 shadow-sm hover:shadow-xl hover:scale-[1.02] transition-all group">
    <div className="flex justify-between items-start mb-6">
      <div className={`p-4 rounded-2xl ${color} shadow-lg transition-transform group-hover:rotate-12`}>
        <Icon className="w-6 h-6 text-white" />
      </div>
    </div>
    <h3 className="text-slate-400 text-[10px] font-black uppercase tracking-widest">{title}</h3>
    <p className="text-3xl font-black text-slate-800 mt-2">{value}</p>
    {subtitle && <p className="text-xs text-slate-400 mt-2 font-medium">{subtitle}</p>}
  </div>
);

const Dashboard: React.FC<{ role: UserRole }> = () => {
  return (
    <div className="space-y-10">
      <div className="flex flex-col md:flex-row md:items-center justify-between gap-6">
        <div>
          <h1 className="text-3xl font-black text-slate-800 tracking-tight">Trung tâm dữ liệu</h1>
          <p className="text-slate-500 font-medium">Báo cáo hiệu suất phòng máy theo Năm học & Ca thực hành.</p>
        </div>
        <div className="flex gap-4">
          <div className="bg-white px-6 py-3 rounded-2xl border border-slate-200 flex items-center gap-3">
             <span className="text-xs font-black text-indigo-600 uppercase">Năm học:</span>
             <span className="text-sm font-bold text-slate-700">2025-2026</span>
          </div>
          <button className="px-8 py-3 bg-indigo-600 text-white rounded-2xl text-sm font-black hover:bg-indigo-700 transition shadow-xl shadow-indigo-600/30 active:scale-95 uppercase tracking-widest">Tạo báo cáo</button>
        </div>
      </div>

      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-6">
        <StatCard title="Tổng số năm học" value="02" icon={Database} color="bg-indigo-500" subtitle="Đã kích hoạt: 01" />
        <StatCard title="Học sinh đăng ký" value="842" icon={Users} color="bg-emerald-500" subtitle="+12% so với học kỳ trước" />
        <StatCard title="Tổng số môn học" value="18" icon={Book} color="bg-amber-500" subtitle="Mapping hoàn tất 100%" />
        <StatCard title="Thiết bị gặp lỗi" value="03" icon={AlertCircle} color="bg-rose-500" subtitle="Đang sửa chữa: 02" />
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-8">
        <div className="lg:col-span-2 bg-white p-10 rounded-[40px] border border-slate-200 shadow-sm relative overflow-hidden">
          <div className="absolute top-0 right-0 p-10 opacity-5 pointer-events-none"><Globe className="w-64 h-64 text-slate-900" /></div>
          <div className="flex items-center justify-between mb-10 relative z-10">
            <h3 className="text-xl font-black text-slate-800 uppercase tracking-tight">Tỉ lệ chiếm dụng máy tính (%)</h3>
            <div className="flex gap-2">
               <span className="w-3 h-3 rounded-full bg-indigo-500"></span>
               <span className="text-[10px] font-black text-slate-400 uppercase tracking-widest">7 ngày gần nhất</span>
            </div>
          </div>
          <div className="h-80 relative z-10 flex items-center justify-center">
            {/* Simple bar chart without recharts */}
            <div className="flex items-end gap-4 h-full w-full px-4">
              {data.map((item, idx) => (
                <div key={idx} className="flex-1 flex flex-col items-center gap-2">
                  <div 
                    className="w-full bg-indigo-500 rounded-t-lg transition-all hover:bg-indigo-600"
                    style={{ height: `${item.usage * 2.5}px` }}
                  />
                  <span className="text-xs text-slate-500">{item.name}</span>
                </div>
              ))}
            </div>
          </div>
        </div>

        <div className="bg-slate-900 p-10 rounded-[40px] shadow-2xl relative overflow-hidden text-white">
          <div className="absolute -bottom-10 -right-10 w-48 h-48 bg-indigo-500 rounded-full blur-[80px] opacity-20"></div>
          <h3 className="text-xl font-black uppercase tracking-tight mb-8">Ca thực hành hôm nay</h3>
          <div className="space-y-6 relative z-10">
            {[1, 2, 3].map((i) => (
              <div key={i} className="flex items-center gap-6 p-5 rounded-3xl bg-white/5 border border-white/10 hover:bg-white/10 transition-colors group cursor-pointer">
                <div className="w-14 h-14 bg-indigo-500/20 rounded-2xl flex items-center justify-center text-indigo-400 font-black text-lg group-hover:scale-110 transition-transform">0{i}</div>
                <div className="flex-1">
                  <p className="font-black text-slate-100 group-hover:text-indigo-400 transition-colors">Lập trình Python {i}</p>
                  <p className="text-[10px] font-bold text-slate-400 uppercase tracking-widest mt-1">Khối 10 • 14:00 - 16:00</p>
                </div>
              </div>
            ))}
          </div>
          <button className="w-full mt-10 py-4 bg-white/10 hover:bg-white/20 rounded-2xl text-[10px] font-black uppercase tracking-[0.2em] transition-all">Toàn bộ lịch biểu</button>
        </div>
      </div>
    </div>
  );
};

export default Dashboard;
