<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/logo-dark.svg">
    <source media="(prefers-color-scheme: light)" srcset="assets/logo-light.svg">
    <img alt="note" src="assets/logo-light.svg" width="300">
  </picture>
</p>

<p align="center"><b>Leave a note on any folder. Spotlight remembers it for you.</b></p>

<br>

Your folders have names, but not memories. `client-work` will never tell you it holds the Prague pitch and the blue deck. With `note` you tell the folder once, in plain words, and Spotlight brings it back the day you forget where you put it.

```sh
cd ~/Projects/client-work
note prague pitch, march, the blue deck   # leave the note
note find prague                          # weeks later, from any folder
```

`note` shows the note on the folder you are in, and `note clear` removes it.

## Install

```sh
git clone https://github.com/baiehclaca/note && cd note
cargo build --release
cp target/release/note ~/bin/note
```

macOS only for now. Windows and Linux: coming.

## How it works

Your words become the folder's Finder comment, the same text you see under **Get Info**. Spotlight already indexes that comment, so searching for those words turns up the folder. `note` adds nothing inside the folder: no dotfile, no database, and nothing new for git to notice.

<sub>A note holds up to 2048 bytes. Spotlight skips hidden folders and <code>/tmp</code>. <code>note -- clear</code> saves the word "clear".</sub>
