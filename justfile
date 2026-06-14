
# Regenerate the README demo GIFs. Needs ffmpeg + a monospace font with
# Braille/block coverage (Adwaita Mono; override with RATATUI_SCIFI_FONT).
# Pass an example name to render just one: `just screenshots dashboard`.
screenshots example='':
    cargo run -p ratatui-sci-fi --example capture_screenshots -- {{example}}

release-patch:
    cargo release patch --no-publish --execute

release-minor:
    cargo release minor --no-publish --execute

release-major:
    cargo release major --no-publish --execute

upgrade:
    cargo +nightly update --breaking -Z unstable-options

publish:
    cargo publish --registry crates-io