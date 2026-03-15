Clip Keeper - Clipboard Manager
==============================

A lightweight Linux clipboard manager built with Rust.
Automatically tracks everything you copy and lets you go back to any clip.


FEATURES
--------
- Watches clipboard every 500ms automatically
- Stores history in SQLite (~/.clipboard-manager.db)
- Keeps last 200 clips
- Search through history
- Click any clip to copy it back
- Delete individual clips or clear all
- Dark themed UI


INSTALL
-------
Download the .deb file and run:

    sudo dpkg -i clip-keep_0.1.0_amd64.deb

That's it. Find "Clip Keep" in your app menu under Utilities.


AUTOSTART ON LOGIN
------------------
After installing, enable autostart with:

    systemctl --user enable clip-keep
    systemctl --user start clip-keep

It will now run silently in the background every time you log in.


UNINSTALL
---------
    sudo dpkg -r clip-keep


BUILD FROM SOURCE
-----------------
Requirements:
    sudo apt install libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
                     libxkbcommon-dev pkg-config build-essential

Build:
    cargo build --release

Run:
    cargo run --release

Build .deb package:
    cargo deb


DATA
----
Clipboard history is stored at:
    ~/.clipboard-manager.db

Delete this file to wipe all history.


LICENSE
-------
MIT