/**
 * useAutoUpdate Hook
 * 
 * Provides a React hook for managing auto-update functionality.
 * Subscribes to update state events and provides methods for check, download, install.
 * 
 * Requirements: All UI requirements (10.1, 10.2, 10.3, 10.4, 10.5, 10.6, 11.1-11.5)
 */

import { useState, useEffect, useCallback, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';

// Types matching Rust backend (src-tauri/src/auto_update/types.rs)
export interface UpdateInfo {
  version: string;
  published_at: string;
  download_url: string;
  sha256: string;
  signature?: string;
  release_notes: string;
  changelog_url?: string;
  min_app_version?: string;
}

export type UpdateState =
  | { type: 'Idle' }
  | { type: 'Checking' }
  | { type: 'UpdateAvailable'; data: { version: string; release_notes: string } }
  | { type: 'Downloading'; data: { progress: number; bytes_downloaded: number; total_bytes: number } }
  | { type: 'Verifying' }
  | { type: 'ReadyToInstall' }
  | { type: 'Installing' }
  | { type: 'Restarting' }
  | { type: 'Done' }
  | { type: 'Failed'; data: { error: string; recoverable: boolean } };

export interface DownloadProgress {
  bytes_downloaded: number;
  total_bytes: number;
  progress: number;
}

export interface StateChangeEvent {
  previous_state: UpdateState;
  new_state: UpdateState;
  timestamp: number;
}

export interface ClientUpdateStatus {
  client_id: string;
  machine_name?: string;
  ip: string;
  current_version?: string;
  status: string;
  progress?: number;
  last_updated: number;
}

// Student update state types
export interface StudentUpdateState {
  type: 'Idle' | 'UpdateRequired' | 'Downloading' | 'Verifying' | 'ReadyToInstall' | 'Installing' | 'Restarting' | 'Done' | 'Failed';
  data?: {
    current_version?: string;
    required_version?: string;
    update_url?: string;
    sha256?: string;
    progress?: number;
    bytes_downloaded?: number;
    total_bytes?: number;
    retry_count?: number;
    download_path?: string;
    error?: string;
    can_retry?: boolean;
  };
}

export interface UseAutoUpdateOptions {
  /** Whether to automatically check for updates on mount */
  autoCheck?: boolean;
  /** Interval in milliseconds to poll update state (default: 1000) */
  pollInterval?: number;
  /** Interval in milliseconds to poll client statuses (default: 2000) */
  clientPollInterval?: number;
}

export interface UseAutoUpdateReturn {
  // State
  updateState: UpdateState;
  updateInfo: UpdateInfo | null;
  clientStatuses: ClientUpdateStatus[];
  isChecking: boolean;
  isDownloading: boolean;
  isInstalling: boolean;
  error: string | null;
  
  // Actions
  checkForUpdates: () => Promise<UpdateInfo | null>;
  downloadUpdate: () => Promise<void>;
  installUpdate: () => Promise<void>;
  restartForUpdate: () => Promise<void>;
  retry: () => Promise<void>;
  resetUpdateState: () => Promise<void>;
  clearError: () => void;
  
  // LAN Distribution (Teacher only)
  startLanDistribution: () => Promise<string>;
  stopLanDistribution: () => Promise<void>;
}

/**
 * Hook for managing auto-update functionality
 * 
 * @param options - Configuration options
 * @returns Update state and control methods
 */
export function useAutoUpdate(options: UseAutoUpdateOptions = {}): UseAutoUpdateReturn {
  const {
    autoCheck = false,
    pollInterval = 1000,
    clientPollInterval = 2000,
  } = options;

  // State
  const [updateState, setUpdateState] = useState<UpdateState>({ type: 'Idle' });
  const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null);
  const [clientStatuses, setClientStatuses] = useState<ClientUpdateStatus[]>([]);
  const [isChecking, setIsChecking] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Refs for cleanup
  const unlistenStateRef = useRef<UnlistenFn | null>(null);
  const unlistenProgressRef = useRef<UnlistenFn | null>(null);
  const pollIntervalRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const clientPollIntervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  // Derived state
  const isDownloading = updateState.type === 'Downloading';
  const isInstalling = updateState.type === 'Installing' || updateState.type === 'Restarting';

  // Poll update state
  useEffect(() => {
    const pollState = async () => {
      try {
        const state = await invoke<UpdateState>('get_update_state');
        setUpdateState(state);
      } catch (err) {
        console.error('[useAutoUpdate] Failed to get update state:', err);
      }
    };

    // Initial poll
    pollState();

    // Set up interval
    pollIntervalRef.current = setInterval(pollState, pollInterval);

    return () => {
      if (pollIntervalRef.current) {
        clearInterval(pollIntervalRef.current);
      }
    };
  }, [pollInterval]);

  // Poll client update statuses
  useEffect(() => {
    const pollClients = async () => {
      try {
        const statuses = await invoke<ClientUpdateStatus[]>('get_client_update_status');
        setClientStatuses(statuses);
      } catch (err) {
        // Silently fail - this is expected for student apps
        console.debug('[useAutoUpdate] Failed to get client statuses:', err);
      }
    };

    // Initial poll
    pollClients();

    // Set up interval
    clientPollIntervalRef.current = setInterval(pollClients, clientPollInterval);

    return () => {
      if (clientPollIntervalRef.current) {
        clearInterval(clientPollIntervalRef.current);
      }
    };
  }, [clientPollInterval]);

  // Listen for update state change events
  useEffect(() => {
    const setupListeners = async () => {
      try {
        // Listen for state changes
        unlistenStateRef.current = await listen<StateChangeEvent>('update-state-changed', (event) => {
          console.log('[useAutoUpdate] State changed:', event.payload);
          setUpdateState(event.payload.new_state);
        });

        // Listen for download progress
        unlistenProgressRef.current = await listen<DownloadProgress>('update-download-progress', (event) => {
          console.log('[useAutoUpdate] Download progress:', event.payload);
          setUpdateState({
            type: 'Downloading',
            data: {
              progress: event.payload.progress,
              bytes_downloaded: event.payload.bytes_downloaded,
              total_bytes: event.payload.total_bytes,
            },
          });
        });
      } catch (err) {
        console.error('[useAutoUpdate] Failed to set up listeners:', err);
      }
    };

    setupListeners();

    return () => {
      if (unlistenStateRef.current) {
        unlistenStateRef.current();
      }
      if (unlistenProgressRef.current) {
        unlistenProgressRef.current();
      }
    };
  }, []);

  // Auto-check on mount if enabled
  useEffect(() => {
    if (autoCheck) {
      checkForUpdates();
    }
  }, [autoCheck]);

  /**
   * Check for updates from the Update API
   * Requirements: 2.1
   */
  const checkForUpdates = useCallback(async (): Promise<UpdateInfo | null> => {
    setIsChecking(true);
    setError(null);

    try {
      const info = await invoke<UpdateInfo | null>('check_for_updates');
      if (info) {
        setUpdateInfo(info);
      }
      return info;
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      console.error('[useAutoUpdate] Failed to check for updates:', errorMessage);
      setError(errorMessage);
      return null;
    } finally {
      setIsChecking(false);
    }
  }, []);

  /**
   * Download the update package
   * Requirements: 3.1
   */
  const downloadUpdate = useCallback(async (): Promise<void> => {
    setError(null);

    try {
      await invoke('download_update');
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      console.error('[useAutoUpdate] Failed to download update:', errorMessage);
      setError(errorMessage);
      throw err;
    }
  }, []);

  /**
   * Install the verified update
   * Requirements: 4.1
   */
  const installUpdate = useCallback(async (): Promise<void> => {
    setError(null);

    try {
      await invoke('install_update');
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      console.error('[useAutoUpdate] Failed to install update:', errorMessage);
      setError(errorMessage);
      throw err;
    }
  }, []);

  /**
   * Restart the application for update
   * Requirements: 4.3
   */
  const restartForUpdate = useCallback(async (): Promise<void> => {
    setError(null);

    try {
      await invoke('restart_for_update');
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      console.error('[useAutoUpdate] Failed to restart for update:', errorMessage);
      setError(errorMessage);
      throw err;
    }
  }, []);

  /**
   * Retry the last failed operation
   */
  const retry = useCallback(async (): Promise<void> => {
    setError(null);

    // Retry by checking for updates again
    await checkForUpdates();
  }, [checkForUpdates]);

  /**
   * Reset the update state to idle
   */
  const resetUpdateState = useCallback(async (): Promise<void> => {
    try {
      await invoke('reset_update_state');
      setError(null);
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      console.error('[useAutoUpdate] Failed to reset update state:', errorMessage);
    }
  }, []);

  /**
   * Clear the current error
   */
  const clearError = useCallback(() => {
    setError(null);
  }, []);

  /**
   * Start the LAN distribution server (Teacher only)
   * Requirements: 7.1
   */
  const startLanDistribution = useCallback(async (): Promise<string> => {
    try {
      const url = await invoke<string>('start_lan_distribution');
      return url;
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      console.error('[useAutoUpdate] Failed to start LAN distribution:', errorMessage);
      setError(errorMessage);
      throw err;
    }
  }, []);

  /**
   * Stop the LAN distribution server (Teacher only)
   * Requirements: 7.5
   */
  const stopLanDistribution = useCallback(async (): Promise<void> => {
    try {
      await invoke('stop_lan_distribution');
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      console.error('[useAutoUpdate] Failed to stop LAN distribution:', errorMessage);
      setError(errorMessage);
      throw err;
    }
  }, []);

  return {
    // State
    updateState,
    updateInfo,
    clientStatuses,
    isChecking,
    isDownloading,
    isInstalling,
    error,

    // Actions
    checkForUpdates,
    downloadUpdate,
    installUpdate,
    restartForUpdate,
    retry,
    resetUpdateState,
    clearError,

    // LAN Distribution
    startLanDistribution,
    stopLanDistribution,
  };
}

