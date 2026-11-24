use serde::{Deserialize, Serialize};

use crate::types::{FlashPhase, ProgressState};
use crate::units::pretty_bytes;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Locale {
    #[default]
    En,
    ZhCn,
    ZhTw,
}

impl Locale {
    pub fn code(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::ZhCn => "zh-CN",
            Self::ZhTw => "zh-TW",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::En => "English",
            Self::ZhCn => "简体中文",
            Self::ZhTw => "繁體中文",
        }
    }

    pub fn from_code(code: &str) -> Self {
        let normalized = code.trim().replace('_', "-").to_ascii_lowercase();
        if normalized == "zh-cn" || normalized == "zh-sg" || normalized == "zh-hans" {
            Self::ZhCn
        } else if normalized.starts_with("zh") {
            Self::ZhTw
        } else {
            Self::En
        }
    }

    pub fn detect_system() -> Self {
        for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
            if let Ok(val) = std::env::var(var)
                && !val.trim().is_empty()
            {
                let base = val.split('.').next().unwrap_or(&val);
                if base.to_ascii_lowercase().starts_with("zh") {
                    return Self::from_code(base);
                }
                return Self::En;
            }
        }
        Self::En
    }
}

pub struct Messages {
    pub cont: &'static str,
    pub ok: &'static str,
    pub cancel: &'static str,
    pub skip: &'static str,
    pub sure: &'static str,
    pub warning: &'static str,
    pub attention: &'static str,
    pub failed: &'static str,
    pub completed: &'static str,
    pub yes_continue: &'static str,
    pub really_exit: &'static str,
    pub yes_exit: &'static str,

    pub progress_starting: &'static str,
    pub progress_decompressing: &'static str,
    pub progress_flashing: &'static str,
    pub progress_finishing: &'static str,
    pub progress_verifying: &'static str,
    pub progress_failing: &'static str,

    pub compat_size_not_recommended: &'static str,
    pub compat_too_small: &'static str,
    pub compat_locked: &'static str,
    pub compat_system: &'static str,
    pub compat_contains_image: &'static str,
    pub compat_large_drive: &'static str,

    pub msg_exit_while_flashing: &'static str,
    pub msg_looks_like_windows_image: &'static str,
    pub msg_image: &'static str,
    pub msg_drive: &'static str,
    pub msg_large_drive_size: &'static str,
    pub msg_system_drive: &'static str,
    pub msg_source_drive: &'static str,
    pub msg_no_space: &'static str,
    pub msg_validation: &'static str,
    pub msg_unplug: &'static str,
    pub msg_cannot_write: &'static str,
    pub msg_child_writer_died: &'static str,
    pub msg_bad_protocol: &'static str,

    pub target_select_target: &'static str,
    pub target_plug_target: &'static str,
    pub target_targets: &'static str,
    pub target_change: &'static str,

    pub source_use_source_url: &'static str,
    pub source_auth: &'static str,
    pub source_username: &'static str,
    pub source_password: &'static str,
    pub source_unsupported_protocol: &'static str,
    pub source_windows_image: &'static str,
    pub source_partition_table: &'static str,
    pub source_error_open: &'static str,
    pub source_from_file: &'static str,
    pub source_from_url: &'static str,
    pub source_clone: &'static str,
    pub source_image: &'static str,
    pub source_name: &'static str,
    pub source_path: &'static str,
    pub source_select_source: &'static str,
    pub source_plug_source: &'static str,
    pub source_os_images: &'static str,
    pub source_all_files: &'static str,
    pub source_enter_valid_url: &'static str,
    pub source_recent: &'static str,

    pub drives_name: &'static str,
    pub drives_size: &'static str,
    pub drives_location: &'static str,
    pub drives_system_drive_danger: &'static str,
    pub drives_change_target: &'static str,
    pub drives_large_drive_warning: &'static str,
    pub drives_large_drive_warning_msg: &'static str,
    pub drives_system_drive_warning: &'static str,
    pub drives_system_drive_warning_msg: &'static str,
    pub drives_initializing: &'static str,

