use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Config {
    pub(crate) executable: String,
    pub(crate) settings: BTreeMap<String, String>,
    pub(crate) extra_args: String,
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
            ("proxy", ""),
            ("cookies", ""),
            ("sub_langs", "en.*"),
            ("date_after", ""),
            ("date_before", ""),
            ("playlist_items", ""),
            ("sponsorblock", ""),
            ("sponsorblock_remove", ""),
            ("ffmpeg_location", ""),
            ("write_subs", "false"),
            ("write_auto_subs", "false"),
            ("embed_subs", "false"),
            ("extract_audio", "false"),
            ("embed_metadata", "true"),
            ("embed_thumbnail", "false"),
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
            "proxy" => OptionCategory::Network,
            "date_after"
            | "date_before"
            | "playlist_items"
            | "download_archive"
            | "playlist"
            | "sponsorblock"
            | "sponsorblock_remove"
            | "format"
            | "remux_video" => OptionCategory::VideoSelection,
            "output" | "paths" | "cookies" | "continue" | "no_part" | "windows_filenames" => {
                OptionCategory::Filesystem
            }
            "embed_thumbnail" => OptionCategory::Thumbnail,
            "no_warnings" => OptionCategory::Verbosity,
            "audio_format" | "audio_quality" | "rate_limit" | "retries" | "fragment_retries"
            | "sub_langs" | "write_subs" | "write_auto_subs" | "embed_subs" | "embed_chapters"
            | "extract_audio" | "embed_metadata" | "ffmpeg_location" => OptionCategory::Download,
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
        key: "proxy",
        label: "Proxy URL",
        flag: "--proxy",
        kind: Kind::Text,
        help: "HTTP, HTTPS, SOCKS or empty for direct",
    },
    Spec {
        key: "cookies",
        label: "Cookies file",
        flag: "--cookies",
        kind: Kind::Path,
        help: "Netscape-format cookies file",
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
