import { useState } from 'react';
import { ScreenSharingPage } from './pages/ScreenSharingPage';
import { ViewClientPage } from './pages/ViewClientPage';
import { FileTransferPage } from './pages/FileTransferPage';
import { StudentAgent } from './components/StudentAgent';
import { AuthSettings } from './components/AuthSettings';
import './App.css';

// Define available pages
type Page = 'home' | 'screen-sharing' | 'view-client' | 'student-agent' | 'auth-settings' | 'file-transfer';

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
      case 'file-transfer':
        return <FileTransferPage onBack={() => navigateTo('home')} />;
      case 'student-agent':
        return <StudentAgent onBack={() => navigateTo('home')} />;
      case 'auth-settings':
        return (
          <div>
            <button onClick={() => navigateTo('home')} className="btn" style={{ margin: '1rem' }}>
              ← Back
            </button>
            <AuthSettings />
          </div>
        );
      case 'home':
      default:
        return (
          <main className="container">
            <h1>Smartlab</h1>
            <p className="subtitle">Chọn chức năng bạn muốn sử dụng</p>

            {/* Teaching Features Section */}
            <div className="feature-section">
              <h2 className="section-title">Chức năng giảng bài</h2>
              <div className="section-divider"></div>
              <div className="page-grid">
                <button
                  onClick={() => navigateTo('screen-sharing')}
                  className="btn page-card"
                >
                  <span className="page-icon">🖥️</span>
                  <span className="page-title">Screen Sharing</span>
                  <span className="page-desc">Chia sẻ màn hình cho lớp học</span>
                </button>
              </div>
            </div>

            {/* Monitoring & Control Features Section */}
            <div className="feature-section">
              <h2 className="section-title">Xem và điều khiển màn hình</h2>
              <div className="section-divider"></div>
              <div className="page-grid">
                <button
                  onClick={() => navigateTo('view-client')}
                  className="btn page-card"
                >
                  <span className="page-icon">👁️</span>
                  <span className="page-title">View Client</span>
                  <span className="page-desc">Xem màn hình học sinh</span>
                </button>

              

                <button
                  onClick={() => navigateTo('student-agent')}
                  className="btn page-card student"
                >
                  <span className="page-icon">🎓</span>
                  <span className="page-title">Student Agent</span>
                  <span className="page-desc">Cho phép giáo viên xem màn hình</span>
                </button>

                  <button
                  onClick={() => navigateTo('file-transfer')}
                  className="btn page-card"
                >
                  <span className="page-icon">📁</span>
                  <span className="page-title">File Transfer</span>
                  <span className="page-desc">Gửi/nhận file với học sinh</span>
                </button>
              </div>
            </div>
          </main>
        );
    }
  };

  return renderPage();
}

export default App;
