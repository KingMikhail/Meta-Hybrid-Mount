
export const APP_VERSION = "v3.0.1-5-g950a83e-dirty";
export const RUST_PATHS = {
  CONFIG: "/data/adb/Meta-Hybrid/config.toml",
  MODE_CONFIG: "/data/adb/Meta-Hybrid/module_mode.conf",
  IMAGE_MNT: "/data/adb/meta-Hybrid/mnt",
  DAEMON_STATE: "/data/adb/Meta-Hybrid/run/daemon_state.json",
  DAEMON_LOG: "/data/adb/Meta-Hybrid/daemon.log",
} as const;
export const BUILTIN_PARTITIONS = ["system", "vendor", "product", "system_ext", "odm", "oem", "apex"] as const;
export const IS_RELEASE = false;
