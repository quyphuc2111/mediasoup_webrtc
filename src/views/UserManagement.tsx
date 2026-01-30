import React from 'react';
import { 
  Plus, Search, Filter, 
  Edit2, Trash2, Shield, UserCheck, Mail
} from 'lucide-react';

const UserManagement: React.FC = () => {
  const users = [
    { id: '1', name: 'Nguyễn Văn A', email: 'vana@edu.vn', role: 'Giáo viên', status: 'active', joinDate: '12/10/2023' },
    { id: '2', name: 'Trần Thị B', email: 'thib@edu.vn', role: 'Học sinh', status: 'active', joinDate: '15/10/2023' },
    { id: '3', name: 'Lê Văn C', email: 'vanc@edu.vn', role: 'Học sinh', status: 'inactive', joinDate: '20/11/2023' },
    { id: '4', name: 'Phạm Thị D', email: 'thid@edu.vn', role: 'Giáo viên', status: 'active', joinDate: '01/01/2024' },
    { id: '5', name: 'Hoàng Văn E', email: 'vane@edu.vn', role: 'Quản trị', status: 'active', joinDate: '05/01/2024' },
  ];

  return (
    <div className="space-y-6">
      <div className="flex flex-col md:flex-row md:items-center justify-between gap-4">
        <div>
          <h1 className="text-2xl font-bold text-slate-800">Quản lý người dùng</h1>
          <p className="text-slate-500">Thêm, sửa, xóa và phân quyền cho người dùng trong hệ thống.</p>
        </div>
        <button className="flex items-center gap-2 px-6 py-3 bg-indigo-600 text-white rounded-2xl font-bold hover:bg-indigo-700 transition shadow-lg shadow-indigo-600/20">
          <Plus className="w-5 h-5" /> Thêm người dùng mới
        </button>
      </div>

      <div className="bg-white rounded-3xl border border-slate-200 overflow-hidden shadow-sm">
        <div className="p-4 border-b border-slate-100 flex flex-col sm:flex-row sm:items-center justify-between gap-4">
          <div className="relative flex-1 max-w-md">
            <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-slate-400" />
            <input 
              type="text" 
              placeholder="Tìm kiếm theo tên, email..." 
              className="w-full pl-10 pr-4 py-2 bg-slate-100 border-none rounded-xl text-sm outline-none focus:ring-2 focus:ring-indigo-500 transition-all"
            />
          </div>
          <div className="flex items-center gap-2">
            <button className="flex items-center gap-2 px-3 py-2 bg-white border border-slate-200 rounded-xl text-xs font-bold text-slate-600 hover:bg-slate-50 transition">
              <Filter className="w-3.5 h-3.5" /> Lọc theo vai trò
            </button>
          </div>
        </div>

        <div className="overflow-x-auto">
          <table className="w-full text-left border-collapse">
            <thead>
              <tr className="bg-slate-50 text-[10px] uppercase tracking-wider font-black text-slate-400">
                <th className="px-6 py-4">Người dùng</th>
                <th className="px-6 py-4">Vai trò</th>
                <th className="px-6 py-4">Ngày tham gia</th>
                <th className="px-6 py-4">Trạng thái</th>
                <th className="px-6 py-4 text-right">Thao tác</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-100">
              {users.map((user) => (
                <tr key={user.id} className="hover:bg-slate-50/80 transition cursor-pointer">
                  <td className="px-6 py-4">
                    <div className="flex items-center gap-3">
                      <div className="w-10 h-10 rounded-xl bg-indigo-50 flex items-center justify-center">
                        <UserCheck className="w-5 h-5 text-indigo-500" />
                      </div>
                      <div>
                        <p className="text-sm font-bold text-slate-800">{user.name}</p>
                        <p className="text-xs text-slate-400 flex items-center gap-1">
                          <Mail className="w-3 h-3" /> {user.email}
                        </p>
                      </div>
                    </div>
                  </td>
                  <td className="px-6 py-4">
                    <div className="flex items-center gap-2">
                      <Shield className={`w-4 h-4 ${user.role === 'Quản trị' ? 'text-rose-500' : 'text-slate-400'}`} />
                      <span className="text-xs font-medium text-slate-600">{user.role}</span>
                    </div>
                  </td>
                  <td className="px-6 py-4 text-xs text-slate-500">{user.joinDate}</td>
                  <td className="px-6 py-4">
                    <span className={`px-2 py-1 rounded-full text-[10px] font-bold uppercase ${user.status === 'active' ? 'bg-emerald-100 text-emerald-700' : 'bg-slate-100 text-slate-500'}`}>
                      {user.status === 'active' ? 'Hoạt động' : 'Tạm khóa'}
                    </span>
                  </td>
                  <td className="px-6 py-4 text-right">
                    <div className="flex items-center justify-end gap-2">
                      <button className="p-2 hover:bg-white border border-transparent hover:border-slate-200 rounded-lg text-slate-400 hover:text-indigo-600 transition">
                        <Edit2 className="w-4 h-4" />
                      </button>
                      <button className="p-2 hover:bg-white border border-transparent hover:border-slate-200 rounded-lg text-slate-400 hover:text-rose-600 transition">
                        <Trash2 className="w-4 h-4" />
                      </button>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        
        <div className="p-4 border-t border-slate-100 flex items-center justify-between text-xs font-medium text-slate-500">
          <p>Hiển thị 5 trên 1,248 người dùng</p>
          <div className="flex gap-2">
            <button className="px-3 py-1 bg-slate-100 rounded-lg hover:bg-slate-200 transition">Trang trước</button>
            <button className="px-3 py-1 bg-slate-100 rounded-lg hover:bg-slate-200 transition">Trang sau</button>
          </div>
        </div>
      </div>
    </div>
  );
};

export default UserManagement;
