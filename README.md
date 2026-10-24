# note

Tag the folder you are in so Spotlight can find it.

```sh
cd ~/Projects/client-work
note prague pitch, march, the blue deck
note find prague
```

The words become the folder's Finder comment. Spotlight indexes that comment, so a search returns the folder. Nothing is added inside the folder.

```text
note [words...]       Set the note
note                  Show it
note clear            Remove it
note find [words...]  List matching folders
```

`clear`, `find`, and `help` are commands when they are the first word. `note -- clear` saves the word "clear".

```sh
cargo build --release
cp target/release/note ~/bin/note
```

macOS only. A note can be 2048 bytes. Spotlight skips hidden folders and `/tmp`.