    pub flash_another: &'static str,
    pub flash_target: &'static str,
    pub flash_location: &'static str,
    pub flash_error: &'static str,
    pub flash_flash: &'static str,
    pub flash_flash_now: &'static str,
    pub flash_skip: &'static str,
    pub flash_more_info: &'static str,
    pub flash_speed_tip: &'static str,
    pub flash_failed_target: &'static str,
    pub flash_failed_retry: &'static str,
    pub flash_failed: &'static str,
    pub flash_completed: &'static str,

    pub settings_error_reporting: &'static str,
    pub settings_auto_update: &'static str,
    pub settings_desktop_notifications: &'static str,
    pub settings_settings: &'static str,
    pub settings_system_information: &'static str,
    pub settings_trim_ext_partitions: &'static str,
    pub settings_language: &'static str,

    pub privacy_line1_prefix: &'static str,
    pub privacy_settings_link: &'static str,
    pub privacy_line2_prefix: &'static str,
    pub privacy_policy_link: &'static str,
    pub retry: &'static str,
}

const EN_MESSAGES: Messages = Messages {
    cont: "Continue",
    ok: "OK",
    cancel: "Cancel",
    skip: "Skip",
    sure: "Yes, I'm sure",
    warning: "WARNING!",
    attention: "Attention",
    failed: "Failed",
    completed: "Completed",
    yes_continue: "Yes, continue",
    really_exit: "Are you sure you want to close Etcher?",
    yes_exit: "Yes, quit",

    progress_starting: "Starting...",
    progress_decompressing: "Decompressing...",
    progress_flashing: "Flashing...",
    progress_finishing: "Finishing...",
    progress_verifying: "Validating...",
    progress_failing: "Failed",

    compat_size_not_recommended: "Not recommended",
    compat_too_small: "Too small",
    compat_locked: "Locked",
    compat_system: "System drive",
    compat_contains_image: "Source drive",
    compat_large_drive: "Large drive",

    msg_exit_while_flashing: "You are currently flashing a drive. Closing Etcher may leave your drive in an unusable state.",
    msg_looks_like_windows_image: "It looks like you are trying to burn a Windows image.\n\nUnlike other images, Windows images require special processing to be made bootable. We suggest you use a tool specially designed for this purpose, such as Rufus (Windows), WoeUSB (Linux), or Boot Camp Assistant (macOS).",
    msg_image: "image",
    msg_drive: "drive",
    msg_large_drive_size: "This is a large drive! Make sure it doesn't contain files that you want to keep.",
    msg_system_drive: "Selecting your system drive is dangerous and will erase your drive!",
    msg_source_drive: "Contains the image you chose to flash",
    msg_no_space: "Not enough space on the drive. Please insert larger one and try again.",
    msg_validation: "The write has been completed successfully but Etcher detected potential corruption issues when reading the image back from the drive.\n\nPlease consider writing the image to a different drive.",
    msg_unplug: "Looks like Etcher lost access to the drive. Did it get unplugged accidentally?\n\nSometimes this error is caused by faulty readers that don't provide stable access to the drive.",
    msg_cannot_write: "Looks like Etcher is not able to write to this location of the drive. This error is usually caused by a faulty drive, reader, or port.\n\nPlease try again with another drive, reader, or port.",
    msg_child_writer_died: "The writer process ended unexpectedly. Please try again, and contact the Etcher team if the problem persists.",
    msg_bad_protocol: "Only http:// and https:// URLs are supported.",

    target_select_target: "Select target",
    target_plug_target: "Plug a target drive",
    target_targets: "Targets",
    target_change: "Change",

    source_use_source_url: "Use Image URL",
    source_auth: "Authentication",
    source_username: "Enter username",
    source_password: "Enter password",
    source_unsupported_protocol: "Unsupported protocol",
    source_windows_image: "Possible Windows image detected",
    source_partition_table: "Missing partition table",
    source_error_open: "Error opening source",
    source_from_file: "Flash from file",
    source_from_url: "Flash from URL",
    source_clone: "Clone drive",
    source_image: "Image",
    source_name: "Name: ",
    source_path: "Path: ",
    source_select_source: "Select source",
    source_plug_source: "Plug a source drive",
    source_os_images: "OS Images",
    source_all_files: "All",
    source_enter_valid_url: "Enter a valid URL",
    source_recent: "Recent",

    drives_name: "Name",
    drives_size: "Size",
    drives_location: "Location",
    drives_system_drive_danger: "Selecting your system drive is dangerous and will erase your drive!",
    drives_change_target: "Change target",
    drives_large_drive_warning: "You are about to erase an unusually large drive",
    drives_large_drive_warning_msg: "Are you sure the selected drive is not a storage drive?",
    drives_system_drive_warning: "You are about to erase your computer's drives",
    drives_system_drive_warning_msg: "Are you sure you want to flash your system drive?",
    drives_initializing: "Initializing device",

    flash_another: "Flash another",
    flash_target: "Target",
    flash_location: "Location",
    flash_error: "Error",
    flash_flash: "Flash",
    flash_flash_now: "Flash!",
    flash_skip: "Validation has been skipped",
    flash_more_info: "more info",
    flash_speed_tip: "The speed is calculated by dividing the image size by the flashing time.\nDisk images with ext partitions flash faster as we are able to skip unused parts.",
    flash_failed_target: "Failed targets",
    flash_failed_retry: "Retry failed targets",
    flash_failed: "Flash Failed.",
    flash_completed: "Flash Completed!",

    settings_error_reporting: "Anonymously report errors to balena.io",
    settings_auto_update: "Auto-updates enabled",
    settings_desktop_notifications: "Desktop notifications",
    settings_settings: "Settings",
    settings_system_information: "System Information",
    settings_trim_ext_partitions: "Trim unallocated space on raw images (in ext-type partitions)",
    settings_language: "Language",

    privacy_line1_prefix: "Etcher collects a limited amount of anonymous data to help us improve user experience. You can opt out in the ",
    privacy_settings_link: "settings",
    privacy_line2_prefix: "For more information about how we use this data, see our ",
    privacy_policy_link: "privacy policy",
    retry: "Retry",
};

