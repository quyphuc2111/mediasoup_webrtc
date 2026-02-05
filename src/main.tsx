import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import StudentTrayApp from "./StudentTrayApp";
import { getName } from '@tauri-apps/api/app';

// Detect if running in student tray mode
async function detectStudentMode(): Promise<boolean> {
  try {
    const appName = await getName();
    return appName.toLowerCase().includes('student');
  } catch {
    return false;
  }
}

// Polyfill for navigator.mediaDevices if it doesn't exist (for Tauri WebView compatibility)
if (typeof navigator !== 'undefined' && !navigator.mediaDevices) {
  console.warn('[Polyfill] navigator.mediaDevices không tồn tại, đang thử khởi tạo...');
  
  // Try to create a minimal mediaDevices object
  // Note: This may not work in all WebView contexts
  try {
    // For some WebView implementations, mediaDevices might need to be explicitly created
    // or might be available under a different path
    const nav = navigator as any;
    
    // Check if getUserMedia exists (legacy API)
    if (nav.getUserMedia || nav.webkitGetUserMedia || nav.mozGetUserMedia) {
      console.log('[Polyfill] Legacy getUserMedia found, attempting to create mediaDevices wrapper');
      
      // Create a basic mediaDevices object
      nav.mediaDevices = {
        getUserMedia: function(constraints: MediaStreamConstraints) {
          return new Promise((resolve, reject) => {
            const getUserMedia = nav.getUserMedia || nav.webkitGetUserMedia || nav.mozGetUserMedia;
            getUserMedia.call(nav, constraints, resolve, reject);
          });
        },
        // getDisplayMedia might not be available in legacy APIs
        getDisplayMedia: function(_constraints: MediaStreamConstraints) {
          return Promise.reject(new Error('getDisplayMedia không khả dụng trong WebView này. Vui lòng sử dụng trình duyệt hiện đại hoặc cập nhật Tauri.'));
        }
      };
      
      console.log('[Polyfill] Created basic mediaDevices object');
    } else {
      console.error('[Polyfill] Không tìm thấy getUserMedia API. MediaDevices không khả dụng.');
    }
  } catch (error) {
    console.error('[Polyfill] Lỗi khi khởi tạo mediaDevices polyfill:', error);
  }
} else if (typeof navigator !== 'undefined' && navigator.mediaDevices) {
  console.log('[MediaDevices] ✅ navigator.mediaDevices is available');
  console.log('[MediaDevices] Methods:', Object.keys(navigator.mediaDevices));
}

// Render appropriate app based on mode
detectStudentMode().then(isStudentMode => {
  const rootElement = document.getElementById("root") as HTMLElement;
  
  if (isStudentMode) {
    console.log('[App] Running in Student Tray mode');
    ReactDOM.createRoot(rootElement).render(
      <React.StrictMode>
        <StudentTrayApp />
      </React.StrictMode>,
    );
  } else {
    console.log('[App] Running in Teacher/Admin mode');
    ReactDOM.createRoot(rootElement).render(
      <React.StrictMode>
        <App />
      </React.StrictMode>,
    );
  }
});
