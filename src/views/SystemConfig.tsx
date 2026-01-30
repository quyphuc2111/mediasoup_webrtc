import React, { useState } from 'react';
import { 
  Plus, Edit2, Trash2, Search, Calendar, 
  Layers, BookOpen, Clock, 
  ArrowRightLeft, Settings2
} from 'lucide-react';
import { SchoolYear, PracticeTimeSlot } from '../types';

const SystemConfig: React.FC = () => {
  const [activeTab, setActiveTab] = useState<'years' | 'grades' | 'subjects' | 'slots' | 'mappings'>('years');

  const mockYears: SchoolYear[] = [
    { schoolYearId: 1, schoolYearName: '2024-2025', startDate: '2024-09-05', endDate: '2025-05-31' },
    { schoolYearId: 2, schoolYearName: '2025-2026', startDate: '2025-09-05', endDate: '2026-05-31' },
  ];

  const mockSlots: PracticeTimeSlot[] = [
    { practiceTimeSlotId: 1, practiceTimeSlotName: 'Ca Sáng 1', schoolYearId: 1, startTime: '07:30', endTime: '09:00' },
    { practiceTimeSlotId: 2, practiceTimeSlotName: 'Ca Sáng 2', schoolYearId: 1, startTime: '09:15', endTime: '10:45' },
  ];

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-slate-800">Quản lý danh mục đào tạo</h1>
          <p className="text-slate-500">Cấu hình tham số hệ thống và các bảng liên kết (Mapping) theo năm học.</p>
        </div>
      </div>

      <div className="flex flex-wrap gap-2 p-1.5 bg-slate-100 rounded-2xl w-fit">
        {[
          { id: 'years', label: 'Năm học', icon: Calendar },
          { id: 'grades', label: 'Lớp học', icon: Layers },
          { id: 'subjects', label: 'Môn học', icon: BookOpen },
          { id: 'slots', label: 'Khung giờ', icon: Clock },
          { id: 'mappings', label: 'Liên kết (Mapping)', icon: ArrowRightLeft },
        ].map(tab => (
          <button 
            key={tab.id}
            onClick={() => setActiveTab(tab.id as any)}
            className={`flex items-center gap-2 px-5 py-2.5 rounded-xl text-sm font-bold transition-all ${activeTab === tab.id ? 'bg-white text-indigo-600 shadow-sm' : 'text-slate-500 hover:bg-slate-200'}`}
          >
            <tab.icon className="w-4 h-4" /> {tab.label}
          </button>
        ))}
      </div>

      <div className="bg-white rounded-3xl border border-slate-200 shadow-sm overflow-hidden">
        <div className="p-4 border-b border-slate-100 flex flex-col sm:flex-row justify-between items-center bg-slate-50/50 gap-4">
           <div className="relative w-full sm:w-80">
              <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-slate-400" />
              <input type="text" placeholder={`Tìm kiếm ${activeTab}...`} className="w-full pl-10 pr-4 py-2.5 bg-white border border-slate-200 rounded-xl text-sm outline-none focus:ring-2 focus:ring-indigo-500 transition-all" />
           </div>
           <div className="flex gap-2 w-full sm:w-auto">
             <button className="flex-1 sm:flex-none flex items-center justify-center gap-2 px-5 py-2.5 bg-slate-800 text-white rounded-xl text-sm font-bold hover:bg-slate-900 transition shadow-lg shadow-slate-900/10">
                <Settings2 className="w-4 h-4" /> Cài đặt hàng loạt
             </button>
             <button className="flex-1 sm:flex-none flex items-center justify-center gap-2 px-5 py-2.5 bg-indigo-600 text-white rounded-xl text-sm font-bold hover:bg-indigo-700 transition shadow-lg shadow-indigo-600/20">
                <Plus className="w-4 h-4" /> Thêm mới
             </button>
           </div>
        </div>

        <div className="overflow-x-auto">
          <table className="w-full text-left">
            <thead>
              <tr className="text-[10px] uppercase font-black text-slate-400 border-b border-slate-100 bg-slate-50/30">
                <th className="px-6 py-4">STT</th>
                <th className="px-6 py-4">
                  {activeTab === 'years' ? 'Niên khóa' : 
                   activeTab === 'grades' ? 'Tên lớp' : 
                   activeTab === 'subjects' ? 'Tên môn' : 
                   activeTab === 'slots' ? 'Khung ca' : 'Cấu trúc liên kết'}
                </th>
                <th className="px-6 py-4">Thông tin chi tiết</th>
                <th className="px-6 py-4 text-right">Thao tác</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-100">
              {activeTab === 'years' && mockYears.map((y, idx) => (
                <tr key={y.schoolYearId} className="hover:bg-slate-50 transition group">
                  <td className="px-6 py-4 text-sm text-slate-400 font-mono">{idx + 1}</td>
                  <td className="px-6 py-4 font-bold text-slate-700">{y.schoolYearName}</td>
                  <td className="px-6 py-4">
                    <div className="flex gap-4">
                      <div className="text-xs"><span className="text-slate-400">Bắt đầu:</span> <span className="font-semibold">{y.startDate}</span></div>
                      <div className="text-xs"><span className="text-slate-400">Kết thúc:</span> <span className="font-semibold">{y.endDate}</span></div>
                    </div>
                  </td>
                  <td className="px-6 py-4 text-right">
                    <RowActions />
                  </td>
                </tr>
              ))}
              {activeTab === 'slots' && mockSlots.map((s, idx) => (
                <tr key={s.practiceTimeSlotId} className="hover:bg-slate-50 transition group">
                  <td className="px-6 py-4 text-sm text-slate-400 font-mono">{idx + 1}</td>
                  <td className="px-6 py-4 font-bold text-slate-700">{s.practiceTimeSlotName}</td>
                  <td className="px-6 py-4">
                    <div className="flex items-center gap-2">
                      <span className="px-2 py-1 bg-indigo-50 text-indigo-600 rounded text-xs font-bold">{s.startTime}</span>
                      <ArrowRightLeft className="w-3 h-3 text-slate-300" />
                      <span className="px-2 py-1 bg-rose-50 text-rose-600 rounded text-xs font-bold">{s.endTime}</span>
                    </div>
                  </td>
                  <td className="px-6 py-4 text-right">
                    <RowActions />
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
};

const RowActions = () => (
  <div className="flex justify-end gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
    <button className="p-2 text-slate-400 hover:text-indigo-600 hover:bg-indigo-50 rounded-lg transition">
      <Edit2 className="w-4 h-4" />
    </button>
    <button className="p-2 text-slate-400 hover:text-rose-600 hover:bg-rose-50 rounded-lg transition">
      <Trash2 className="w-4 h-4" />
    </button>
  </div>
);

export default SystemConfig;
