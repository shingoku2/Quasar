import React from 'react';
import {
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  AreaChart,
  Area,
} from 'recharts';
import { cn } from '../../lib/utils';

interface MetricChartCardProps {
  title: string;
  value: string;
  data: Record<string, number | string>[];
  color?: string;
  className?: string;
}

const MetricChartCard: React.FC<MetricChartCardProps> = ({ 
  title, 
  value, 
  data, 
  color = "#3b82f6",
  className
}) => {
  // linearGradient ids are referenced via `url(#...)`, so they must not contain
  // characters (spaces, slashes) that break the url() token.
  const gradientId = `color-${title.replace(/[^a-zA-Z0-9_-]/g, '-')}`;

  return (
    <div className={cn("bg-bg-card border border-gray-800 rounded-xl p-4 flex flex-col shadow-sm", className)}>
      <div className="flex justify-between items-start mb-4">
        <div>
          <h3 className="text-xs font-bold text-gray-500 uppercase tracking-widest leading-none">{title}</h3>
          <p className="text-2xl font-black text-white mt-1 tracking-tighter">{value}</p>
        </div>
        <div className="h-2 w-2 rounded-full animate-pulse mt-1" style={{ backgroundColor: color }} />
      </div>

      <div className="flex-1 min-h-[150px] w-full mt-2">
        <ResponsiveContainer width="100%" height="100%">
          <AreaChart data={data}>
            <defs>
              <linearGradient id={gradientId} x1="0" y1="0" x2="0" y2="1">
                <stop offset="5%" stopColor={color} stopOpacity={0.3}/>
                <stop offset="95%" stopColor={color} stopOpacity={0}/>
              </linearGradient>
            </defs>
            <CartesianGrid strokeDasharray="3 3" vertical={false} stroke="#333" opacity={0.5} />
            <Tooltip 
              contentStyle={{ backgroundColor: '#1a1b21', border: '1px solid #333', borderRadius: '8px', fontSize: '12px' }}
              itemStyle={{ color: '#fff' }}
            />
            <Area 
              type="monotone" 
              dataKey="value" 
              stroke={color} 
              fillOpacity={1} 
              fill={`url(#${gradientId})`}
              strokeWidth={2}
              isAnimationActive={true}
            />
          </AreaChart>
        </ResponsiveContainer>
      </div>
    </div>
  );
};

export default MetricChartCard;