const ZH_CN_MESSAGES: Messages = Messages {
    cont: "继续",
    ok: "好",
    cancel: "取消",
    skip: "跳过",
    sure: "我确定",
    warning: "请注意！",
    attention: "请注意",
    failed: "失败",
    completed: "完毕",
    yes_continue: "是的，继续",
    really_exit: "真的要现在退出 Etcher 吗？",
    yes_exit: "是的，可以退出",

    progress_starting: "正在启动……",
    progress_decompressing: "正在解压……",
    progress_flashing: "正在烧录……",
    progress_finishing: "正在结束……",
    progress_verifying: "正在验证……",
    progress_failing: "失败……",

    compat_size_not_recommended: "大小不推荐",
    compat_too_small: "空间太小",
    compat_locked: "被锁定",
    compat_system: "系统盘",
    compat_contains_image: "存放源镜像",
    compat_large_drive: "很大的磁盘",

    msg_exit_while_flashing: "您当前正在刷机。关闭 Etcher 可能会导致您的磁盘无法使用。",
    msg_looks_like_windows_image: "看起来您正在尝试刻录 Windows 镜像。\n\n与其他镜像不同，Windows 镜像需要特殊处理才能使其可启动。我们建议您使用专门为此目的设计的工具，例如 Rufus (Windows)、WoeUSB (Linux) 或 Boot Camp 助理 (macOS)。",
    msg_image: "镜像",
    msg_drive: "磁盘",
    msg_large_drive_size: "这是个很大的磁盘！请检查并确认它不包含对您很重要的信息",
    msg_system_drive: "选择系统盘很危险，因为这将会删除你的系统",
    msg_source_drive: "源镜像位于这个分区中",
    msg_no_space: "磁盘空间不足。请插入另一个较大的磁盘并重试。",
    msg_validation: "写入已成功完成，但 Etcher 在从磁盘读取镜像时检测到潜在的损坏问题。\n\n请考虑将镜像写入其他磁盘。",
    msg_unplug: "看起来 Etcher 失去了对磁盘的连接。它是不是被意外拔掉了？\n\n有时这个错误是因为读卡器出了故障。",
    msg_cannot_write: "看起来 Etcher 无法写入磁盘的这个位置。此错误通常是由故障的磁盘、读取器或端口引起的。\n\n请使用其他磁盘、读卡器或端口重试。",
    msg_child_writer_died: "写入进程意外崩溃。请再试一次，如果问题仍然存在，请联系 Etcher 团队。",
    msg_bad_protocol: "仅支持 http:// 和 https:// 开头的网址。",

    target_select_target: "选择目标磁盘",
    target_plug_target: "请插入目标磁盘",
    target_targets: "个目标",
    target_change: "更改",

    source_use_source_url: "使用镜像网络地址",
    source_auth: "验证",
    source_username: "输入用户名",
    source_password: "输入密码",
    source_unsupported_protocol: "不支持的协议",
    source_windows_image: "这可能是 Windows 系统镜像",
    source_partition_table: "找不到分区表",
    source_error_open: "打开源镜像时出错",
    source_from_file: "从文件烧录",
    source_from_url: "从在线地址烧录",
    source_clone: "克隆磁盘",
    source_image: "镜像信息",
    source_name: "名称：",
    source_path: "路径：",
    source_select_source: "选择源",
    source_plug_source: "请插入源磁盘",
    source_os_images: "系统镜像格式",
    source_all_files: "任何文件格式",
    source_enter_valid_url: "请输入一个正确的地址",
    source_recent: "最近使用",

    drives_name: "名称",
    drives_size: "大小",
    drives_location: "位置",
    drives_system_drive_danger: "选择系统盘很危险，因为这将会删除你的系统！",
    drives_change_target: "改变目标",
    drives_large_drive_warning: "您即将擦除一个非常大的磁盘",
    drives_large_drive_warning_msg: "您确定所选磁盘不是存储磁盘吗？",
    drives_system_drive_warning: "您将要擦除系统盘",
    drives_system_drive_warning_msg: "您确定要烧录到系统盘吗？",
    drives_initializing: "正在初始化设备",

    flash_another: "烧录另一目标",
    flash_target: "目标",
    flash_location: "位置",
    flash_error: "错误",
    flash_flash: "烧录",
    flash_flash_now: "现在烧录！",
    flash_skip: "跳过了验证",
    flash_more_info: "更多信息",
    flash_speed_tip: "通过将镜像大小除以烧录时间来计算速度。\n由于我们能够跳过未使用的部分，因此具有EXT分区的磁盘镜像烧录速度更快。",
    flash_failed_target: "失败的烧录目标",
    flash_failed_retry: "重试烧录失败目标",
    flash_failed: "烧录失败。",
    flash_completed: "烧录成功！",

    settings_error_reporting: "匿名地向 balena.io 报告运行错误和使用统计",
    settings_auto_update: "自动更新",
    settings_desktop_notifications: "桌面通知",
    settings_settings: "软件设置",
    settings_system_information: "系统信息",
    settings_trim_ext_partitions: "裁剪原始镜像上未分配的 ext 分区空间",
    settings_language: "语言",

    privacy_line1_prefix: "Etcher 会收集少量匿名数据以帮助改进用户体验。您可以在",
    privacy_settings_link: "设置",
    privacy_line2_prefix: "如需了解更多数据使用详情，请参阅我们的",
    privacy_policy_link: "隐私政策",
    retry: "重试",
};

