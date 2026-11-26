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
- Confirm a staged item with **Download**. Active downloads show yt-dlp output and progress lines. Successful downloads move to History; failed downloads return to staging with their error available there.
- The command line field imports recognized yt-dlp options into typed settings, stages YouTube URLs, and keeps unrecognized flags in Additional arguments. Export builds a reusable command line from the baseline.

The UI exposes common format, output, path, audio, subtitle, retry, proxy, cookie, playlist, date, and SponsorBlock options. Additional yt-dlp flags remain available through the Additional arguments field.

The window opens at 1280 × 900 and can be resized. Page content scrolls in both directions when it does not fit the available space. The application icon is available as SVG, PNG, and multi-resolution ICO under `resources/`; Windows builds embed the ICO in the executable.

## Project guide

Open [the HTML documentation](docs/index.html) for an overview, module map, download workflow, configuration reference, and Rust feature guide.

## Source layout

- `src/app.rs` owns app state, clipboard polling, command execution, and download transitions.
- `src/model.rs` defines configuration, typed option metadata, and download state.
- `src/helpers.rs` contains shared input widgets and URL/progress helpers.
- `src/pages/` contains the staging, downloads, history, and options screens.
- `docs/` contains the static HTML project guide.
