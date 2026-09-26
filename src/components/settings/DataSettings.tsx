import React, { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Database } from 'lucide-react';
import { getErrorMessage } from '../../lib/utils';
import type { AppInfo } from './appInfo';

const METRICS_RETENTION_DAYS = 30;

const DataSettings: React.FC = () => {
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const [success, setSuccess] = useState('');
  const [exporting, setExporting] = useState(false);
  const [importing, setImporting] = useState(false);
  const [clearing, setClearing] = useState(false);

  const loadInfo = useCallback(async () => {
    setLoading(true);
    setError('');
    try {
      const info = await invoke<AppInfo>('get_app_info');
      setAppInfo(info);
    } catch (err) {
      setError(getErrorMessage(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadInfo();
  }, [loadInfo]);

  const handleExport = async () => {
    // The backend opens the dialog and only accepts paths chosen there (IPC-001).
    const path = await invoke<string | null>('pick_save_location', {
      defaultName: `quasar-backup-${new Date().toISOString().slice(0, 10)}.db`,
      extensions: ['db'],
    });
    if (!path) return;
    setExporting(true);
    setError('');
    setSuccess('');
    try {
      await invoke('export_database', { destPath: path });
      setSuccess('Database exported successfully.');
      setTimeout(() => setSuccess(''), 3000);
      await loadInfo();
    } catch (err) {
      setError(getErrorMessage(err));
    } finally {
      setExporting(false);
    }
  };

  const handleImport = async () => {
    const path = await invoke<string | null>('pick_local_file', {
      title: 'Choose a Quasar backup to import',
      extensions: ['db'],
    });
    if (!path) return;
    setImporting(true);
    setError('');
    setSuccess('');
    try {
      await invoke('import_database', { sourcePath: path });
      setSuccess('Database imported. The vault is locked: unlock it with the imported vault\'s master password, then restart the app.');
      setTimeout(() => setSuccess(''), 5000);
      await loadInfo();
    } catch (err) {
      setError(getErrorMessage(err));
    } finally {
      setImporting(false);
    }
  };

  const handleClearMetrics = async () => {
    if (!window.confirm('Clear all metrics and alert history? This cannot be undone.')) return;
    setClearing(true);
    setError('');
    setSuccess('');
    try {
      await invoke('clear_metrics_data');
      setSuccess('Metrics and alert history cleared.');
      setTimeout(() => setSuccess(''), 3000);
      await loadInfo();
    } catch (err) {
      setError(getErrorMessage(err));
    } finally {
      setClearing(false);
    }
  };

  const dbSizeStr = appInfo?.db_size_bytes != null
    ? (appInfo.db_size_bytes < 1024
        ? `${appInfo.db_size_bytes} B`
        : appInfo.db_size_bytes < 1024 * 1024
          ? `${(appInfo.db_size_bytes / 1024).toFixed(1)} KB`
          : `${(appInfo.db_size_bytes / (1024 * 1024)).toFixed(2)} MB`)
    : '—';

  return (
    <div className="flex flex-col h-full bg-bg-root">
      <div className="p-6 border-b border-border">
        <div className="flex items-center space-x-2">
          <Database className="h-5 w-5 text-accent" />
          <h2 className="text-xl font-bold text-white">Data & Storage</h2>
        </div>
      </div>

      <div className="flex-1 overflow-auto p-6">
        <div className="max-w-2xl space-y-6">
          {error && (
            <div className="bg-alert/10 border border-alert/30 rounded-lg p-4 text-alert text-sm">
              {error}
            </div>
          )}
          {success && (
            <div className="bg-success/10 border border-success/30 rounded-lg p-4 text-success text-sm">
              {success}
            </div>
          )}

          <div className="bg-bg-card border border-border rounded-lg p-6">
            <h3 className="text-white font-bold mb-4">Database</h3>
            {loading ? (
              <p className="text-gray-400 text-sm">Loading…</p>
            ) : appInfo ? (
              <div className="space-y-3">
                <div className="flex justify-between items-center">
                  <span className="text-gray-400 text-sm">Database Location</span>
                  <span className="text-white text-sm font-mono truncate max-w-[60%]" title={appInfo.db_path}>
                    {appInfo.db_path}
                  </span>
                </div>
                <div className="flex justify-between items-center">
                  <span className="text-gray-400 text-sm">Database Size</span>
                  <span className="text-white text-sm">{dbSizeStr}</span>
                </div>
                <div className="flex justify-between items-center">
                  <span className="text-gray-400 text-sm">Metrics Retention</span>
                  <span className="text-white text-sm">{METRICS_RETENTION_DAYS} days</span>
                </div>
              </div>
            ) : (
              <p className="text-gray-400 text-sm">Unable to load database info.</p>
            )}
          </div>

          <div className="bg-bg-card border border-border rounded-lg p-6">
            <h3 className="text-white font-bold mb-4">Backup & Export</h3>
            <div className="space-y-3">
              <button
                type="button"
                onClick={handleExport}
                disabled={exporting}
                className="w-full bg-bg-root border border-border hover:border-accent text-white py-2.5 px-4 rounded-lg text-sm font-medium transition-all text-left disabled:opacity-50"
              >
                {exporting ? 'Exporting…' : 'Export Database Backup'}
              </button>
              <button
                type="button"
                onClick={handleImport}
                disabled={importing}
                className="w-full bg-bg-root border border-border hover:border-accent text-white py-2.5 px-4 rounded-lg text-sm font-medium transition-all text-left disabled:opacity-50"
              >
                {importing ? 'Importing…' : 'Import Database Backup'}
              </button>
              <button
                type="button"
                onClick={handleClearMetrics}
                disabled={clearing}
                className="w-full bg-bg-root border border-alert/50 hover:border-alert text-white py-2.5 px-4 rounded-lg text-sm font-medium transition-all text-left disabled:opacity-50"
              >
                {clearing ? 'Clearing…' : 'Clear All Metrics Data'}
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};

export default DataSettings;