const ZH_TW_MESSAGES: Messages = Messages {
    cont: "繼續",
    ok: "好",
    cancel: "取消",
    skip: "跳過",
    sure: "我確定",
    warning: "請注意！",
    attention: "請注意",
    failed: "失敗",
    completed: "完成",
    yes_continue: "是的，繼續",
    really_exit: "真的要現在結束 Etcher 嗎？",
    yes_exit: "是的，可以結束",

    progress_starting: "正在啟動……",
    progress_decompressing: "正在解壓縮……",
    progress_flashing: "正在燒錄……",
    progress_finishing: "正在結束……",
    progress_verifying: "正在驗證……",
    progress_failing: "失敗……",

    compat_size_not_recommended: "大小不建議",
    compat_too_small: "空間太小",
    compat_locked: "被鎖定",
    compat_system: "系統",
    compat_contains_image: "存放來源映像檔",
    compat_large_drive: "很大的磁碟",

    msg_exit_while_flashing: "您目前正在刷寫。關閉 Etcher 可能會導致您的磁碟無法使用。",
    msg_looks_like_windows_image: "看起來您正在嘗試燒錄 Windows 映像檔。\n\n與其他映像檔不同，Windows 映像檔需要特殊處理才能使其可啟動。我們建議您使用專門為此目的設計的工具，例如 Rufus (Windows)、WoeUSB (Linux) 或 Boot Camp 助理 (macOS)。",
    msg_image: "映像檔",
    msg_drive: "磁碟",
    msg_large_drive_size: "這是個很大容量的磁碟！請檢查並確認它不包含對您來說存放很重要的資料",
    msg_system_drive: "選擇系統分割區很危險，因為這將會刪除你的系統",
    msg_source_drive: "來源映像檔位於這個分割區中",
    msg_no_space: "磁碟空間不足。請插入另一個較大的磁碟並重試。",
    msg_validation: "寫入已成功完成，但 Etcher 在從磁碟讀取映像檔時檢測到潛在的損壞問題。\n\n請考慮將映像檔寫入其他磁碟。",
    msg_unplug: "看起來 Etcher 失去了對磁碟的連接。是不是被意外拔掉了？\n\n有時這個錯誤是因為讀卡器出了故障。",
    msg_cannot_write: "看起來 Etcher 無法寫入磁碟的這個位置。此錯誤通常是由故障的磁碟、讀取器或連接埠引起的。\n\n請使用其他磁碟、讀卡器或連接埠重試。",
    msg_child_writer_died: "寫入處理程序意外崩潰。請再試一次，如果問題仍然存在，請聯絡 Etcher 團隊。",
    msg_bad_protocol: "僅支援 http:// 和 https:// 開頭的網址。",

    target_select_target: "選擇目標磁碟",
    target_plug_target: "請插入目標磁碟",
    target_targets: "個目標",
    target_change: "更改",

    source_use_source_url: "使用映像檔網址",
    source_auth: "驗證",
    source_username: "輸入使用者名稱",
    source_password: "輸入密碼",
    source_unsupported_protocol: "不支持的通訊協定",
    source_windows_image: "這可能是 Windows 系統映像檔",
    source_partition_table: "找不到分割表",
    source_error_open: "打開來源映像檔時出錯",
    source_from_file: "從檔案燒錄",
    source_from_url: "從網址燒錄",
    source_clone: "再製磁碟",
    source_image: "映像檔訊息",
    source_name: "名稱：",
    source_path: "路徑：",
    source_select_source: "選擇來源",
    source_plug_source: "請插入來源磁碟",
    source_os_images: "系統映像檔格式",
    source_all_files: "任何檔案格式",
    source_enter_valid_url: "請輸入正確的網址",
    source_recent: "最近使用",

    drives_name: "名稱",
    drives_size: "大小",
    drives_location: "位置",
    drives_system_drive_danger: "選擇系統分割區很危險，因為這將會刪除你的系統！",
    drives_change_target: "更改目標",
    drives_large_drive_warning: "您即將格式化一個非常大的磁碟",
    drives_large_drive_warning_msg: "您確定所選磁碟不是儲存資料的磁碟嗎？",
    drives_system_drive_warning: "您將要格式化系統分割區",
    drives_system_drive_warning_msg: "您確定要燒錄到系統分割區嗎？",
    drives_initializing: "正在初始化裝置",

    flash_another: "燒錄另一目標",
    flash_target: "目標",
    flash_location: "位置",
    flash_error: "錯誤",
    flash_flash: "燒錄",
    flash_flash_now: "現在燒錄！",
    flash_skip: "跳過了驗證",
    flash_more_info: "更多資訊",
    flash_speed_tip: "透過將映像檔大小除以燒錄時間來計算速度。\n由於我們能夠跳過未使用的部分，因此具有 ext 分割區的磁碟映像檔燒錄速度更快。",
    flash_failed_target: "目標燒錄失敗",
    flash_failed_retry: "重試燒錄失敗的目標",
    flash_failed: "燒錄失敗。",
    flash_completed: "燒錄成功！",

    settings_error_reporting: "匿名向 balena.io 回報程式錯誤和使用統計資料",
    settings_auto_update: "自動更新",
    settings_desktop_notifications: "桌面通知",
    settings_settings: "軟體設定",
    settings_system_information: "系統資訊",
    settings_trim_ext_partitions: "修改原始映像檔上未分配的空間（在 ext 類型分割區中）",
    settings_language: "語言",

    privacy_line1_prefix: "Etcher 會收集少量匿名資料以幫助改進使用者體驗。您可以在",
    privacy_settings_link: "設定",
    privacy_line2_prefix: "欲了解更多資料使用方式，請參閱我們的",
    privacy_policy_link: "隱私權政策",
    retry: "重試",
};

