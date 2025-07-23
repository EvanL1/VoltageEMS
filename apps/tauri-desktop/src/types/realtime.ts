export interface ChannelStatus {
  channel_id: number;
  name: string;
  status: "online" | "offline" | "error";
  last_update?: Date;
  point_count: number;
  error_count: number;
}

export interface PointData {
  point_id: number;
  point_type: "YC" | "YX" | "YK" | "YT";
  value: any;
  timestamp: Date;
  description?: string;
}

export interface Statistics {
  total_channels: number;
  online_channels: number;
  offline_channels: number;
  total_points: number;
  total_errors: number;
  timestamp: Date;
}
