import React from 'react';
import StudentTrayWindow from './components/StudentTrayWindow';
import './App.css';

/**
 * Student Tray App - Minimal UI for system tray mode
 * This is shown when the student app runs in the background
 */
const StudentTrayApp: React.FC = () => {
  return (
    <div className="w-full h-full">
      <StudentTrayWindow />
    </div>
  );
};

export default StudentTrayApp;