pub fn messages(locale: Locale) -> &'static Messages {
    match locale {
        Locale::En => &EN_MESSAGES,
        Locale::ZhCn => &ZH_CN_MESSAGES,
        Locale::ZhTw => &ZH_TW_MESSAGES,
    }
}

pub fn progress_successful(locale: Locale, count: usize) -> String {
    match locale {
        Locale::En => {
            if count == 1 {
                "Successful target".to_string()
            } else {
                "Successful targets".to_string()
            }
        }
        Locale::ZhCn => "烧录成功".to_string(),
        Locale::ZhTw => "燒錄成功".to_string(),
    }
}

pub fn progress_failed(locale: Locale, count: usize) -> String {
    match locale {
        Locale::En => {
            if count == 1 {
                "Failed target".to_string()
            } else {
                "Failed targets".to_string()
            }
        }
        Locale::ZhCn => "烧录失败".to_string(),
        Locale::ZhTw => "燒錄失敗".to_string(),
    }
}

pub fn drives_found(locale: Locale, length: usize) -> String {
    match locale {
        Locale::En => format!("{length} found"),
        Locale::ZhCn => format!("找到 {length} 个"),
        Locale::ZhTw => format!("找到 {length} 個"),
    }
}

pub fn drives_select(locale: Locale, select: usize) -> String {
    match locale {
        Locale::En => format!("Select ({select})"),
        Locale::ZhCn => format!("选定 ({select})"),
        Locale::ZhTw => format!("選取 ({select})"),
    }
}

