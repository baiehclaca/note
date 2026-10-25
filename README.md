<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/hero-dark.svg">
    <source media="(prefers-color-scheme: light)" srcset="assets/hero-light.svg">
    <img alt="note: leave a note on any folder. Spotlight remembers it." src="assets/hero-light.svg" width="100%">
  </picture>
</p>

<p align="center">
  <a href="assets/demo.mp4"><img alt="note demo: tag a folder in the terminal, then find it with Spotlight" src="assets/demo.gif" width="100%"></a>
  <br>
  <a href="assets/demo.mp4"><b>Watch the demo with sound →</b></a>
</p>

Your folders have names, but not memories. Tell a folder what's inside once, in your own words, and Spotlight hands it back the day you forget where it went.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/steps-dark.svg">
  <source media="(prefers-color-scheme: light)" srcset="assets/steps-light.svg">
  <img alt="1 Note it, 2 Search it, 3 Found" src="assets/steps-light.svg" width="100%">
</picture>

```sh
cd ~/Projects/client-work
note prague pitch, march, the blue deck   # leave the note
note find prague                          # weeks later, from any folder
```

<p align="center">
  <img alt="Setting a note in the terminal" src="assets/still-terminal.png" width="49%">
  <img alt="Spotlight finds the folder by its note" src="assets/still-spotlight.png" width="49%">
</p>

## Install

```sh
git clone https://github.com/baiehclaca/note && cd note
cargo build --release && cp target/release/note ~/bin/note
```

macOS only for now. Windows and Linux: coming.

**How it works:** your words become the folder's Finder comment, Spotlight indexes it, and nothing is added inside the folder.

<sub><code>note</code> shows the note, <code>note clear</code> removes it. Notes hold up to 2048 bytes. Spotlight skips hidden folders and <code>/tmp</code>. <code>note -- clear</code> saves the word "clear".</sub>