/**
 * Hook for student update functionality
 * Provides state and methods specific to student update flow
 */
export function useStudentUpdate() {
  const [updateState, setUpdateState] = useState<StudentUpdateState>({ type: 'Idle' });
  const [error, setError] = useState<string | null>(null);
  const unlistenRef = useRef<UnlistenFn | null>(null);

  // Derived state
  const isUpdateRequired = updateState.type === 'UpdateRequired' ||
    updateState.type === 'Downloading' ||
    updateState.type === 'Verifying' ||
    updateState.type === 'ReadyToInstall' ||
    updateState.type === 'Installing' ||
    updateState.type === 'Restarting' ||
    updateState.type === 'Failed';

  // Get initial state and listen for changes
  useEffect(() => {
    const setup = async () => {
      try {
        // Get initial state
        const state = await invoke<StudentUpdateState>('get_student_update_state');
        setUpdateState(state);

        // Listen for state changes
        unlistenRef.current = await listen<{ state: StudentUpdateState; timestamp: number }>(
          'student-update-state-changed',
          (event) => {
            console.log('[useStudentUpdate] State changed:', event.payload);
            setUpdateState(event.payload.state);
          }
        );
      } catch (err) {
        console.error('[useStudentUpdate] Failed to setup:', err);
      }
    };

    setup();

    return () => {
      if (unlistenRef.current) {
        unlistenRef.current();
      }
    };
  }, []);

  /**
   * Download update from Teacher's LAN server
   * Requirements: 8.1, 8.3
   */
  const downloadUpdate = useCallback(async (): Promise<void> => {
    setError(null);

    try {
      await invoke('download_student_update');
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      console.error('[useStudentUpdate] Failed to download update:', errorMessage);
      setError(errorMessage);
      throw err;
    }
  }, []);

  /**
   * Retry student update download after failure
   * Requirements: 8.5, 11.4
   */
  const retryDownload = useCallback(async (): Promise<void> => {
    setError(null);

    try {
      await invoke('retry_student_update');
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      console.error('[useStudentUpdate] Failed to retry download:', errorMessage);
      setError(errorMessage);
      throw err;
    }
  }, []);

  /**
   * Install the student update
   * Requirements: 9.1
   */
  const installUpdate = useCallback(async (): Promise<void> => {
    setError(null);

    try {
      await invoke('install_student_update');
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      console.error('[useStudentUpdate] Failed to install update:', errorMessage);
      setError(errorMessage);
      throw err;
    }
  }, []);

  /**
   * Clear the current error
   */
  const clearError = useCallback(() => {
    setError(null);
  }, []);

  return {
    updateState,
    isUpdateRequired,
    error,
    downloadUpdate,
    retryDownload,
    installUpdate,
    clearError,
  };
}

export default useAutoUpdate;