pub fn drives_show_hidden(locale: Locale, num: usize) -> String {
    match locale {
        Locale::En => format!("Show {num} hidden"),
        Locale::ZhCn => format!("显示 {num} 个隐藏的磁盘"),
        Locale::ZhTw => format!("顯示 {num} 個隱藏的磁碟"),
    }
}

pub fn flash_speed(locale: Locale, speed_mb: f64) -> String {
    match locale {
        Locale::En => format!("Effective speed: {speed_mb:.1} MB/s"),
        Locale::ZhCn => format!("速度：{speed_mb:.1} MB/秒"),
        Locale::ZhTw => format!("速度：{speed_mb:.1} MB/秒"),
    }
}

pub fn flash_speed_short(locale: Locale, speed_mb: f64) -> String {
    match locale {
        Locale::En => format!("{speed_mb:.2} MB/s"),
        Locale::ZhCn => format!("{speed_mb:.2} MB/秒"),
        Locale::ZhTw => format!("{speed_mb:.2} MB/秒"),
    }
}

pub fn flash_eta(locale: Locale, eta: &str) -> String {
    match locale {
        Locale::En => format!("ETA: {eta}"),
        Locale::ZhCn => format!("预计还需要：{eta}"),
        Locale::ZhTw => format!("預計還需要：{eta}"),
    }
}

