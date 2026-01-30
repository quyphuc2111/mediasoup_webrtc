export enum UserRole {
  ADMIN = 'Administrator',
  TEACHER = 'Teacher',
  STUDENT = 'Student'
}

export interface UserAccount {
  userId: number;
  userName: string;
  role: UserRole;
  status: boolean;
}

export interface SchoolYear {
  schoolYearId: number;
  schoolYearName: string;
  startDate?: string;
  endDate?: string;
}

export interface Grade {
  gradeId: number;
  gradeName: string;
  schoolYearId: number;
}

export interface Subject {
  subjectId: number;
  subjectName: string;
  schoolYearId: number;
}

export interface PracticeTimeSlot {
  practiceTimeSlotId: number;
  practiceTimeSlotName: string;
  schoolYearId: number;
  startTime: string;
  endTime: string;
}

export interface RoomComputer {
  roomComputerId: number;
  computerName: string;
  ipAddress: string;
  status: 'Active' | 'Repairing' | 'Broken';
}

export interface PracticeSession {
  practiceSessionId: number;
  practiceSessionName: string;
  createdByUserId: number;
  gradeId: number;
  subjectId: number;
  schoolYearId: number;
  practiceTimeSlotId: number;
  status: boolean;
  createdAt: string;
}

export interface Message {
  id: string;
  senderId: string;
  senderName: string;
  content: string;
  timestamp: string;
  type?: 'Direct' | 'All';
  role?: UserRole;
}
