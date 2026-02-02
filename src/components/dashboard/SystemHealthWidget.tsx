import React, { useState, useEffect } from 'react';
import { Server, AlertTriangle, Cpu, HardDrive } from 'lucide-react';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { cn } from '../../lib/utils';

interface SystemMetrics {
  cpu_usage_percent: number;
  memory_usage_percent: number;
  disk_usage_percent: number;
  uptime_seconds: number;
}

interface Alert {
  id: string;
  severity: string;
}

const SystemHealthWidget: React.FC = () => {
  const [metrics, setMetrics] = useState<SystemMetrics | null>(null);
  const [alerts, setAlerts] = useState<Alert[]>([]);

  useEffect(() => {
    // Listen for system metrics
    const unlistenMetrics = listen<SystemMetrics>('system-metrics', (event) => {
      setMetrics(event.payload);
    });

    // Listen for alerts
    const unlistenAlerts = listen<Alert[]>('alerts-triggered', (event) => {
      setAlerts(prev => [...event.payload, ...prev].slice(0, 50));
    });

    // Initial fetch
    invoke<SystemMetrics>('get_system_metrics').then(setMetrics).catch(console.error);

    return () => {
      unlistenMetrics.then(fn => fn());
      unlistenAlerts.then(fn => fn());
    };
  }, []);

  // Calculate health score based on metrics (inverted - lower usage = better health)
  const calculateHealthScore = (): number => {
    if (!metrics) return 100;
    
    // Calculate how "healthy" each metric is (100% - usage%)
    const cpuHealth = Math.max(0, 100 - metrics.cpu_usage_percent);
    const memHealth = Math.max(0, 100 - metrics.memory_usage_percent);
    const diskHealth = Math.max(0, 100 - metrics.disk_usage_percent);
    
    // Average the health scores
    return Math.round((cpuHealth + memHealth + diskHealth) / 3);
  };

  const healthScore = calculateHealthScore();
  
  // Determine status based on health score (higher is better)
  const statusText = healthScore >= 80 ? 'SYSTEMS NOMINAL' : 
                     healthScore >= 60 ? 'DEGRADED PERFORMANCE' : 
                     'CRITICAL STATUS';

  // Generate detailed status reason
  const getStatusReason = (): string => {
    if (!metrics) return 'Waiting for metrics...';
    
    const issues: string[] = [];
    
    if (metrics.cpu_usage_percent > 80) {
      issues.push(`CPU at ${metrics.cpu_usage_percent.toFixed(0)}%`);
    }
    if (metrics.memory_usage_percent > 85) {
      issues.push(`Memory at ${metrics.memory_usage_percent.toFixed(0)}%`);
    }
    if (metrics.disk_usage_percent > 90) {
      issues.push(`Disk at ${metrics.disk_usage_percent.toFixed(0)}%`);
    }
    
    if (issues.length > 0) {
      return issues.join(' • ');
    }
    
    // If no critical issues but health is degraded, show moderate usage
    if (healthScore < 80) {
      const moderateIssues: string[] = [];
      if (metrics.cpu_usage_percent > 40) {
        moderateIssues.push(`CPU ${metrics.cpu_usage_percent.toFixed(0)}%`);
      }
      if (metrics.memory_usage_percent > 50) {
        moderateIssues.push(`Memory ${metrics.memory_usage_percent.toFixed(0)}%`);
      }
      if (metrics.disk_usage_percent > 60) {
        moderateIssues.push(`Disk ${metrics.disk_usage_percent.toFixed(0)}%`);
      }
      
      if (moderateIssues.length > 0) {
        return moderateIssues.join(' • ');
      }
    }
    
    return 'All systems operating normally';
  };

  const statusReason = getStatusReason();
  const activeAlertCount = alerts.filter(a => !a.id.includes('acknowledged')).length;

  // Create metric-based "nodes" for display
  const nodes = [
    { 
      id: 'CPU', 
      status: (metrics?.cpu_usage_percent || 0) > 80 ? 'warning' : 'online',
      icon: Cpu,
      value: `${(metrics?.cpu_usage_percent || 0).toFixed(0)}%`
    },
    { 
      id: 'RAM', 
      status: (metrics?.memory_usage_percent || 0) > 85 ? 'warning' : 'online',
      icon: Server,
      value: `${(metrics?.memory_usage_percent || 0).toFixed(0)}%`
    },
    { 
      id: 'Disk', 
      status: (metrics?.disk_usage_percent || 0) > 90 ? 'warning' : 'online',
      icon: HardDrive,
      value: `${(metrics?.disk_usage_percent || 0).toFixed(0)}%`
    },
  ];

  return (
    <div className="bg-bg-card border border-gray-800 rounded-xl p-4 flex items-center justify-between shadow-sm">
      <div className="flex items-center space-x-4">
        <div className={cn(
          "h-12 w-12 rounded-full flex items-center justify-center border-4 shadow-inner transition-all duration-1000",
          healthScore >= 80 ? "border-green-500/20 text-green-500" : 
          healthScore >= 60 ? "border-warning/20 text-warning" : "border-alert/20 text-alert"
        )}>
          <span className="text-lg font-black">{healthScore}%</span>
        </div>
        <div className="flex flex-col">
          <h3 className="text-sm font-bold text-gray-400 uppercase tracking-wider leading-none">System Health</h3>
          <p className={cn(
            "text-xl font-black mt-1 tracking-tight leading-tight",
            healthScore >= 80 ? "text-green-500" : 
            healthScore >= 60 ? "text-warning" : "text-alert"
          )}>{statusText}</p>
          <p className="text-sm text-gray-100 mt-2 leading-tight font-normal">
            {statusReason}
          </p>
        </div>
      </div>

      <div className="hidden md:flex items-center">
        <div className="text-right border-l border-gray-800 pl-6">
          <p className="text-[10px] font-bold text-gray-500 uppercase tracking-widest">Active Alerts</p>
          <div className={cn(
            "flex items-center mt-1",
            activeAlertCount > 0 ? "text-alert" : "text-gray-500"
          )}>
            <AlertTriangle className="h-4 w-4 mr-1.5" />
            <span className="text-lg font-black leading-none">{activeAlertCount}</span>
          </div>
        </div>
      </div>
    </div>
  );
};

export default SystemHealthWidget;