pub fn msg_too_small(locale: Locale, source_size: u64, target_size: u64) -> String {
    let diff = pretty_bytes(source_size.saturating_sub(target_size));
    match locale {
        Locale::En => format!("The selected source is {diff} larger than this drive."),
        Locale::ZhCn => format!("所选的镜像比目标盘大了 {diff}。"),
        Locale::ZhTw => format!("所選的映像檔比目標磁碟大了 {diff}。"),
    }
}

pub fn msg_missing_partition_table(locale: Locale, is_drive: bool) -> String {
    let m = messages(locale);
    let typ = if is_drive { m.msg_drive } else { m.msg_image };
    match locale {
        Locale::En => format!(
            "It looks like this is not a bootable {typ}.\n\nThe {typ} does not appear to contain a partition table, and might not be recognized or bootable by your device."
        ),
        Locale::ZhCn => format!(
            "看起来这不是一个可启动的{typ}。\n\n这个{typ}似乎不包含分区表，因此您的设备可能无法识别或无法正确启动。"
        ),
        Locale::ZhTw => format!(
            "看起來這不是一個可啟動的{typ}。\n\n這個{typ}似乎不包含分割表，因此您的設備可能無法識別或無法正確啟動。"
        ),
    }
}

pub fn msg_open_source(locale: Locale, source_name: &str, error_message: &str) -> String {
    match locale {
        Locale::En => {
            format!("Something went wrong while opening {source_name}.\n\nError: {error_message}")
        }
        Locale::ZhCn => format!("打开 {source_name} 时出错。\n\n错误信息： {error_message}"),
        Locale::ZhTw => format!("打開 {source_name} 時發生錯誤。\n\n錯誤訊息： {error_message}"),
    }
}

pub fn msg_generic_flash_error(locale: Locale, error_message: &str) -> String {
    match locale {
        Locale::En => format!(
            "Something went wrong. If it is a compressed image, please check that the archive is not corrupted.\n{error_message}"
        ),
        Locale::ZhCn => {
            format!("出了点问题。如果源镜像曾被压缩过，请检查它是否已损坏。\n{error_message}")
        }
        Locale::ZhTw => {
            format!("出了點問題。如果來源映像檔曾被壓縮過，請檢查它是否已損壞。\n{error_message}")
        }
    }
}

pub fn msg_flash_complete(
    locale: Locale,
    image_basename: &str,
    drive_desc_and_name: Option<(&str, &str)>,
    successful: usize,
    failed: usize,
) -> String {
    let mut parts = Vec::new();
    if failed + successful == 1 {
        if let Some((description, name)) = drive_desc_and_name {
            parts.push(match locale {
                Locale::En => format!("to {description} ({name})"),
                Locale::ZhCn | Locale::ZhTw => format!("到 {description} ({name})"),
            });
        }
    } else {
        if successful > 0 {
            parts.push(match locale {
                Locale::En => {
                    if successful == 1 {
                        format!("to {successful} target")
                    } else {
                        format!("to {successful} targets")
                    }
                }
                Locale::ZhCn => format!("到 {successful} 个目标"),
                Locale::ZhTw => format!("到 {successful} 個目標"),
            });
        }
        if failed > 0 {
            parts.push(match locale {
                Locale::En => {
                    if failed == 1 {
                        format!("and failed to be flashed to {failed} target")
                    } else {
                        format!("and failed to be flashed to {failed} targets")
                    }
                }
                Locale::ZhCn => format!("并烧录失败了 {failed} 个目标"),
                Locale::ZhTw => format!("並燒錄失敗了 {failed} 個目標"),
            });
        }
    }
    let targets = parts.join(" ");
    match locale {
        Locale::En => format!("{image_basename} was successfully flashed {targets}"),
        Locale::ZhCn => format!("{image_basename} 被成功烧录 {targets}"),
        Locale::ZhTw => format!("{image_basename} 被成功燒錄 {targets}"),
    }
}

