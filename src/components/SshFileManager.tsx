import React, { useState, useEffect, useCallback, useRef } from 'react';
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import {
  Folder,
  ArrowLeft,
  Download,
  Upload,
  RefreshCw,
  ChevronRight,
  HardDrive,
  FileText,
  Loader2,
  AlertCircle
} from 'lucide-react';
import { cn } from '../lib/utils';

interface RemoteFile {
  name: string;
  is_dir: boolean;
  size: number;
  permissions?: number;
  modified?: number;
}

interface SshFileManagerProps {
  host: string;
  port: number;
  username: string;
  password?: string;
}

const SshFileManager: React.FC<SshFileManagerProps> = ({
  host, 
  port, 
  username, 
  password 
}) => {
  const [currentPath, setCurrentPath] = useState('/');
  const [files, setFiles] = useState<RemoteFile[]>([]);
  const [loading, setLoading] = useState(false);
  const [transferring, setTransferring] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [selectedFile, setSelectedFile] = useState<string | null>(null);
  const isMounted = useRef(true);

  useEffect(() => {
    isMounted.current = true;
    return () => {
      isMounted.current = false;
    };
  }, []);

  const fetchFiles = useCallback(async (path: string) => {
    if (!password) return;
    
    if (isMounted.current) {
      setLoading(true);
      setError(null);
    }
    
    try {
      const result = await invoke<RemoteFile[]>('sftp_list_directory', {
        host,
        port,
        username,
        password,
        remotePath: path
      });
      
      if (isMounted.current) {
        // Sort: Directories first, then alphabetically
        const sortedFiles = result.sort((a, b) => {
          if (a.is_dir && !b.is_dir) return -1;
          if (!a.is_dir && b.is_dir) return 1;
          return a.name.localeCompare(b.name);
        });
        
        setFiles(sortedFiles);
        setCurrentPath(path);
      }
    } catch (err) {
      console.error('Failed to list directory:', err);
      if (isMounted.current) {
        setError(err as string);
      }
    } finally {
      if (isMounted.current) {
        setLoading(false);
      }
    }
  }, [host, port, username, password]);

  useEffect(() => {
    fetchFiles('/');
  }, [fetchFiles]);

  const handleNavigate = (fileName: string) => {
    const newPath = currentPath === '/' 
      ? `/${fileName}` 
      : `${currentPath}/${fileName}`;
    fetchFiles(newPath);
  };

  const handleBack = () => {
    if (currentPath === '/') return;
    const parts = currentPath.split('/').filter(Boolean);
    parts.pop();
    const newPath = '/' + parts.join('/');
    fetchFiles(newPath);
  };

  const handleUpload = async () => {
    if (!password) return;
    
    try {
      const selected = await open({
        multiple: false,
        directory: false,
      });
      
      if (selected) {
        const localPath = selected as string;
        const fileName = localPath.split(/[\\/]/).pop();
        const remotePath = currentPath === '/' ? `/${fileName}` : `${currentPath}/${fileName}`;
        
        if (isMounted.current) {
          setTransferring(`Uploading ${fileName}...`);
        }
        
        await invoke('sftp_upload_file', {
          host,
          port,
          username,
          password,
          localPath,
          remotePath
        });
        
        await fetchFiles(currentPath);
      }
    } catch (err) {
      console.error('Upload failed:', err);
      if (isMounted.current) {
        setError(`Upload failed: ${err}`);
      }
    } finally {
      if (isMounted.current) {
        setTransferring(null);
      }
    }
  };

  const handleDownload = async (file: RemoteFile) => {
    if (!password) return;
    
    try {
      const localPath = await save({
        defaultPath: file.name,
      });
      
      if (localPath) {
        const remotePath = currentPath === '/' ? `/${file.name}` : `${currentPath}/${file.name}`;
        
        if (isMounted.current) {
          setTransferring(`Downloading ${file.name}...`);
        }
        
        await invoke('sftp_download_file', {
          host,
          port,
          username,
          password,
          remotePath,
          localPath
        });
      }
    } catch (err) {
      console.error('Download failed:', err);
      if (isMounted.current) {
        setError(`Download failed: ${err}`);
      }
    } finally {
      if (isMounted.current) {
        setTransferring(null);
      }
    }
  };
  const formatBytes = (bytes: number) => {
    if (bytes === 0) return '0 Bytes';
    const k = 1024;
    const sizes = ['Bytes', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  };

  const formatDate = (timestamp?: number) => {
    if (!timestamp) return 'Unknown';
    return new Date(timestamp * 1000).toLocaleString();
  };

  return (
    <div className="flex flex-col h-full bg-bg-root text-gray-200">
      {/* Toolbar */}
      <div className="p-4 border-b border-gray-800 flex items-center justify-between bg-bg-sidebar/50">
        <div className="flex items-center space-x-4 flex-1">
          <button 
            onClick={handleBack}
            disabled={currentPath === '/' || loading || !!transferring}
            className="p-2 hover:bg-gray-800 rounded-lg disabled:opacity-30 transition-colors"
            title="Go back"
          >
            <ArrowLeft className="h-4 w-4" />
          </button>
          
          <div className="flex items-center space-x-2 bg-bg-root border border-gray-700 rounded-lg px-3 py-1.5 flex-1 max-w-2xl overflow-hidden">
            <HardDrive className="h-4 w-4 text-gray-500 shrink-0" />
            <span className="text-sm font-mono truncate">{currentPath}</span>
          </div>

          <button 
            onClick={() => fetchFiles(currentPath)}
            disabled={loading || !!transferring}
            className="p-2 hover:bg-gray-800 rounded-lg transition-colors"
            title="Refresh"
          >
            <RefreshCw className={cn("h-4 w-4", (loading || !!transferring) && "animate-spin")} />
          </button>
        </div>

        <div className="flex items-center space-x-2">
          <button 
            onClick={handleUpload}
            disabled={loading || !!transferring}
            className="flex items-center space-x-2 px-3 py-1.5 bg-gray-800 hover:bg-gray-700 rounded-lg text-sm transition-colors border border-gray-700 disabled:opacity-50"
          >
            <Upload className="h-4 w-4" />
            <span>Upload</span>
          </button>
        </div>
      </div>

      {/* Main Content */}
      <div className="flex-1 overflow-auto relative">
        {transferring && (
          <div className="absolute inset-0 bg-bg-root/80 z-20 flex flex-col items-center justify-center space-y-4">
            <Loader2 className="h-12 w-12 text-accent animate-spin" />
            <p className="text-white font-medium">{transferring}</p>
          </div>
        )}

        {error ? (
          <div className="flex flex-col items-center justify-center h-full space-y-4 p-8">
            <AlertCircle className="h-12 w-12 text-alert opacity-50" />
            <div className="text-center">
              <p className="text-white font-bold">Operation failed</p>
              <p className="text-gray-400 text-sm mt-1">{error}</p>
            </div>
            <button 
              onClick={() => { setError(null); fetchFiles(currentPath); }}
              className="px-4 py-2 bg-accent text-white rounded-lg text-sm font-bold"
            >
              Dismiss & Retry
            </button>
          </div>
        ) : loading && files.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-full space-y-4">
            <Loader2 className="h-12 w-12 text-accent animate-spin opacity-50" />
            <p className="text-gray-400 animate-pulse">Connecting to remote host...</p>
          </div>
        ) : (
          <table className="w-full text-left text-sm border-separate border-spacing-0">
            <thead className="sticky top-0 bg-bg-root z-10">
              <tr className="text-gray-400 border-b border-gray-800">
                <th className="px-4 py-2 font-medium border-b border-gray-800">Name</th>
                <th className="px-4 py-2 font-medium border-b border-gray-800 w-32">Size</th>
                <th className="px-4 py-2 font-medium border-b border-gray-800 w-48">Modified</th>
                <th className="px-4 py-2 font-medium border-b border-gray-800 w-24 text-right">Actions</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-gray-800/50">
              {files.map((file) => (
                <tr 
                  key={file.name} 
                  className={cn(
                    "hover:bg-accent/5 group transition-colors cursor-pointer",
                    selectedFile === file.name && "bg-accent/10"
                  )}
                  onClick={() => setSelectedFile(file.name)}
                  onDoubleClick={() => file.is_dir ? handleNavigate(file.name) : null}
                >
                  <td className="px-4 py-3">
                    <div className="flex items-center space-x-3">
                      {file.is_dir ? (
                        <Folder className="h-4 w-4 text-accent fill-accent/10" />
                      ) : (
                        <FileText className="h-4 w-4 text-gray-400" />
                      )}
                      <span className={cn(
                        "font-medium",
                        file.is_dir ? "text-gray-100" : "text-gray-300"
                      )}>
                        {file.name}
                      </span>
                      {file.is_dir && (
                        <ChevronRight className="h-3 w-3 text-gray-600 opacity-0 group-hover:opacity-100 transition-opacity" />
                      )}
                    </div>
                  </td>
                  <td className="px-4 py-3 text-gray-500 font-mono">
                    {file.is_dir ? '--' : formatBytes(file.size)}
                  </td>
                  <td className="px-4 py-3 text-gray-500">
                    {formatDate(file.modified)}
                  </td>
                  <td className="px-4 py-3 text-right">
                    {!file.is_dir && (
                      <button 
                        onClick={(e) => { e.stopPropagation(); handleDownload(file); }}
                        className="p-1.5 hover:bg-gray-800 rounded transition-colors text-gray-400 hover:text-white"
                        title="Download"
                        disabled={!!transferring}
                      >
                        <Download className="h-4 w-4" />
                      </button>
                    )}
                  </td>
                </tr>
              ))}
              {files.length === 0 && !loading && (
                <tr>
                  <td colSpan={4} className="px-4 py-12 text-center text-gray-500 italic">
                    This directory is empty
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        )}
      </div>

      {/* Footer / Selection Info */}
      <div className="p-2 border-t border-gray-800 bg-bg-sidebar/30 flex justify-between items-center text-xs text-gray-500">
        <div>
          {files.length} items
        </div>
        {selectedFile && (
          <div>
            Selected: <span className="text-gray-300">{selectedFile}</span>
          </div>
        )}
      </div>
    </div>
  );
};

export default SshFileManager;
