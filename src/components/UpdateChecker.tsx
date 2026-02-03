import { useEffect, useState } from 'react';
import { check } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';

export function UpdateChecker() {
  const [updateAvailable, setUpdateAvailable] = useState(false);
  const [updateVersion, setUpdateVersion] = useState('');
  const [downloading, setDownloading] = useState(false);
  const [downloadProgress, setDownloadProgress] = useState(0);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    checkForUpdates();
    
    // Check for updates every hour
    const interval = setInterval(checkForUpdates, 60 * 60 * 1000);
    return () => clearInterval(interval);
  }, []);

  const checkForUpdates = async () => {
    try {
      console.log('[Updater] Checking for updates...');
      const update = await check();
      
      if (update?.available) {
        console.log('[Updater] Update available:', update.version);
        setUpdateAvailable(true);
        setUpdateVersion(update.version);
      } else {
        console.log('[Updater] No updates available');
      }
    } catch (err) {
      console.error('[Updater] Check failed:', err);
      setError(err instanceof Error ? err.message : 'Failed to check for updates');
    }
  };

  const downloadAndInstall = async () => {
    try {
      setDownloading(true);
      setError(null);
      
      console.log('[Updater] Starting download...');
      const update = await check();
      
      if (!update?.available) {
        setError('No update available');
        setDownloading(false);
        return;
      }

      // Download with progress
      await update.downloadAndInstall((event) => {
        switch (event.event) {
          case 'Started':
            console.log('[Updater] Download started');
            setDownloadProgress(0);
            break;
          case 'Progress':
            const progress = Math.round((event.data.chunkLength / event.data.contentLength!) * 100);
            console.log(`[Updater] Download progress: ${progress}%`);
            setDownloadProgress(progress);
            break;
          case 'Finished':
            console.log('[Updater] Download finished');
            setDownloadProgress(100);
            break;
        }
      });

      console.log('[Updater] Update installed, restarting...');
      
      // Relaunch app
      await relaunch();
    } catch (err) {
      console.error('[Updater] Download/install failed:', err);
      setError(err instanceof Error ? err.message : 'Failed to install update');
      setDownloading(false);
    }
  };

  if (error) {
    return (
      <div style={{
        position: 'fixed',
        bottom: 20,
        right: 20,
        backgroundColor: '#fee',
        border: '1px solid #fcc',
        borderRadius: 8,
        padding: 16,
        maxWidth: 400,
        boxShadow: '0 4px 12px rgba(0,0,0,0.15)',
        zIndex: 9999
      }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 8 }}>
          <span style={{ fontSize: 20 }}>⚠️</span>
          <strong style={{ color: '#991b1b' }}>Update Error</strong>
        </div>
        <p style={{ margin: 0, fontSize: 14, color: '#991b1b' }}>{error}</p>
        <button
          onClick={() => setError(null)}
          style={{
            marginTop: 12,
            padding: '6px 12px',
            backgroundColor: '#fff',
            border: '1px solid #ddd',
            borderRadius: 4,
            cursor: 'pointer',
            color: '#374151',
            fontWeight: 500
          }}
        >
          Dismiss
        </button>
      </div>
    );
  }

  if (!updateAvailable) {
    return null;
  }

  return (
    <div style={{
      position: 'fixed',
      bottom: 20,
      right: 20,
      backgroundColor: '#fff',
      border: '1px solid #ddd',
      borderRadius: 8,
      padding: 16,
      maxWidth: 400,
      boxShadow: '0 4px 12px rgba(0,0,0,0.15)',
      zIndex: 9999
    }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 8 }}>
        <span style={{ fontSize: 20 }}>🎉</span>
        <strong style={{ color: '#1f2937' }}>Update Available</strong>
      </div>
      
      <p style={{ margin: '8px 0', fontSize: 14, color: '#4b5563' }}>
        Version {updateVersion} is now available. Would you like to update?
      </p>

      {downloading && (
        <div style={{ marginTop: 12 }}>
          <div style={{
            width: '100%',
            height: 8,
            backgroundColor: '#f0f0f0',
            borderRadius: 4,
            overflow: 'hidden'
          }}>
            <div style={{
              width: `${downloadProgress}%`,
              height: '100%',
              backgroundColor: '#4CAF50',
              transition: 'width 0.3s ease'
            }} />
          </div>
          <p style={{ margin: '8px 0 0', fontSize: 12, color: '#6b7280', textAlign: 'center' }}>
            Downloading... {downloadProgress}%
          </p>
        </div>
      )}

      <div style={{ display: 'flex', gap: 8, marginTop: 12 }}>
        <button
          onClick={downloadAndInstall}
          disabled={downloading}
          style={{
            flex: 1,
            padding: '8px 16px',
            backgroundColor: downloading ? '#ccc' : '#4CAF50',
            color: '#fff',
            border: 'none',
            borderRadius: 4,
            cursor: downloading ? 'not-allowed' : 'pointer',
            fontWeight: 500
          }}
        >
          {downloading ? 'Downloading...' : 'Update Now'}
        </button>
        <button
          onClick={() => setUpdateAvailable(false)}
          disabled={downloading}
          style={{
            padding: '8px 16px',
            backgroundColor: '#fff',
            border: '1px solid #ddd',
            borderRadius: 4,
            cursor: downloading ? 'not-allowed' : 'pointer',
            color: '#374151',
            fontWeight: 500
          }}
        >
          Later
        </button>
      </div>
    </div>
  );
}
