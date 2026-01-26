import { useState } from 'react';
import { ScreenSharingPage } from './pages/ScreenSharingPage';
import { ViewClientPage } from './pages/ViewClientPage';
import { StudentAgent } from './components/StudentAgent';
import './App.css';

// Define available pages
type Page = 'home' | 'screen-sharing' | 'view-client' | 'student-agent';

function App() {
  const [currentPage, setCurrentPage] = useState<Page>('home');

  // Navigate to a page
  const navigateTo = (page: Page) => {
    setCurrentPage(page);
  };

  // Render current page
  const renderPage = () => {
    switch (currentPage) {
      case 'screen-sharing':
        return <ScreenSharingPage onBack={() => navigateTo('home')} />;
      case 'view-client':
        return <ViewClientPage onBack={() => navigateTo('home')} />;
      case 'student-agent':
        return <StudentAgent onBack={() => navigateTo('home')} />;
      case 'home':
      default:
        return (
          <main className="container">
            <h1>Smartlab</h1>
            <p className="subtitle">Chọn chức năng bạn muốn sử dụng</p>
            
            <div className="page-grid">
              {/* Teacher Features */}
              <button 
                onClick={() => navigateTo('screen-sharing')} 
                className="btn page-card"
              >
                <span className="page-icon">🖥️</span>
                <span className="page-title">Screen Sharing</span>
                <span className="page-desc">Chia sẻ màn hình cho lớp học</span>
              </button>
              
              <button 
                onClick={() => navigateTo('view-client')} 
                className="btn page-card"
              >
                <span className="page-icon">👁️</span>
                <span className="page-title">View Client</span>
                <span className="page-desc">Xem màn hình học sinh</span>
              </button>
              
              {/* Student Features */}
              <button 
                onClick={() => navigateTo('student-agent')} 
                className="btn page-card student"
              >
                <span className="page-icon">🎓</span>
                <span className="page-title">Student Agent</span>
                <span className="page-desc">Cho phép giáo viên xem màn hình</span>
              </button>
            </div>
          </main>
        );
    }
  };

  return renderPage();
}

export default App;
