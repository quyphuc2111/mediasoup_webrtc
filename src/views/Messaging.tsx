import React from 'react';
import { Search, Plus, MoreVertical, Send, Smile, Paperclip } from 'lucide-react';
import { UserAccount as User } from '../types';

const Messaging: React.FC<{ user: User }> = () => {
  return (
    <div className="h-[calc(100vh-10rem)] bg-white rounded-3xl border border-slate-200 overflow-hidden shadow-sm flex">
      {/* Contact List */}
      <div className="w-full md:w-80 border-r border-slate-100 flex flex-col">
        <div className="p-4 border-b border-slate-100">
          <div className="flex items-center justify-between mb-4">
            <h3 className="font-bold text-xl text-slate-800">Tin nhắn</h3>
            <button className="p-2 bg-indigo-50 text-indigo-600 rounded-xl">
              <Plus className="w-5 h-5" />
            </button>
          </div>
          <div className="relative">
             <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-slate-400" />
             <input 
               type="text" 
               placeholder="Tìm hội thoại..." 
               className="w-full pl-10 pr-4 py-2 bg-slate-100 border-none rounded-xl text-sm outline-none"
             />
          </div>
        </div>
        
        <div className="flex-1 overflow-y-auto scrollbar-hide">
           {[1, 2, 3, 4, 5, 6].map((i) => (
             <div key={i} className={`p-4 flex items-center gap-3 hover:bg-slate-50 cursor-pointer transition ${i === 1 ? 'bg-indigo-50/50' : ''}`}>
               <div className="relative">
                 <img src={`https://i.pravatar.cc/150?u=${i+20}`} className="w-12 h-12 rounded-2xl object-cover" alt="contact" />
                 {i % 2 === 0 && <div className="absolute -top-1 -right-1 w-3 h-3 bg-emerald-500 border-2 border-white rounded-full"></div>}
               </div>
               <div className="flex-1 min-w-0">
                 <div className="flex justify-between items-baseline mb-0.5">
                   <h4 className="text-sm font-bold text-slate-800 truncate">Lớp lập trình React {i}</h4>
                   <span className="text-[10px] text-slate-400">14:2{i}</span>
                 </div>
                 <p className="text-xs text-slate-500 truncate">Thầy ơi em không cài được Node...</p>
               </div>
               {i === 1 && <div className="w-5 h-5 bg-rose-500 text-white text-[10px] font-black rounded-full flex items-center justify-center shadow-sm">2</div>}
             </div>
           ))}
        </div>
      </div>

      {/* Chat Area */}
      <div className="flex-1 flex flex-col bg-slate-50">
         <div className="p-4 bg-white border-b border-slate-100 flex items-center justify-between shadow-sm">
            <div className="flex items-center gap-3">
               <img src="https://i.pravatar.cc/150?u=21" className="w-10 h-10 rounded-xl object-cover" alt="chat-target" />
               <div>
                  <h4 className="text-sm font-bold text-slate-800 leading-none">Lớp lập trình React 1</h4>
                  <p className="text-[10px] text-emerald-500 font-bold uppercase mt-1">Đang hoạt động</p>
               </div>
            </div>
            <button className="p-2 text-slate-400 hover:text-slate-600 rounded-lg transition">
               <MoreVertical className="w-5 h-5" />
            </button>
         </div>

         <div className="flex-1 p-6 overflow-y-auto space-y-4">
            <div className="flex justify-center">
               <span className="text-[10px] font-bold text-slate-400 bg-slate-200/50 px-3 py-1 rounded-full uppercase tracking-widest">Hôm nay</span>
            </div>

            <div className="flex items-start gap-3">
               <img src="https://i.pravatar.cc/150?u=25" className="w-8 h-8 rounded-lg mt-1" alt="avatar" />
               <div className="max-w-md p-4 bg-white rounded-2xl rounded-tl-none border border-slate-200 text-sm shadow-sm">
                  Dạ chào thầy, em có câu hỏi về phần useEffect ạ. Sao nó cứ render vô tận hoài vậy thầy?
               </div>
            </div>

            <div className="flex flex-col items-end gap-1">
               <div className="max-w-md p-4 bg-indigo-600 text-white rounded-2xl rounded-tr-none text-sm shadow-md">
                  Em kiểm tra lại dependency array nhé. Có thể em đang update chính cái state mà em dependency vào đó.
               </div>
               <span className="text-[10px] text-slate-400 px-2 font-medium">Đã xem 14:35</span>
            </div>

            <div className="flex items-start gap-3">
               <img src="https://i.pravatar.cc/150?u=25" className="w-8 h-8 rounded-lg mt-1" alt="avatar" />
               <div className="max-w-md p-4 bg-white rounded-2xl rounded-tl-none border border-slate-200 text-sm shadow-sm">
                  À em thấy rồi, em cảm ơn thầy ạ! Để em sửa lại.
               </div>
            </div>
         </div>

         <div className="p-4 bg-white border-t border-slate-100">
            <div className="max-w-4xl mx-auto relative flex items-center gap-2">
               <button className="p-2 text-slate-400 hover:text-indigo-600 transition">
                  <Paperclip className="w-5 h-5" />
               </button>
               <input 
                 type="text" 
                 placeholder="Nhập nội dung tin nhắn..."
                 className="flex-1 bg-slate-100 border-none rounded-2xl px-4 py-3 text-sm outline-none focus:ring-2 focus:ring-indigo-500 focus:bg-white transition-all"
               />
               <button className="p-2 text-slate-400 hover:text-indigo-600 transition">
                  <Smile className="w-5 h-5" />
               </button>
               <button className="p-3 bg-indigo-600 text-white rounded-2xl hover:bg-indigo-700 transition shadow-lg shadow-indigo-600/20">
                  <Send className="w-5 h-5" />
               </button>
            </div>
         </div>
      </div>
    </div>
  );
};

export default Messaging;
