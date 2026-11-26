use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Config {
    pub(crate) executable: String,
    pub(crate) settings: BTreeMap<String, String>,
    pub(crate) extra_args: String,
    pub(crate) notifications_enabled: bool,
    pub(crate) notification_display_seconds: u32,
    pub(crate) notification_color_scheme: String,
}

impl Default for Config {
    fn default() -> Self {
        let mut settings = BTreeMap::new();
        for (key, value) in [
            ("format", "bestvideo+bestaudio/best"),
            ("remux_video", ""),
            ("output", "%(title)s.%(ext)s"),
            ("download_archive", ""),
            ("paths", ""),
            ("audio_format", "mp3"),
            ("audio_quality", "0"),
            ("rate_limit", ""),
            ("retries", "10"),
            ("fragment_retries", "10"),
            ("concurrent_fragments", "1"),
            ("retry_sleep", ""),
            ("proxy", ""),
            ("socket_timeout", ""),
            ("cookies", ""),
            ("cookies_from_browser", ""),
            ("sub_langs", "en.*"),
            ("date_after", ""),
            ("date_before", ""),
            ("playlist_items", ""),
            ("download_sections", ""),
            ("match_filters", ""),
            ("min_filesize", ""),
            ("max_filesize", ""),
            ("sponsorblock", ""),
            ("sponsorblock_remove", ""),
            ("ffmpeg_location", ""),
            ("write_subs", "false"),
            ("write_auto_subs", "false"),
            ("embed_subs", "false"),
            ("extract_audio", "false"),
            ("embed_metadata", "true"),
            ("embed_thumbnail", "false"),
            ("write_thumbnail", "false"),
            ("write_info_json", "false"),
            ("write_description", "false"),
            ("no_warnings", "false"),
            ("ignore_errors", "false"),
            ("continue", "true"),
            ("playlist", "true"),
            ("windows_filenames", "false"),
            ("no_part", "false"),
            ("embed_chapters", "false"),
        ] {
            settings.insert(key.into(), value.into());
        }
        Self {
            executable: "yt-dlp".into(),
            settings,
            extra_args: String::new(),
            notifications_enabled: true,
            notification_display_seconds: 6,
            notification_color_scheme: "Blue".into(),
        }
    }
}

#[derive(Clone, PartialEq)]
pub(crate) enum Kind {
    Text,
    Number,
    Bool,
    Choice(&'static [&'static str]),
    Path,
}

pub(crate) struct Spec {
    pub(crate) key: &'static str,
    pub(crate) label: &'static str,
    pub(crate) flag: &'static str,
    pub(crate) kind: Kind,
    pub(crate) help: &'static str,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum OptionCategory {
    General,
    Network,
    GeoRestriction,
    VideoSelection,
    Download,
    Filesystem,
    Thumbnail,
    InternetShortcut,
    Verbosity,
}

impl OptionCategory {
    pub(crate) const HELP_ORDER: [Self; 9] = [
        Self::General,
        Self::Network,
        Self::GeoRestriction,
        Self::VideoSelection,
        Self::Download,
        Self::Filesystem,
        Self::Thumbnail,
        Self::InternetShortcut,
        Self::Verbosity,
    ];

    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::General => "General Options",
            Self::Network => "Network Options",
            Self::GeoRestriction => "Geo-restriction",
            Self::VideoSelection => "Video Selection",
            Self::Download => "Download Options",
            Self::Filesystem => "Filesystem Options",
            Self::Thumbnail => "Thumbnail Options",
            Self::InternetShortcut => "Internet Shortcut Options",
            Self::Verbosity => "Verbosity and Simulation Options",
        }
    }

    pub(crate) fn id(self) -> &'static str {
        match self {
            Self::General => "general",
            Self::Network => "network",
            Self::GeoRestriction => "geo-restriction",
            Self::VideoSelection => "video-selection",
            Self::Download => "download",
            Self::Filesystem => "filesystem",
            Self::Thumbnail => "thumbnail",
            Self::InternetShortcut => "internet-shortcut",
            Self::Verbosity => "verbosity",
        }
    }
}

impl Spec {
    pub(crate) fn category(&self) -> OptionCategory {
        match self.key {
            "ignore_errors" => OptionCategory::General,
            "proxy" | "socket_timeout" => OptionCategory::Network,
            "date_after"
            | "date_before"
            | "playlist_items"
            | "download_archive"
            | "playlist"
            | "sponsorblock"
            | "sponsorblock_remove"
            | "format"
            | "remux_video"
            | "download_sections"
            | "match_filters"
            | "min_filesize"
            | "max_filesize" => OptionCategory::VideoSelection,
            "output"
            | "paths"
            | "cookies"
            | "cookies_from_browser"
            | "write_info_json"
            | "write_description"
            | "continue"
            | "no_part"
            | "windows_filenames" => OptionCategory::Filesystem,
            "embed_thumbnail" | "write_thumbnail" => OptionCategory::Thumbnail,
            "no_warnings" => OptionCategory::Verbosity,
            "audio_format"
            | "audio_quality"
            | "rate_limit"
            | "retries"
            | "fragment_retries"
            | "concurrent_fragments"
            | "retry_sleep"
            | "sub_langs"
            | "write_subs"
            | "write_auto_subs"
            | "embed_subs"
            | "embed_chapters"
            | "extract_audio"
            | "embed_metadata"
            | "ffmpeg_location" => OptionCategory::Download,
            _ => OptionCategory::Download,
        }
    }
}

