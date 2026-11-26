# yt-dlp UI

A small Rust desktop front end for an existing `yt-dlp` executable.

![yt-dlp UI application icon](resources/yt-dlp-ui.png)

## Run

Install Rust, then run:

```sh
cargo run
```

On the **Options** page, set the path to `yt-dlp` or `yt-dlp.exe` if it is not on `PATH`. Set the baseline download options and use **Save settings** to write `yt-dlp-ui.toml` in the current working directory. The TOML configuration can also be imported or exported from that page.

## Workflow

- The app polls the clipboard for YouTube video, Shorts, live stream, and playlist URLs. You can also paste a URL into the staging page.
- Duplicate URLs are rejected while they are staged, downloading, or present in this session's history.
- Staged items can override individual typed options or add extra arguments. Overrides inherit the current baseline until enabled.
- Confirm a staged item with **Download** or use **Download all** to queue the staging list in order. Only one yt-dlp process runs at a time. The Downloads view shows the current item and the remaining queue; successful items move to History and failures return to staging.
- Cancel the current download from the Downloads view. The app stops yt-dlp, removes its partial media files when possible, and returns the item to staging marked failed. On Windows, yt-dlp runs without opening a separate console window; its output remains in the app.
- The command line field imports recognized yt-dlp options into typed settings, stages YouTube URLs, and keeps unrecognized flags in Additional arguments. Export builds a reusable command line from the baseline.

The UI exposes common format, output, path, audio, subtitle, retry, proxy, cookie, playlist, date, and SponsorBlock options. Additional yt-dlp flags remain available through the Additional arguments field.

The baseline format selector offers common yt-dlp expressions and accepts custom expressions. In staging, opening a download's override panel queries the video's available formats with yt-dlp; choose a format ID from that item's override dropdown. The Output container setting can remux to MP4, MKV, WebM, MOV, or AVI when FFmpeg can place the selected codecs in that container.

The window opens at 1280 × 900 and can be resized. Page content scrolls in both directions when it does not fit the available space. The application icon is available as SVG, PNG, and multi-resolution ICO under `resources/`; Windows builds embed the ICO in the executable.

Completed downloads are saved in `yt-dlp-ui-history.toml` in the current working directory and restored on startup. When a `--download-archive` file is configured, existing YouTube IDs in that archive are added to the UI history on startup, so downloads from before this persistence feature can be recovered as URLs. Archive-recovered entries have no original title metadata.

## Source layout

- `src/app.rs` owns app state, clipboard polling, command execution, and download transitions.
- `src/model.rs` defines configuration, typed option metadata, and download state.
- `src/helpers.rs` contains shared input widgets and URL/progress helpers.
- `src/pages/` contains the staging, downloads, history, and options screens.
