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
npm install -g @sendroon/note
```

Or build from source: `cargo install --git https://github.com/baiehclaca/note`

Works on macOS, Linux and Windows (x64; arm64 on macOS and Linux).

**How it works:** your words are stored as the folder's comment, and nothing is added inside the folder. Where that comment is searchable depends on your system:

- **macOS:** the Finder comment. Spotlight and `note find` search it.
- **Linux:** the `user.xdg.comment` attribute, shown by Dolphin and indexed by KDE Baloo (`baloosearch userComment:word`). GNOME search does not read it. `note find` scans your home folder.
- **Windows:** an NTFS alternate data stream named `note.comment`. Windows Search can't see it, so `note find` scans your user folder.

Notes don't survive FAT/exFAT, zip, git or most cloud sync.

<sub><code>note</code> shows the note, <code>note clear</code> removes it. Notes hold up to 2048 bytes. Spotlight skips hidden folders and <code>/tmp</code>. <code>note -- clear</code> saves the word "clear".</sub>