pub fn msg_flash_failure(
    locale: Locale,
    image_basename: &str,
    drives: &[(&str, &str)],
) -> String {
    let target = if drives.len() == 1 {
        let (description, name) = drives[0];
        match locale {
            Locale::En => format!("to {description} ({name})"),
            Locale::ZhCn | Locale::ZhTw => format!("到 {description} ({name})"),
        }
    } else {
        let count = drives.len();
        match locale {
            Locale::En => {
                if count == 1 {
                    format!("to {count} target")
                } else {
                    format!("to {count} targets")
                }
            }
            Locale::ZhCn => format!("到 {count} 个目标"),
            Locale::ZhTw => format!("到 {count} 個目標"),
        }
    };
    match locale {
        Locale::En => format!("Something went wrong while writing {image_basename} {target}."),
        Locale::ZhCn => format!("烧录 {image_basename} {target} 失败。"),
        Locale::ZhTw => format!("燒錄 {image_basename} {target} 失敗。"),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgressDisplay {
    pub status: String,
    pub position: Option<String>,
}

pub fn progress_from_state(
    locale: Locale,
    phase: FlashPhase,
    percentage: Option<u8>,
    position: u64,
) -> ProgressDisplay {
    let m = messages(locale);
    match phase {
        FlashPhase::Starting => ProgressDisplay {
            status: m.progress_starting.to_string(),
            position: None,
        },
        FlashPhase::Decompressing => ProgressDisplay {
            status: m.progress_decompressing.to_string(),
            position: percentage.map(|p| format!("{p}%")),
        },
        FlashPhase::Flashing => match percentage {
            Some(p) if p < 100 => ProgressDisplay {
                status: m.progress_flashing.to_string(),
                position: Some(format!("{p}%")),
            },
            Some(_) => ProgressDisplay {
                status: m.progress_finishing.to_string(),
                position: None,
            },
            None => ProgressDisplay {
                status: m.progress_flashing.to_string(),
                position: Some(if position > 0 {
                    pretty_bytes(position)
                } else {
                    String::new()
                }),
            },
        },
        FlashPhase::Verifying => match percentage {
            Some(p) if p < 100 => ProgressDisplay {
                status: m.progress_verifying.to_string(),
                position: Some(format!("{p}%")),
            },
            Some(_) => ProgressDisplay {
                status: m.progress_finishing.to_string(),
                position: None,
            },
            None => ProgressDisplay {
                status: m.progress_verifying.to_string(),
                position: None,
            },
        },
        FlashPhase::Finishing => ProgressDisplay {
            status: m.progress_finishing.to_string(),
            position: None,
        },
    }
}

pub fn title_from_flash_state(locale: Locale, state: &ProgressState) -> String {
    let disp = progress_from_state(locale, state.phase, state.percentage, state.position);
    match disp.position {
        Some(pos) if !pos.is_empty() => format!("{pos} {}", disp.status),
        _ => disp.status,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_title_from_flash_state_parity() {
        let mut state = ProgressState {
            phase: FlashPhase::Flashing,
            percentage: Some(0),
            speed: 100.0,
            active: 1,
            ..Default::default()
        };
        assert_eq!(title_from_flash_state(Locale::En, &state), "0% Flashing...");

        state.phase = FlashPhase::Verifying;
        assert_eq!(
            title_from_flash_state(Locale::En, &state),
            "0% Validating..."
        );

        state.percentage = Some(50);
        state.phase = FlashPhase::Flashing;
        assert_eq!(title_from_flash_state(Locale::En, &state), "50% Flashing...");

        state.phase = FlashPhase::Verifying;
        assert_eq!(
            title_from_flash_state(Locale::En, &state),
            "50% Validating..."
        );

        state.percentage = Some(100);
        state.phase = FlashPhase::Flashing;
        assert_eq!(title_from_flash_state(Locale::En, &state), "Finishing...");

        state.phase = FlashPhase::Verifying;
        assert_eq!(title_from_flash_state(Locale::En, &state), "Finishing...");
    }

    #[test]
    fn test_all_locales_have_strings() {
        for locale in [Locale::En, Locale::ZhCn, Locale::ZhTw] {
            let m = messages(locale);
            assert!(!m.flash_flash_now.is_empty());
            assert!(!m.settings_settings.is_empty());
            assert!(!progress_successful(locale, 1).is_empty());
            assert!(!progress_failed(locale, 2).is_empty());
        }
    }
}
