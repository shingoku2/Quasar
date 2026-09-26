import React from 'react';
import { Bell } from 'lucide-react';

const NotificationsSettings: React.FC = () => (
  <div className="flex flex-col h-full bg-bg-root">
    <div className="p-6 border-b border-border">
      <div className="flex items-center space-x-2">
        <Bell className="h-5 w-5 text-accent" />
        <h2 className="text-xl font-bold text-white">Notifications</h2>
      </div>
    </div>
    <div className="flex-1 overflow-auto p-6">
      <div className="max-w-2xl bg-bg-card border border-border rounded-lg p-6 space-y-3 text-sm text-gray-300">
        <p>
          Triggered and recovered alerts appear in the <span className="text-white font-medium">Alert Feed</span> on
          the Dashboard. Configure the rules under <span className="text-white font-medium">Monitoring → Alert Rules</span>.
        </p>
        <p className="text-gray-500">Desktop, email and sound notifications aren't available yet.</p>
      </div>
    </div>
  </div>
);

export default NotificationsSettings;