pub(crate) const SPECS: &[Spec] = &[
    Spec {
        key: "format",
        label: "Format selector",
        flag: "-f",
        kind: Kind::Text,
        help: "yt-dlp format expression",
    },
    Spec {
        key: "remux_video",
        label: "Output container",
        flag: "--remux-video",
        kind: Kind::Choice(&["", "mp4", "mkv", "webm", "mov", "avi"]),
        help: "Remux without re-encoding when supported; requires FFmpeg",
    },
    Spec {
        key: "output",
        label: "Output filename template",
        flag: "-o",
        kind: Kind::Text,
        help: "%(title)s.%(ext)s",
    },
    Spec {
        key: "paths",
        label: "Download directory",
        flag: "-P",
        kind: Kind::Path,
        help: "Directory for downloaded files",
    },
    Spec {
        key: "download_archive",
        label: "Download archive file",
        flag: "--download-archive",
        kind: Kind::Path,
        help: "Skip items already recorded in this file",
    },
    Spec {
        key: "audio_format",
        label: "Audio format",
        flag: "--audio-format",
        kind: Kind::Choice(&[
            "mp3", "m4a", "opus", "wav", "flac", "vorbis", "aac", "alac", "best",
        ]),
        help: "Used when audio extraction is enabled",
    },
    Spec {
        key: "audio_quality",
        label: "Audio quality",
        flag: "--audio-quality",
        kind: Kind::Choice(&["0", "2", "5", "7", "9"]),
        help: "0 is best quality; 9 is smallest",
    },
    Spec {
        key: "rate_limit",
        label: "Rate limit",
        flag: "-r",
        kind: Kind::Text,
        help: "For example 2M or 500K",
    },
    Spec {
        key: "retries",
        label: "Retries",
        flag: "-R",
        kind: Kind::Number,
        help: "Retry count for downloads",
    },
    Spec {
        key: "fragment_retries",
        label: "Fragment retries",
        flag: "--fragment-retries",
        kind: Kind::Number,
        help: "Retry count for media fragments",
    },
    Spec {
        key: "concurrent_fragments",
        label: "Concurrent fragments",
        flag: "--concurrent-fragments",
        kind: Kind::Number,
        help: "Parallel downloads for DASH/HLS fragments; increase to improve speed",
    },
    Spec {
        key: "retry_sleep",
        label: "Retry delay",
        flag: "--retry-sleep",
        kind: Kind::Text,
        help: "Delay between retries, e.g. 5 or linear=1::2",
    },
    Spec {
        key: "proxy",
        label: "Proxy URL",
        flag: "--proxy",
        kind: Kind::Text,
        help: "HTTP, HTTPS, SOCKS or empty for direct",
    },
    Spec {
        key: "socket_timeout",
        label: "Network timeout (seconds)",
        flag: "--socket-timeout",
        kind: Kind::Number,
        help: "Time to wait for a network response before giving up",
    },
    Spec {
        key: "cookies",
        label: "Cookies file",
        flag: "--cookies",
        kind: Kind::Path,
        help: "Netscape-format cookies file",
    },
    Spec {
        key: "cookies_from_browser",
        label: "Cookies from browser",
        flag: "--cookies-from-browser",
        kind: Kind::Text,
        help: "Browser and optional profile, e.g. firefox or chrome:Profile 1",
    },
    Spec {
        key: "sub_langs",
        label: "Subtitle languages",
        flag: "--sub-langs",
        kind: Kind::Text,
        help: "Comma separated language patterns",
    },
    Spec {
        key: "date_after",
        label: "Date after",
        flag: "--dateafter",
        kind: Kind::Text,
        help: "YYYYMMDD",
    },
    Spec {
        key: "date_before",
        label: "Date before",
        flag: "--datebefore",
        kind: Kind::Text,
        help: "YYYYMMDD",
    },
    Spec {
        key: "playlist_items",
        label: "Playlist items",
        flag: "--playlist-items",
        kind: Kind::Text,
        help: "For example 1,2,5-8",
    },
    Spec {
        key: "download_sections",
        label: "Download sections",
        flag: "--download-sections",
        kind: Kind::Text,
        help: "Chapter name or time range, e.g. *10:15-12:30 or intro",
    },
    Spec {
        key: "match_filters",
        label: "Video match filter",
        flag: "--match-filters",
        kind: Kind::Text,
        help: "Metadata expression, e.g. duration<3600 or !is_live",
    },
    Spec {
        key: "min_filesize",
        label: "Minimum file size",
        flag: "--min-filesize",
        kind: Kind::Text,
        help: "Skip smaller files, e.g. 50k or 1M",
    },
    Spec {
        key: "max_filesize",
        label: "Maximum file size",
        flag: "--max-filesize",
        kind: Kind::Text,
        help: "Skip larger files, e.g. 500M or 2G",
    },
    Spec {
        key: "sponsorblock",
        label: "SponsorBlock categories",
        flag: "--sponsorblock-mark",
        kind: Kind::Text,
        help: "For example all or sponsor,intro",
    },
    Spec {
        key: "sponsorblock_remove",
        label: "Remove SponsorBlock categories",
        flag: "--sponsorblock-remove",
        kind: Kind::Text,
        help: "For example sponsor,selfpromo,interaction",
    },
    Spec {
        key: "ffmpeg_location",
        label: "FFmpeg location",
        flag: "--ffmpeg-location",
        kind: Kind::Path,
        help: "FFmpeg executable or its containing directory",
    },
    Spec {
        key: "write_subs",
        label: "Write subtitles",
        flag: "--write-subs",
        kind: Kind::Bool,
        help: "Download manual subtitles",
    },
    Spec {
        key: "write_auto_subs",
        label: "Write automatic subtitles",
        flag: "--write-auto-subs",
        kind: Kind::Bool,
        help: "Download auto-generated subtitles",
    },
    Spec {
        key: "embed_subs",
        label: "Embed subtitles",
        flag: "--embed-subs",
        kind: Kind::Bool,
        help: "Embed downloaded subtitles",
    },
    Spec {
        key: "embed_chapters",
        label: "Embed chapters",
        flag: "--embed-chapters",
        kind: Kind::Bool,
        help: "Write video chapters as embedded chapters",
    },
    Spec {
        key: "extract_audio",
        label: "Extract audio",
        flag: "-x",
        kind: Kind::Bool,
        help: "Convert video to audio",
    },
    Spec {
        key: "embed_metadata",
        label: "Embed metadata",
        flag: "--embed-metadata",
        kind: Kind::Bool,
        help: "Add metadata to media files",
    },
    Spec {
        key: "embed_thumbnail",
        label: "Embed thumbnail",
        flag: "--embed-thumbnail",
        kind: Kind::Bool,
        help: "Add thumbnail to media files",
    },
    Spec {
        key: "write_thumbnail",
        label: "Save thumbnail",
        flag: "--write-thumbnail",
        kind: Kind::Bool,
        help: "Save the thumbnail image beside the downloaded media",
    },
    Spec {
        key: "write_info_json",
        label: "Save info JSON",
        flag: "--write-info-json",
        kind: Kind::Bool,
        help: "Save extracted video metadata in a .info.json file",
    },
    Spec {
        key: "write_description",
        label: "Save description",
        flag: "--write-description",
        kind: Kind::Bool,
        help: "Save the video description in a .description file",
    },
    Spec {
        key: "no_warnings",
        label: "Suppress warnings",
        flag: "--no-warnings",
        kind: Kind::Bool,
        help: "Hide yt-dlp warning messages",
    },
    Spec {
        key: "ignore_errors",
        label: "Ignore playlist errors",
        flag: "-i",
        kind: Kind::Bool,
        help: "Continue a playlist after an item fails",
    },
    Spec {
        key: "continue",
        label: "Continue partial downloads",
        flag: "-c",
        kind: Kind::Bool,
        help: "Resume partially downloaded files",
    },
    Spec {
        key: "playlist",
        label: "Download playlists",
        flag: "--yes-playlist",
        kind: Kind::Bool,
        help: "Allow playlist URLs to download the playlist",
    },
    Spec {
        key: "windows_filenames",
        label: "Windows-safe filenames",
        flag: "--windows-filenames",
        kind: Kind::Bool,
        help: "Replace characters that are invalid in Windows filenames",
    },
    Spec {
        key: "no_part",
        label: "Disable .part files",
        flag: "--no-part",
        kind: Kind::Bool,
        help: "Write downloads directly to the destination filename",
    },
];

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct Entry {
    pub(crate) id: u64,
    #[serde(default)]
    pub(crate) title: String,
    #[serde(default)]
    pub(crate) already_in_history: bool,
    #[serde(default)]
    pub(crate) completed_at: i64,
    pub(crate) url: String,
    pub(crate) status: String,
    pub(crate) progress: String,
    pub(crate) error: String,
    pub(crate) overrides: BTreeMap<String, String>,
    pub(crate) extra_args: String,
}

#[derive(Default, Serialize, Deserialize)]
pub(crate) struct HistoryFile {
    pub(crate) entries: Vec<Entry>,
}

#[derive(Clone)]
pub(crate) enum Event {
    Line(u64, String),
    Done(u64, Result<(), String>),
    Formats(u64, Result<Vec<VideoFormat>, String>),
    Title(u64, Result<String, String>, bool),
}

#[derive(Clone)]
pub(crate) struct VideoFormat {
    pub(crate) id: String,
    pub(crate) label: String,
}

pub(crate) enum FormatState {
    Loading,
    Loaded(Vec<VideoFormat>),
    Failed(String),
}

#[derive(Clone, PartialEq)]
pub(crate) enum Page {
    Staging,
    Downloads,
    History,
    Options,
}
