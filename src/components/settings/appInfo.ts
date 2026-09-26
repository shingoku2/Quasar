/** `get_app_info` result. */
export interface AppInfo {
  app_data_dir: string;
  db_path: string;
  db_size_bytes: number | null;
  version: string;
  platform: string;
  arch: string;
}
